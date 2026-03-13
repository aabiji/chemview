use std::path::PathBuf;

enum SecondaryStructureType {
    Helix,
    BetaSheet,
}

struct SecondaryStructure {
    struct_type: SecondaryStructureType,
    start: (u32, u32), // chain id, sequence id
    end: (u32, u32),   // chain id, sequence id
}

struct Transformation {
    chains: Vec<u32>, // id of the chains to apply the transform to
    matrix: glam::Mat4,
}

struct Atom {
    chain_id: u32,
    entity_id: u32,
    sequence_id: u32,
    residue_name: String,
    position: glam::Vec3,
}

struct Structure {
    atom: Vec<Atom>,
    aseemblies: Vec<Transformation>,
    structures: Vec<SecondaryStructure>,
}

// - Tokenize bytes -> Seperator, Label(block name, attribute name), value(data)
// - Parse tokens into the structs above

enum Token {
    Seperator,
    Label((String, String)), // block name, attribute name
    Value(String),
}

pub fn parse() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data/mmcif/T44.cif");

    let content = std::fs::read_to_string(path).unwrap();
    let mut lines = content.lines().map(|s| s.to_string());
}
