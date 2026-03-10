use std::path::PathBuf;

pub fn parse() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data/mmcif/T44.cif");

    let content = std::fs::read_to_string(path).unwrap();
    let lines = content.lines();
}
