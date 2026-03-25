use glam::Vec3;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use crate::loader::MMCIFLoader;
use crate::mesh::Shape;

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct AtomKey {
    pub atom_id: Option<String>,
    pub in_ligand: bool,
    // for mmCIF files
    pub chain_id: Option<String>,
    pub sequence_id: Option<String>,
    pub residue: Option<String>,
    // for SDF files
    pub index: Option<usize>,
}

impl AtomKey {
    pub fn from_index(index: usize) -> Self {
        Self {
            atom_id: None,
            in_ligand: true,
            chain_id: None,
            sequence_id: None,
            residue: None,
            index: Some(index),
        }
    }

    pub fn from_ligand(ligand_id: String, atom_id: String) -> Self {
        Self {
            atom_id: Some(atom_id),
            in_ligand: true,
            chain_id: Some(ligand_id),
            sequence_id: None,
            residue: None,
            index: None,
        }
    }

    pub fn from_residue(
        residue: String,
        chain_id: String,
        sequence_id: String,
        atom_id: String,
    ) -> Self {
        Self {
            atom_id: Some(atom_id),
            residue: Some(residue),
            in_ligand: false,
            chain_id: Some(chain_id),
            sequence_id: Some(sequence_id),
            index: None,
        }
    }
}

#[derive(Debug)]
pub struct Bond {
    pub src: AtomKey,
    pub dst: AtomKey,
    pub multiplicity: usize,
}

#[derive(Debug)]
pub struct Atom {
    pub element: String,
    pub position: Vec3,
    pub have_position: bool,
}

#[derive(Default)]
pub struct Structure {
    pub bonds: Vec<Bond>,
    pub atoms: HashMap<AtomKey, Atom>,
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
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data/mmcif/chemical-component-dictionary.cif");
        let mut ccd = MMCIFLoader::default();
        ccd.open_file(&path)?;

        dbg!(&ccd.get_sequence_bonds("PRO"));

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
        multiplicity: usize,
    ) {
        let bond_radius = 0.04;
        let bond_direction = (end_pos - start_pos).normalize();
        let midpoint = (start_pos + end_pos) / 2.0;
        let view_right = bond_direction.cross(camera_front).normalize();

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
            let src_atom = &structure.atoms[&bond.src];
            let dst_atom = &structure.atoms[&bond.dst];
            if !src_atom.have_position
                || !dst_atom.have_position
                || src_atom.element.to_lowercase() == "h"
                || dst_atom.element.to_lowercase() == "h"
            {
                continue;
            }

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
                bond.multiplicity,
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

        for (_, atom) in &structure.atoms {
            if !atom.have_position {
                continue;
            }

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
