use crate::shape::CompoundMeshInfo;
use glam::Vec3;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

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

pub trait CompoundPipeline: Sized {
    fn init() -> Result<Self, String>;

    fn parse_file(&mut self, path: &PathBuf) -> Result<(), String>;

    fn compute_mesh_info(&mut self, camera_front: Vec3, use_waal_radius: bool) -> CompoundMeshInfo;
}
