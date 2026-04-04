use glam::{Mat4, Vec3};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::f32;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use crate::shape::Shape;

#[derive(Default, Debug)]
pub struct Atom {
    pub chain_id: String,
    pub sequence_id: String,
    pub component_name: String,
    pub atom_id: String,
    pub element: String,
    pub is_ligand: bool,
    pub position: Vec3,
}

#[derive(Default, Debug, Copy, Clone)]
pub enum BondType {
    #[default]
    Single,
    Double,
    Triple,
    HBond,
}

#[derive(Default, Debug)]
pub enum SecondaryType {
    #[default]
    AlphaHelix,
    BetaSheet,
}

#[derive(Default, Debug)]
pub struct Bond {
    pub src: usize,
    pub dst: usize,
    pub bond_type: BondType,
}

#[derive(Default, Debug)]
pub struct SecondaryStructure {
    pub struct_type: SecondaryType,
    pub start: usize,
    pub end: usize,
}

#[derive(Default, Debug)]
pub struct Structure {
    pub atoms: Vec<Atom>,
    pub bonds: Vec<Bond>,
    pub secondary: Vec<SecondaryStructure>,
    pub chain_copies: Vec<(String, Mat4)>,
}

#[derive(Deserialize)]
struct ElementInfo {
    waal_radius: f32,
    covalent_radius: f32,
    color: [f32; 3],
}

#[derive(PartialEq, Clone, Copy)]
pub enum RenderStyle {
    Wireframe,
    BallAndStick,
    SpaceFilling,
}

impl Display for RenderStyle {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            RenderStyle::Wireframe => write!(f, "Wireframe"),
            RenderStyle::BallAndStick => write!(f, "Ball and Stick"),
            RenderStyle::SpaceFilling => write!(f, "Space filling"),
        }
    }
}

pub struct Tessellator {
    element_db: HashMap<String, ElementInfo>,
}

impl Tessellator {
    pub fn new() -> Result<Tessellator, String> {
        /*
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data/mmcif/chemical-component-dictionary.cif");
        let mut ccd = MMCIFLoader::default();
        ccd.open_file(&path)?;
        */

        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let info_path = base.join("data/element_data.json");
        let contents = std::fs::read_to_string(info_path).map_err(|err| err.to_string())?;
        let element_db = serde_json::from_str(&contents).map_err(|err| err.to_string())?;

        Ok(Tessellator { element_db })
    }

    fn add_bond(
        shapes: &mut Vec<Shape>,
        start_pos: Vec3,
        end_pos: Vec3,
        start_color: Vec3,
        end_color: Vec3,
        camera_front: Vec3,
        bond_type: &BondType,
        cap_cylinders: bool,
    ) {
        let bond_radius = 0.04;
        let bond_direction = (end_pos - start_pos).normalize();
        let midpoint = (start_pos + end_pos) / 2.0;
        let view_right = bond_direction.cross(camera_front).normalize();

        let multiplicity = match bond_type {
            BondType::Single => 1,
            BondType::Double => 2,
            BondType::Triple => 3,
            BondType::HBond => 0, // FIXME!
        };
        let spacing = 0.15;
        let spread = (multiplicity - 1) as f32 * spacing;

        // Orient the bond to be facing the camera
        // Split the bonds in half to handle the wireframe render type cleanly
        for i in 0..multiplicity {
            let offset = view_right * (i as f32 * spacing - spread / 2.0);

            shapes.push(Shape::Cylinder {
                start: start_pos + offset,
                end: midpoint + offset,
                color: start_color,
                radius: bond_radius,
            });

            shapes.push(Shape::Cylinder {
                start: midpoint + offset,
                end: end_pos + offset,
                color: end_color,
                radius: bond_radius,
            });

            if cap_cylinders {
                shapes.push(Shape::Sphere {
                    // bottom cap
                    origin: start_pos + offset,
                    color: start_color,
                    radius: bond_radius,
                });
                shapes.push(Shape::Sphere {
                    // top cap
                    origin: end_pos + offset,
                    color: end_color,
                    radius: bond_radius,
                });
            }
        }
    }

    fn wireframe(
        &mut self,
        structure: &Structure,
        camera_front: Vec3,
        wireframe: bool,
    ) -> (Vec<Shape>, Vec3, Vec3) {
        let mut sphere_set: HashSet<Shape> = HashSet::new();
        let mut cylinders: Vec<Shape> = Vec::new();
        let mut bounding_min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut bounding_max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        let bond_color = Vec3::new(0.67, 0.67, 0.67);
        let radius_scale = 0.5;

        for bond in &structure.bonds {
            let src_atom = &structure.atoms[bond.src];
            let dst_atom = &structure.atoms[bond.dst];
            let src_color = Vec3::from_slice(&self.element_db[&src_atom.element].color);
            let dst_color = Vec3::from_slice(&self.element_db[&dst_atom.element].color);

            let src_sphere = Shape::Sphere {
                origin: src_atom.position,
                color: src_color,
                radius: self.element_db[&src_atom.element].covalent_radius * radius_scale,
            };

            let dst_sphere = Shape::Sphere {
                origin: dst_atom.position,
                color: dst_color,
                radius: self.element_db[&dst_atom.element].covalent_radius * radius_scale,
            };

            // Position the bonds spread out horizontally relative to the screen
            // The bonds are centered in between the two atoms
            Self::add_bond(
                &mut cylinders,
                src_atom.position,
                dst_atom.position,
                if wireframe { src_color } else { bond_color },
                if wireframe { dst_color } else { bond_color },
                camera_front,
                &bond.bond_type,
                wireframe,
            );

            bounding_min = bounding_min
                .min(src_sphere.bounds().0)
                .min(dst_sphere.bounds().0);
            bounding_max = bounding_max
                .max(src_sphere.bounds().1)
                .max(dst_sphere.bounds().1);

            if !wireframe {
                sphere_set.insert(src_sphere);
                sphere_set.insert(dst_sphere);
            }
        }

        let mut shapes: Vec<Shape> = sphere_set.iter().cloned().collect();
        shapes.append(&mut cylinders);
        (shapes, bounding_min, bounding_max)
    }

    fn space_filling(&mut self, structure: &Structure) -> (Vec<Shape>, Vec3, Vec3) {
        let mut bounding_min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut bounding_max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);
        let mut shapes: Vec<Shape> = Vec::new();

        for atom in &structure.atoms {
            let shape = Shape::Sphere {
                origin: atom.position,
                color: Vec3::from_slice(&self.element_db[&atom.element].color),
                radius: self.element_db[&atom.element].waal_radius,
            };
            bounding_min = bounding_min.min(shape.bounds().0);
            bounding_max = bounding_max.max(shape.bounds().1);
            shapes.push(shape);
        }

        (shapes, bounding_max, bounding_max)
    }

    pub fn tessellate(
        &mut self,
        structure: &Structure,
        camera_front: Vec3,
        view: &RenderStyle,
    ) -> (Vec<Shape>, Vec3, Vec3) {
        match view {
            RenderStyle::BallAndStick | RenderStyle::Wireframe => {
                self.wireframe(structure, camera_front, view == &RenderStyle::Wireframe)
            }
            RenderStyle::SpaceFilling => self.space_filling(structure),
        }
    }
}
