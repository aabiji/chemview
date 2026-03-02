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

#[derive(Debug, PartialEq)]
pub struct Compound {
    moniker: String,
    iupac_name: String,
    formula: String,
    is_chiral: bool,
    atoms: Vec<Atom>,
    bonds: Vec<Bond>,
}

#[derive(Debug, PartialEq)]
struct Atom {
    position: Vec3,
    element: String,
}

#[derive(Debug, PartialEq)]
struct Bond {
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

// Parse chemical data from a V2000 SDF file
pub fn parse_compound(contents: &str) -> Result<Compound, String> {
    let lines = split(contents, '\n', false);
    let count_line = parse::<String>(&lines, 3)?;
    let count_fields = split(&count_line, ' ', true);
    let num_atoms = parse::<usize>(&count_fields, 0)?;
    let num_bonds = parse::<usize>(&count_fields, 1)?;

    let mut compound = Compound {
        moniker: String::new(),
        iupac_name: String::new(),
        formula: String::new(),
        is_chiral: parse::<u8>(&count_fields, 3)? == 1,
        atoms: Vec::new(),
        bonds: Vec::new(),
    };

    for i in 0..num_atoms {
        let line = parse::<String>(&lines, 4 + i)?;
        let fields = split(&line, ' ', true);
        compound.atoms.push(Atom {
            position: Vec3::new(
                parse::<f32>(&fields, 0)?,
                parse::<f32>(&fields, 1)?,
                parse::<f32>(&fields, 2)?,
            ),
            element: parse::<String>(&fields, 3)?,
        });
    }

    for i in 0..num_bonds {
        let line = parse::<String>(&lines, 4 + num_atoms + i)?;
        let fields = split(&line, ' ', true);
        compound.bonds.push(Bond {
            src_index: parse::<usize>(&fields, 0)? - 1,
            dst_index: parse::<usize>(&fields, 1)? - 1,
            multiplcity: match parse::<usize>(&fields, 2)? {
                n @ 1..=3 => n,
                m => return Err(format!("Unreconized bond type: {m}")),
            },
        });
    }

    for i in (5 + num_atoms + num_bonds)..lines.len() {
        match lines[i] {
            "> <PUBCHEM_MOLECULAR_FORMULA>" => compound.formula = lines[i + 1].to_string(),
            "> <PUBCHEM_IUPAC_TRADITIONAL_NAME>" => compound.moniker = lines[i + 1].to_string(),
            "> <PUBCHEM_IUPAC_NAME>" => compound.iupac_name = lines[i + 1].to_string(),
            _ => {}
        }
    }

    Ok(compound)
}

fn atom_to_sphere(
    atom: &Atom,
    bond_multiplicity: usize,
    max_radius: f32,
    infos: &HashMap<String, ElementInfo>,
) -> Shape {
    let mut radius = 0;
    let info = &infos[&atom.element];

    // Choose the closest defined covalent radius
    for i in (0..bond_multiplicity).rev() {
        if info.covalent_radius[i] != -1 {
            radius = info.covalent_radius[i];
            break;
        }
    }

    Shape::Sphere {
        origin: atom.position,
        color: Vec3::from_slice(&info.color),
        radius: radius as f32 / max_radius,
    }
}

pub struct CompoundShapes {
    pub shapes: Vec<Shape>,
    pub num_spheres: u32,
    pub bounding_min: Vec3,
    pub bounding_max: Vec3,
}

impl CompoundShapes {
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

    fn update_bounds(&mut self, bounds: (Vec3, Vec3)) {
        self.bounding_min = self.bounding_min.min(bounds.0);
        self.bounding_max = self.bounding_max.max(bounds.1);
    }

    fn from(
        compound: &Compound,
        element_infos: &HashMap<String, ElementInfo>,
        camera_front: Vec3,
    ) -> Self {
        let mut c = CompoundShapes::default(compound.atoms.len());

        let max_covalent_radii = *element_infos
            .values()
            .flat_map(|e| e.covalent_radius.iter())
            .filter(|&&r| r != -1)
            .max()
            .unwrap_or(&0) as f32;

        for bond in &compound.bonds {
            let src_sphere = atom_to_sphere(
                &compound.atoms[bond.src_index],
                bond.multiplcity,
                max_covalent_radii,
                element_infos,
            );
            let dst_sphere = atom_to_sphere(
                &compound.atoms[bond.dst_index],
                bond.multiplcity,
                max_covalent_radii,
                element_infos,
            );

            // Update the bounding box
            c.update_bounds(src_sphere.bounds());
            c.update_bounds(dst_sphere.bounds());

            // Update the radius of the bonded atoms
            c.shapes[bond.src_index] = src_sphere;
            c.shapes[bond.dst_index] = dst_sphere;

            // Position the bonds spread out horizontally relative to the screen
            // The bonds are centered in between the two atoms
            let start = compound.atoms[bond.src_index].position;
            let end = compound.atoms[bond.dst_index].position;

            let bond_direction = (end - start).normalize();
            let view_right = bond_direction.cross(camera_front).normalize();

            let spacing = 0.2;
            let spread = (bond.multiplcity - 1) as f32 * spacing;

            for i in 0..bond.multiplcity {
                let offset = view_right * (i as f32 * spacing - spread / 2.0);
                c.shapes.push(Shape::Cylinder {
                    start: start + offset,
                    end: end + offset,
                    color: Vec3::new(0.67, 0.67, 0.67),
                    radius: 0.045,
                });
            }
        }

        c
    }
}

pub fn load_compound(name: &str, camera_front: Vec3) -> Result<CompoundShapes, String> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let sdf_path = base.join(format!("data/{name}.sdf").as_str());
    let info_path = base.join("data/element_data.json");

    let contents = std::fs::read_to_string(info_path).map_err(|err| err.to_string())?;
    let info: HashMap<String, ElementInfo> =
        serde_json::from_str(&contents).map_err(|err| err.to_string())?;

    let contents = std::fs::read_to_string(&sdf_path).map_err(|err| err.to_string())?;
    let compound = parse_compound(&contents)?;

    Ok(CompoundShapes::from(&compound, &info, camera_front))
}

mod tests {
    #[test]
    fn test_parser() {
        use crate::compound::{Atom, Bond, Compound, parse_compound};
        use glam::Vec3;
        let content = "783
                -OEChem-02172615072D

            2  1  0     0  0  0  0  0  0999 V2000
            2.0000    0.0000    0.0000 H   0  0  0  0  0  0  0  0  0  0  0  0
            3.0000    0.0000    0.0000 H   0  0  0  0  0  0  0  0  0  0  0  0
            1  2  1  0  0  0  0
            M  END
        ";
        let expected = Ok(Compound {
            moniker: String::new(),
            iupac_name: String::new(),
            formula: String::new(),
            is_chiral: false,
            atoms: vec![
                Atom {
                    position: Vec3::new(2.0, 0.0, 0.0),
                    element: "H".to_string(),
                },
                Atom {
                    position: Vec3::new(3.0, 0.0, 0.0),
                    element: "H".to_string(),
                },
            ],
            bonds: vec![Bond {
                src_index: 0,
                dst_index: 1,
                multiplcity: 1,
            }],
        });
        assert!(parse_compound(content) == expected);
    }
}
