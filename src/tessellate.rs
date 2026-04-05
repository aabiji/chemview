use glam::{Mat4, Vec3};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::f32;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use crate::shape::{Shape, Vertex, generate_curve_mesh};

#[derive(Default, Debug)]
pub struct Atom {
    pub chain_id: String,
    pub sequence_id: String,
    pub component_name: String,
    pub atom_id: String,
    pub element: String,
    pub position: Vec3,
}

#[derive(Default, Copy, Clone)]
pub enum BondType {
    #[default]
    Single,
    Double,
    Triple,
    HBond,
}

#[derive(Default)]
pub enum SecondaryType {
    #[default]
    AlphaHelix,
    BetaSheet,
}

#[derive(Default)]
pub struct Bond {
    pub src: usize,
    pub dst: usize,
    pub bond_type: BondType,
}

#[derive(Default)]
pub struct SecondaryStructure {
    pub struct_type: SecondaryType,
    pub start: usize,
    pub end: usize,
}

#[derive(Default)]
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
    Ribbon,
    Wireframe,
    BallAndStick,
    SpaceFilling,
}

impl Display for RenderStyle {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            RenderStyle::Ribbon => write!(f, "Ribbon"),
            RenderStyle::Wireframe => write!(f, "Wireframe"),
            RenderStyle::BallAndStick => write!(f, "Ball and Stick"),
            RenderStyle::SpaceFilling => write!(f, "Space filling"),
        }
    }
}

pub struct TesselateOutput {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub shapes: Vec<Shape>,
    pub bounding_min: Vec3,
    pub bounding_max: Vec3,
}

pub struct Tessellator {
    element_db: HashMap<String, ElementInfo>,
}

impl Tessellator {
    pub fn new() -> Result<Tessellator, String> {
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
    ) -> TesselateOutput {
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
        TesselateOutput {
            vertices: Vec::new(),
            indices: Vec::new(),
            shapes,
            bounding_min,
            bounding_max,
        }
    }

    fn space_filling(&mut self, structure: &Structure) -> TesselateOutput {
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

        TesselateOutput {
            shapes,
            bounding_min,
            bounding_max,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn ribbon(&mut self, structure: &Structure) -> TesselateOutput {
        let mut bounding_min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut bounding_max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for s in &structure.secondary {
            let mut points: Vec<Vec3> = Vec::new();

            // Get the positions of the backbone atoms
            for i in s.start..s.end {
                let is_alpha_carbon = structure.atoms[i].atom_id.to_lowercase() == "ca";
                let p = structure.atoms[i].position;

                bounding_min = bounding_min.min(p);
                bounding_max = bounding_max.max(p);

                if is_alpha_carbon {
                    points.push(p);
                }
            }

            let color = Vec3::new(0.25, 0.5, 1.0);
            let (v, idx) = generate_curve_mesh(&points, vertices.len(), 3, color);
            vertices.extend(v.iter());
            indices.extend(idx.iter());
        }

        TesselateOutput {
            vertices,
            indices,
            bounding_min,
            bounding_max,
            shapes: Vec::new(),
        }
    }

    pub fn tessellate(
        &mut self,
        structure: &Structure,
        camera_front: Vec3,
        view: &RenderStyle,
    ) -> TesselateOutput {
        match view {
            RenderStyle::Ribbon => self.ribbon(structure),
            RenderStyle::BallAndStick | RenderStyle::Wireframe => {
                self.wireframe(structure, camera_front, view == &RenderStyle::Wireframe)
            }
            RenderStyle::SpaceFilling => self.space_filling(structure),
        }
    }
}
