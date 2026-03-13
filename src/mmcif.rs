use std::path::PathBuf;

#[derive(Debug)]
enum Token {
    TableStart,
    Label((String, String)), // block name, attribute name
    ValueStr(String),
    ValueNum(f32),
}

fn create_token(data: &str) -> Token {
    if data == "_loop" {
        return Token::TableStart;
    }

    if let Some(rest) = data.strip_prefix('_') {
        let dot = data.find('.').unwrap();
        return Token::Label((rest[..dot].to_string(), rest[dot + 1..].to_string()));
    }

    match data.parse::<f32>() {
        Ok(n) => Token::ValueNum(n),
        Err(_) => Token::ValueStr(data.to_string()),
    }
}

fn tokenize(content: &str) -> Vec<Token> {
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < len {
        // Ignore comments
        if bytes[i] == b'#' {
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // Ignore whitespace
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        let start = i;

        if bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i] == b';' {
            // Start quote
            let quote = bytes[i];
            i += 1;

            while i < len && bytes[i] != quote {
                i += 1;
            }

            tokens.push(create_token(&content[start + 1..i]));
            i += 1; // End quote
        } else {
            while i < len && !bytes[i].is_ascii_whitespace() && bytes[i] != b'#' {
                i += 1;
            }
            tokens.push(create_token(&content[start..i]));
        }
    }

    tokens
}

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

pub fn parse() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data/mmcif/T44.cif");

    let tokens = tokenize(&std::fs::read_to_string(path).unwrap());
    println!("{:#?}", tokens);
}
