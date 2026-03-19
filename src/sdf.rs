use crate::mesh::{CompoundMeshInfo, Shape};
use crate::pipeline::{CompoundPipeline, ElementDB, ViewType, load_element_db};
use glam::Vec3;
use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct Atom {
    position: Vec3,
    element: String,
}

#[derive(Debug, PartialEq)]
pub struct Bond {
    src_index: usize,
    dst_index: usize,
    multiplicity: usize, // single, double or triple bond
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
fn parse_sdf_content(contents: &str) -> Result<(String, String, Vec<Atom>, Vec<Bond>), String> {
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
            multiplicity: match parse::<usize>(&fields, 2)? {
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

pub struct SDFLoader {
    name: String,
    formula: String,
    atoms: Vec<Atom>,
    bonds: Vec<Bond>,
    element_db: ElementDB,
}

impl SDFLoader {
    pub fn init() -> Result<Self, String> {
        Ok(Self {
            name: String::new(),
            formula: String::new(),
            atoms: Vec::new(),
            bonds: Vec::new(),
            element_db: load_element_db()?,
        })
    }

    fn atom_to_sphere(
        &self,
        atom: &Atom,
        bond_multiplicity: usize,
        max_radius: f32,
        use_waal_radius: bool,
    ) -> Shape {
        let info = &self.element_db[&atom.element];

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
            radius: radius / max_radius,
        }
    }
}

impl CompoundPipeline for SDFLoader {
    fn parse_file(&mut self, path: &Path) -> Result<(), String> {
        let contents = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
        (self.name, self.formula, self.atoms, self.bonds) = parse_sdf_content(&contents)?;
        Ok(())
    }

    fn compute_mesh_info(&mut self, camera_front: Vec3, view: &ViewType) -> CompoundMeshInfo {
        let mut mesh = CompoundMeshInfo::default(self.atoms.len());

        let max_covalent_radii = *self
            .element_db
            .values()
            .flat_map(|e| e.covalent_radius.iter())
            .filter(|&&r| r != -1)
            .max()
            .unwrap_or(&0) as f32;

        for bond in &self.bonds {
            let src_sphere = self.atom_to_sphere(
                &self.atoms[bond.src_index],
                bond.multiplicity,
                max_covalent_radii,
                *view == ViewType::SpacingFilling,
            );
            let dst_sphere = self.atom_to_sphere(
                &self.atoms[bond.dst_index],
                bond.multiplicity,
                max_covalent_radii,
                *view == ViewType::SpacingFilling,
            );

            // Update the bounding box
            mesh.update_bounds(src_sphere.bounds());
            mesh.update_bounds(dst_sphere.bounds());

            // Update the radius of the bonded atoms
            mesh.shapes[bond.src_index] = src_sphere;
            mesh.shapes[bond.dst_index] = dst_sphere;

            // Only need to render bonds in the ball and stick model
            if *view != ViewType::SpacingFilling {
                // Position the bonds spread out horizontally relative to the screen
                // The bonds are centered in between the two atoms
                let start = self.atoms[bond.src_index].position;
                let end = self.atoms[bond.dst_index].position;
                mesh.add_bond(start, end, camera_front, bond.multiplicity);
            }
        }

        mesh
    }
}
