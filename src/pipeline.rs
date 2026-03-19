use crate::mesh::CompoundMeshInfo;
use glam::Vec3;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use std::fmt::{self, Display, Formatter};

#[derive(Deserialize)]
pub struct ElementInfo {
    pub waal_radius: i32,
    pub covalent_radius: [i32; 3],
    pub color: [f32; 3],
}

pub type ElementDB = HashMap<String, ElementInfo>;

pub fn load_element_db() -> Result<ElementDB, String> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let info_path = base.join("data/element_data.json");
    let contents = std::fs::read_to_string(info_path).map_err(|err| err.to_string())?;
    serde_json::from_str(&contents).map_err(|err| err.to_string())
}

#[derive(PartialEq)]
pub enum ViewType {
    BallAndStick,
    SpacingFilling,
}

impl Display for ViewType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            ViewType::BallAndStick => write!(f, "Ball and Stick"),
            ViewType::SpacingFilling => write!(f, "Space filling"),
        }
    }
}

pub trait CompoundPipeline {
    fn parse_file(&mut self, path: &Path) -> Result<(), String>;

    fn compute_mesh_info(&mut self, camera_front: Vec3, view: &ViewType) -> CompoundMeshInfo;
}
