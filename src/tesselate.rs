use glam::Vec3;
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use crate::loader::MMCIFLoader;
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

impl Ligand {
    fn get_atom(&self, index: &Option<usize>, id: &Option<String>) -> &Atom {
        if let Some(i) = index {
            self.atoms.get_index(*i).map(|(_, v)| v).unwrap()
        } else {
            let id = id.as_ref().unwrap();
            self.atoms.get(id).unwrap()
        }
    }
}

#[derive(Default)]
pub struct Structure {
    pub ligands: HashMap<String, Ligand>, // ligand id to ligand
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
pub struct TesselateOutput {
    pub shapes: Vec<Shape>,
    pub num_spheres: usize,
    pub bounding_min: Vec3,
    pub bounding_max: Vec3,
}

pub struct Tesselator {
    element_db: HashMap<String, ElementInfo>,
    ccd: MMCIFLoader,
}

impl Tesselator {
    pub fn new() -> Result<Tesselator, String> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data/mmcif/chemical-componnet-dictionary.cif");
        let mut ccd = MMCIFLoader::default();
        ccd.open_file(&path)?;

        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let info_path = base.join("data/element_data.json");
        let contents = std::fs::read_to_string(info_path).map_err(|err| err.to_string())?;
        let element_db = serde_json::from_str(&contents).map_err(|err| err.to_string())?;

        Ok(Tesselator { element_db, ccd })
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
    ) -> TesselateOutput {
        let mut output = TesselateOutput::default();

        let mut sphere_set: HashSet<Shape> = HashSet::new();
        let mut cylinders: Vec<Shape> = Vec::new();

        let bond_color = Vec3::new(0.67, 0.67, 0.67);
        let radius_scale = 0.5;

        for (_, ligand) in &structure.ligands {
            for bond in &ligand.bonds {
                let src_atom = ligand.get_atom(&bond.src_index, &bond.src_id);
                let dst_atom = ligand.get_atom(&bond.dst_index, &bond.dst_id);

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
        }

        output.num_spheres = sphere_set.len();
        output.shapes = sphere_set.iter().cloned().collect();
        output.shapes.extend(cylinders.drain(..));

        output
    }

    fn space_filling(&mut self, structure: &Structure) -> TesselateOutput {
        let mut output = TesselateOutput::default();

        for (_, ligand) in &structure.ligands {
            for (_, atom) in &ligand.atoms {
                let shape = Shape::Sphere {
                    origin: atom.position,
                    color: Vec3::from_slice(&self.element_db[&atom.element].color),
                    radius: self.element_db[&atom.element].waal_radius,
                };

                output.bounding_min = output.bounding_min.min(shape.bounds().0);
                output.bounding_max = output.bounding_max.max(shape.bounds().1);
                output.shapes.push(shape);
            }
        }

        output.num_spheres = output.shapes.len();
        output
    }

    pub fn tesselate(
        &mut self,
        structure: &Structure,
        camera_front: Vec3,
        view: &RenderStyle,
    ) -> TesselateOutput {
        match view {
            RenderStyle::BallAndStick | RenderStyle::Wireframe => {
                self.wireframe(structure, camera_front, view == &RenderStyle::Wireframe)
            }
            RenderStyle::SpaceFilling => self.space_filling(structure),
        }
    }
}
