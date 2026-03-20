use glam::Vec3;
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use crate::mesh::Shape;

#[derive(Debug)]
pub struct Bond {
    // for mmcif
    pub src_id: Option<String>,
    pub dst_id: Option<String>,

    // for sdf
    pub src_index: Option<usize>,
    pub dst_index: Option<usize>,

    pub multiplicity: usize,
}

#[derive(Debug)]
pub struct Atom {
    pub element: String,
    pub position: Vec3,
}

#[derive(Default, Debug)]
pub struct Ligand {
    pub bonds: Vec<Bond>,
    // Atom ID to atom, with preserved insertion order
    pub atoms: IndexMap<String, Atom>,
}

pub type Chain = IndexMap<String, Vec<Atom>>; // Sequence id to residue atoms

#[derive(Default)]
pub struct Structure {
    pub ligands: HashMap<String, Ligand>, // ligand id to ligand
}

impl Structure {
    // TODO: what about chains?
    fn num_atoms(&self) -> usize {
        self.ligands.values().map(|l| l.atoms.len()).sum()
    }
}

#[derive(Deserialize)]
struct ElementInfo {
    waal_radius: i32,
    covalent_radius: [i32; 3],
    color: [f32; 3],
}

type ElementDB = HashMap<String, ElementInfo>;

fn load_element_db() -> Result<ElementDB, String> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let info_path = base.join("data/element_data.json");
    let contents = std::fs::read_to_string(info_path).map_err(|err| err.to_string())?;
    serde_json::from_str(&contents).map_err(|err| err.to_string())
}

#[derive(PartialEq)]
pub enum RenderStyle {
    BallAndStick,
    SpacingFilling,
}

impl Display for RenderStyle {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            RenderStyle::BallAndStick => write!(f, "Ball and Stick"),
            RenderStyle::SpacingFilling => write!(f, "Space filling"),
        }
    }
}

#[derive(Default)]
pub struct Tesselator {
    pub shapes: Vec<Shape>,
    pub num_spheres: usize,
    pub bounding_min: Vec3,
    pub bounding_max: Vec3,

    element_db: ElementDB,
}

impl Tesselator {
    pub fn update_bounds(&mut self, bounds: (Vec3, Vec3)) {
        self.bounding_min = self.bounding_min.min(bounds.0);
        self.bounding_max = self.bounding_max.max(bounds.1);
    }

    pub fn add_bond(
        &mut self,
        start_pos: Vec3,
        end_pos: Vec3,
        camera_front: Vec3,
        multiplicity: usize,
    ) {
        let bond_direction = (end_pos - start_pos).normalize();
        let view_right = bond_direction.cross(camera_front).normalize();

        let spacing = 0.2;
        let spread = (multiplicity - 1) as f32 * spacing;

        for i in 0..multiplicity {
            let offset = view_right * (i as f32 * spacing - spread / 2.0);
            self.shapes.push(Shape::Cylinder {
                start: start_pos + offset,
                end: end_pos + offset,
                color: Vec3::new(0.67, 0.67, 0.67),
                radius: 0.045,
            });
        }
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

    // TODO: fix this!
    pub fn tesselate(&mut self, structure: &Structure, camera_front: Vec3, view: &RenderStyle) {
        let default_sphere = Shape::Sphere {
            origin: Vec3::ZERO,
            color: Vec3::ZERO,
            radius: 0.0,
        };
        self.shapes = vec![default_sphere; structure.num_atoms()];
        self.num_spheres = structure.num_atoms();

        let max_covalent_radii = *self
            .element_db
            .values()
            .flat_map(|e| e.covalent_radius.iter())
            .filter(|&&r| r != -1)
            .max()
            .unwrap_or(&0) as f32;

        for (_, ligand) in structure.ligands {
            for bond in &ligand.bonds {
                let src_sphere = self.atom_to_sphere(
                    &self.atoms[bond.src_index],
                    bond.multiplicity,
                    max_covalent_radii,
                    *view == RenderStyle::SpacingFilling,
                );
                let dst_sphere = self.atom_to_sphere(
                    &self.atoms[bond.dst_index],
                    bond.multiplicity,
                    max_covalent_radii,
                    *view == RenderStyle::SpacingFilling,
                );

                // Update the bounding box
                self.update_bounds(src_sphere.bounds());
                self.update_bounds(dst_sphere.bounds());

                // Update the radius of the bonded atoms
                self.shapes[bond.src_index] = src_sphere;
                self.shapes[bond.dst_index] = dst_sphere;

                // Only need to render bonds in the ball and stick model
                if *view != RenderStyle::SpacingFilling {
                    // Position the bonds spread out horizontally relative to the screen
                    // The bonds are centered in between the two atoms
                    let start = self.atoms[bond.src_index].position;
                    let end = self.atoms[bond.dst_index].position;
                    self.add_bond(start, end, camera_front, bond.multiplicity);
                }
            }
        }
    }
}
