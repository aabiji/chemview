use crate::shape::Shape;
use glam::Vec3;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct ElementInfo {
    waal_radius: i32,
    covalent_radius: [i32; 3],
    color: [f32; 3],
}

pub fn load_element_info() -> Result<HashMap<String, ElementInfo>, String> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let info_path = base.join("data/element_data.json");
    let contents = std::fs::read_to_string(info_path).map_err(|err| err.to_string())?;
    serde_json::from_str(&contents).map_err(|err| err.to_string())
}

#[derive(Debug, PartialEq)]
pub struct Atom {
    position: Vec3,
    element: String,
}

#[derive(Debug, PartialEq)]
pub struct Bond {
    src_index: usize,
    dst_index: usize,
    multiplcity: usize, // single, double or triple bond
}

fn split(lines: &str, sep: char, strip: bool) -> Vec<&str> {
    lines
        .split(sep)
        .filter(|x| !strip || !x.is_empty())
        .collect()
}

fn parse<T: std::str::FromStr>(v: &Vec<&str>, index: usize) -> Result<T, String> {
    let element = v.get(index).ok_or(String::from("Missing value"))?;
    element
        .parse::<T>()
        .map_err(|_| String::from("Invalid value"))
}

// Parse chemical data from a V2000 SDF file and return the compound
// name, formula, atoms and bonds
pub fn parse_compound(contents: &str) -> Result<(String, String, Vec<Atom>, Vec<Bond>), String> {
    let lines = split(contents, '\n', false);
    let count_line = parse::<String>(&lines, 3)?;
    let count_fields = split(&count_line, ' ', true);
    let num_atoms = parse::<usize>(&count_fields, 0)?;
    let num_bonds = parse::<usize>(&count_fields, 1)?;

    let mut atoms = Vec::new();
    for i in 0..num_atoms {
        let line = parse::<String>(&lines, 4 + i)?;
        let fields = split(&line, ' ', true);
        atoms.push(Atom {
            position: Vec3::new(
                parse::<f32>(&fields, 0)?,
                parse::<f32>(&fields, 1)?,
                parse::<f32>(&fields, 2)?,
            ),
            element: parse::<String>(&fields, 3)?,
        });
    }

    let mut bonds = Vec::new();
    for i in 0..num_bonds {
        let line = parse::<String>(&lines, 4 + num_atoms + i)?;
        let fields = split(&line, ' ', true);
        bonds.push(Bond {
            src_index: parse::<usize>(&fields, 0)? - 1,
            dst_index: parse::<usize>(&fields, 1)? - 1,
            multiplcity: match parse::<usize>(&fields, 2)? {
                n @ 1..=3 => n,
                m => return Err(format!("Unreconized bond type: {m}")),
            },
        });
    }

    let mut saw_iupac = false;
    let mut name = String::from("Unkown compound");
    let mut formula = String::from("Unkonwn formula");
    for i in (5 + num_atoms + num_bonds)..lines.len() {
        match lines[i] {
            "> <PUBCHEM_MOLECULAR_FORMULA>" => formula = lines[i + 1].to_string(),
            "> <PUBCHEM_IUPAC_TRADITIONAL_NAME>" => {
                if !saw_iupac {
                    name = lines[i + 1].to_string();
                }
            }
            "> <PUBCHEM_IUPAC_NAME>" => {
                name = lines[i + 1].to_string();
                saw_iupac = true;
            }
            _ => {}
        }
    }

    Ok((name, formula, atoms, bonds))
}

pub struct CompoundMesh {
    pub shapes: Vec<Shape>,
    pub num_spheres: u32,
    pub bounding_min: Vec3,
    pub bounding_max: Vec3,
}

impl CompoundMesh {
    fn default(num_spheres: usize) -> Self {
        let default_sphere = Shape::Sphere {
            origin: Vec3::ZERO,
            color: Vec3::ZERO,
            radius: 0.0,
        };

        Self {
            num_spheres: num_spheres as u32,
            shapes: vec![default_sphere; num_spheres],
            bounding_min: Vec3::new(1000000.0, 1000000.0, 1000000.0),
            bounding_max: Vec3::new(-1000000.0, -1000000.0, -1000000.0),
        }
    }
}

fn atom_to_sphere(
    atom: &Atom,
    bond_multiplicity: usize,
    max_radius: f32,
    use_waal_radius: bool,
    infos: &HashMap<String, ElementInfo>,
) -> Shape {
    let info = &infos[&atom.element];

    let radius = if use_waal_radius {
        if info.waal_radius == -1 {
            1.0
        } else {
            info.waal_radius as f32
        }
    } else {
        // Choose the closest defined covalent radius
        *info
            .covalent_radius
            .iter()
            .take(bond_multiplicity)
            .rev()
            .find(|v| **v != -1)
            .unwrap() as f32
    };

    Shape::Sphere {
        origin: atom.position,
        color: Vec3::from_slice(&info.color),
        radius: radius as f32 / max_radius,
    }
}

pub fn assemble_mesh(
    atoms: Vec<Atom>,
    bonds: Vec<Bond>,
    element_infos: &HashMap<String, ElementInfo>,
    camera_front: Vec3,
    use_waal_radius: bool,
) -> CompoundMesh {
    let mut mesh = CompoundMesh::default(atoms.len());

    let mut update_bounds = |bounds: (Vec3, Vec3)| {
        mesh.bounding_min = mesh.bounding_min.min(bounds.0);
        mesh.bounding_max = mesh.bounding_max.max(bounds.1);
    };

    let max_covalent_radii = *element_infos
        .values()
        .flat_map(|e| e.covalent_radius.iter())
        .filter(|&&r| r != -1)
        .max()
        .unwrap_or(&0) as f32;

    for bond in &bonds {
        let src_sphere = atom_to_sphere(
            &atoms[bond.src_index],
            bond.multiplcity,
            max_covalent_radii,
            use_waal_radius,
            element_infos,
        );
        let dst_sphere = atom_to_sphere(
            &atoms[bond.dst_index],
            bond.multiplcity,
            max_covalent_radii,
            use_waal_radius,
            element_infos,
        );

        // Update the bounding box
        update_bounds(src_sphere.bounds());
        update_bounds(dst_sphere.bounds());

        // Update the radius of the bonded atoms
        mesh.shapes[bond.src_index] = src_sphere;
        mesh.shapes[bond.dst_index] = dst_sphere;

        // Only need to render bonds in the ball and stick model
        if !use_waal_radius {
            // Position the bonds spread out horizontally relative to the screen
            // The bonds are centered in between the two atoms
            let start = atoms[bond.src_index].position;
            let end = atoms[bond.dst_index].position;

            let bond_direction = (end - start).normalize();
            let view_right = bond_direction.cross(camera_front).normalize();

            let spacing = 0.2;
            let spread = (bond.multiplcity - 1) as f32 * spacing;

            for i in 0..bond.multiplcity {
                let offset = view_right * (i as f32 * spacing - spread / 2.0);
                mesh.shapes.push(Shape::Cylinder {
                    start: start + offset,
                    end: end + offset,
                    color: Vec3::new(0.67, 0.67, 0.67),
                    radius: 0.045,
                });
            }
        }
    }

    mesh
}
