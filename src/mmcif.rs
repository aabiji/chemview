use glam::Vec3;
use std::collections::HashMap;
use std::path::PathBuf;
use wgpu::naga::FastIndexMap;

type Block = FastIndexMap<String, Vec<String>>;
type Blocks = HashMap<String, Block>;

fn to_vec3(a: &str, b: &str, c: &str) -> Vec3 {
    Vec3::new(
        a.parse::<f32>().unwrap(),
        b.parse::<f32>().unwrap(),
        c.parse::<f32>().unwrap(),
    )
}

fn split_line(line: &str, delim: char) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut start_index = 0;
    let mut end_index = 0;
    let mut found_delim = false;
    let mut in_quotes = false;

    for (i, c) in line.chars().enumerate() {
        if c == '"' {
            in_quotes = !in_quotes;
        }

        if c == delim && !found_delim && !in_quotes {
            end_index = i;
            found_delim = true;
        }

        if c != delim && found_delim {
            parts.push(line[start_index..end_index].to_string());
            found_delim = false;
            start_index = i;
        }

        if i == line.len() - 1 && c != ' ' {
            end_index = line.len();
        }
    }

    parts.push(line[start_index..end_index].to_string());
    parts
}

fn parse_blocks(lines: &mut impl Iterator<Item = String>) -> Blocks {
    let mut block_name = String::new();
    let mut block: Block = FastIndexMap::default();
    let mut blocks: HashMap<String, Block> = HashMap::new();

    // Skip the first mandatory `_data` line` and the next "#" line
    for line in lines.skip(2) {
        // End of current block, start of the next block
        if line.starts_with('#') {
            blocks.insert(block_name.to_string(), block);
            block = FastIndexMap::default();
            continue;
        }

        let line_parts = split_line(&line, ' ');

        if line.starts_with('_') {
            let attr_parts = split_line(&line_parts[0], '.');
            let column_name = attr_parts[1].clone();
            block_name = attr_parts[0].clone();

            if line_parts.len() == 1 {
                block.insert(column_name, Vec::new()); // Table column name
            } else {
                let value = line_parts[1].to_string();
                block.insert(column_name, vec![value]); // Key/value pair
            }
        } else if !line.starts_with("loop_") {
            // Table row
            for i in 0..line_parts.len() {
                block
                    .get_index_mut(i)
                    .unwrap()
                    .1
                    .push(line_parts[i].clone());
            }
        }
    }

    blocks
}

struct Atom {
    component_id: String,
    atom_id: String,
    element: String,
    is_backbone: bool,
    position: Vec3,
}

// TODO: the basic flow is shown, remove the uncessary clones

pub fn parse() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data/mmcif/T44.cif");

    let content = std::fs::read_to_string(path).unwrap();
    let mut lines = content.lines().map(|s| s.to_string());

    let blocks = parse_blocks(&mut lines);
    let atom_block = blocks["_chem_comp_atom"].clone();
    let num_rows = atom_block.get_index(0).unwrap().1.len();

    let atoms = (0..num_rows).map(|i| Atom {
        component_id: atom_block["comp_id"][i].clone(),
        atom_id: atom_block["atom_id"][i].clone(),
        element: atom_block["type_symbol"][i].clone(),
        is_backbone: atom_block["pdbx_backbone_atom_flag"][i] == "Y",
        position: if atom_block.contains_key("model_Cartn_x") {
            to_vec3(
                &atom_block["model_Cartn_x"][i],
                &atom_block["model_Cartn_y"][i],
                &atom_block["model_Cartn_z"][i],
            )
        } else {
            to_vec3(
                &atom_block["pdbx_model_Cartn_x_ideal"][i],
                &atom_block["pdbx_model_Cartn_y_ideal"][i],
                &atom_block["pdbx_model_Cartn_z_ideal"][i],
            )
        },
    });
}
