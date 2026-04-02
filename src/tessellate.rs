use glam::{Mat4, Vec3};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use crate::loader::MMCIFLoader;
use crate::mesh::Shape;

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

#[derive(Default)]
pub struct TessellateOutput {
    pub shapes: Vec<Shape>,
    pub num_spheres: usize,
    pub bounding_min: Vec3,
    pub bounding_max: Vec3,
}

pub struct Tessellator {
    element_db: HashMap<String, ElementInfo>,
    ccd: MMCIFLoader,
}

impl Tessellator {
    pub fn new() -> Result<Tessellator, String> {
        /*
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data/mmcif/chemical-component-dictionary.cif");
        let mut ccd = MMCIFLoader::default();
        ccd.open_file(&path)?;
        */
        let mut ccd = MMCIFLoader::default();

        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let info_path = base.join("data/element_data.json");
        let contents = std::fs::read_to_string(info_path).map_err(|err| err.to_string())?;
        let element_db = serde_json::from_str(&contents).map_err(|err| err.to_string())?;

        Ok(Tessellator { element_db, ccd })
    }

    fn add_bond(
        shapes: &mut Vec<Shape>,
        start_pos: Vec3,
        end_pos: Vec3,
        start_color: Vec3,
        end_color: Vec3,
        camera_front: Vec3,
        bond_type: &BondType,
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
        }
    }

    fn wireframe(
        &mut self,
        structure: &Structure,
        camera_front: Vec3,
        wireframe: bool,
    ) -> TessellateOutput {
        let mut output = TessellateOutput::default();

        let mut sphere_set: HashSet<Shape> = HashSet::new();
        let mut cylinders: Vec<Shape> = Vec::new();

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
            );

            output.bounding_min = output
                .bounding_min
                .min(src_sphere.bounds().0)
                .min(dst_sphere.bounds().0);
            output.bounding_min = output
                .bounding_max
                .max(src_sphere.bounds().1)
                .max(dst_sphere.bounds().1);

            if !wireframe {
                sphere_set.insert(src_sphere);
                sphere_set.insert(dst_sphere);
            }
        }

        output.num_spheres = sphere_set.len();
        output.shapes = sphere_set.iter().cloned().collect();
        output.shapes.extend(cylinders.drain(..));
        output
    }

    fn space_filling(&mut self, structure: &Structure) -> TessellateOutput {
        let mut output = TessellateOutput::default();

        for atom in &structure.atoms {
            let shape = Shape::Sphere {
                origin: atom.position,
                color: Vec3::from_slice(&self.element_db[&atom.element].color),
                radius: self.element_db[&atom.element].waal_radius,
            };
            output.bounding_min = output.bounding_min.min(shape.bounds().0);
            output.bounding_max = output.bounding_max.max(shape.bounds().1);
            output.shapes.push(shape);
        }

        output.num_spheres = output.shapes.len();
        output
    }

    pub fn tessellate(
        &mut self,
        structure: &Structure,
        camera_front: Vec3,
        view: &RenderStyle,
    ) -> TessellateOutput {
        match view {
            RenderStyle::BallAndStick | RenderStyle::Wireframe => {
                self.wireframe(structure, camera_front, view == &RenderStyle::Wireframe)
            }
            RenderStyle::SpaceFilling => self.space_filling(structure),
        }
    }
}
