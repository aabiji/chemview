use glam::Vec3;
use std::collections::HashMap;
use std::path::PathBuf;
use wgpu::naga::FastIndexMap;

// Split a string using a delimeter, ignoring all content inside quotes
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

        // End of current part
        if c == delim && !found_delim && !in_quotes {
            end_index = i;
            found_delim = true;
        }

        // Start of part
        if c != delim && found_delim {
            parts.push(line[start_index..end_index].to_string());
            found_delim = false;
            start_index = i;
        }

        // Ignore trailing spaces
        if i == line.len() - 1 && c != ' ' {
            end_index = line.len();
        }
    }

    parts.push(line[start_index..end_index].to_string());
    parts
}

#[derive(Default)]
struct Block {
    // Map column names to column values, keeping the key insertion order intact
    table: FastIndexMap<String, Vec<String>>,
    num_rows: usize,
    name: String,
}

impl Block {
    fn get_str(&self, key: &str, i: usize) -> String {
        self.table[key][i].clone()
    }

    fn get_f32(&self, key: &str, i: usize) -> f32 {
        if self.table[key][i].trim().len() == 0 || self.table[key][i] == "?" {
            return 0.0;
        }
        self.table[key][i].parse::<f32>().unwrap()
    }

    fn get_vec3(&self, a: &str, b: &str, c: &str, i: usize) -> Vec3 {
        Vec3::new(
            self.table[a][i].parse::<f32>().unwrap(),
            self.table[b][i].parse::<f32>().unwrap(),
            self.table[c][i].parse::<f32>().unwrap(),
        )
    }

    fn column_exists(&self, name: &str) -> bool {
        self.table.contains_key(name)
    }
}

fn parse_blocks(lines: &mut impl Iterator<Item = String>) -> HashMap<String, Block> {
    let mut block = Block::default();
    let mut blocks: HashMap<String, Block> = HashMap::new();

    // Skip the first mandatory `_data` line` and the next "#" line
    for line in lines.skip(2) {
        // End of current block, start of the next block
        if line.starts_with('#') {
            blocks.insert(block.name.clone(), block);
            block = Block::default();
            continue;
        }

        let line_parts = split_line(&line, ' ');

        if line.starts_with('_') {
            let attr_parts = split_line(&line_parts[0], '.');
            let column_name = attr_parts[1].clone();
            block.name = attr_parts[0].clone();

            if line_parts.len() == 1 {
                block.table.insert(column_name, Vec::new()); // Table column name
            } else {
                let value = line_parts[1].to_string();
                block.table.insert(column_name, vec![value]); // Key/value pair
                block.num_rows = 1;
            }
        } else if !line.starts_with("loop_") {
            // Table row
            block.num_rows += 1;
            for i in 0..line_parts.len() {
                block
                    .table
                    .get_index_mut(i)
                    .unwrap()
                    .1
                    .push(line_parts[i].clone());
            }
        }
    }

    blocks
}

// TODO: handle types for `_pdbx_struct_assembly_gen` and friends
#[derive(Debug)]
struct Atom {
    position: Vec3,
    part_of_ligand: bool,

    residue_name: String, // ex: ALA, etc
    atom_id: String,      // ex: CA, CB, N, etc
    element: String,      // ex: C, N, O

    // Id the protein strand of which the atom belongs
    chain_id: String,

    // Id of the amino acid of which the atom belongs
    sequence_id: String,
}

#[derive(Debug)]
struct Bond {
    molecule_type: String, // Which molecule this bond belongs to
    src_atom_id: String,
    dst_atom_id: String,
    multiplicity: usize,
}

#[derive(Debug)]
struct Helix {
    helix_type: String,
    // corresponds to ``Atom.sequence_id`
    start_sequence_id: String,
    end_sequence_id: String,
    // corresponds to ``Atom.chain_id`
    start_chain_id: String,
    end_chain_id: String,
}

#[derive(Debug)]
struct Sheet {
    // Which beta sheet this strand belongs to
    sheet_id: String,
    // Stand index within the sheet
    id: String,
    // corresponds to ``Atom.sequence_id`
    start_sequence_id: String,
    end_sequence_id: String,
    // corresponds to ``Atom.chain_id`
    start_chain_id: String,
    end_chain_id: String,
}

#[derive(Debug)]
struct Arrow {
    sheet_id: String,

    // corresponds to `Sheet.id`
    adjacent_strand_id1: String,
    adjacent_strand_id2: String,

    parallel: bool,
}

fn parse_atoms(blocks: &HashMap<String, Block>) -> Vec<Atom> {
    if blocks.contains_key("_atom_site") {
        let block = &blocks["_atom_site"];
        return (0..block.num_rows)
            .filter_map(|i| {
                if block.get_f32("occupancy", i) == 0.0 || block.get_f32("model_num", i) != 1.0 {
                    return None; // Skip zero occupancy
                }

                Some(Atom {
                    position: block.get_vec3("Cartn_x", "Cartn_y", "Cartn_z", i),
                    part_of_ligand: block.get_str("group_PDB", i) == "HETATM",
                    element: block.get_str("type_symbol", i),
                    atom_id: block.get_str("label_atom_id", i),
                    residue_name: block.get_str("label_comp_id", i),
                    sequence_id: block.get_str("label_seq_id", i),
                    chain_id: block.get_str("label_asym_id", i),
                })
            })
            .collect();
    }

    // Fallback to this info
    let block = &blocks["_chem_comp_atom"];
    (0..block.num_rows)
        .filter_map(|i| {
            let residue_name = block.get_str("comp_id", i);
            let element = block.get_str("type_symbol", i);
            let alt_id = block.get_str("label_alt_id", i);

            // Ignore hydrogen and water, since it'll clutter up the rendering
            // and ignore alternate conformations
            if element == "H" || residue_name == "HOH" && alt_id != "." || alt_id != "A" {
                return None;
            }

            return Some(Atom {
                residue_name,
                element,
                atom_id: block.get_str("atom_id", i),
                position: if block.column_exists("pdbx_model_Cartn_x_ideal") {
                    block.get_vec3(
                        "pdbx_model_Cartn_x_ideal",
                        "pdbx_model_Cartn_y_ideal",
                        "pdbx_model_Cartn_z_ideal",
                        i,
                    )
                } else {
                    block.get_vec3("model_Cartn_x", "model_Cartn_y", "model_Cartn_z", i)
                },

                // the following are not available
                chain_id: String::new(),
                sequence_id: String::new(),
                part_of_ligand: false,
            });
        })
        .collect()
}

fn parse_ligand_bonds(blocks: &HashMap<String, Block>) -> Vec<Bond> {
    // This is only for ligands, bonding for proteins completely predictable from chemistry
    // TODO: perform bond inference for the atoms that make up the protein
    let block = &blocks["_chem_comp_bond"];
    if !blocks.contains_key("_atom_site") {
        return Vec::new();
    }

    (0..block.num_rows)
        .map(|i| Bond {
            molecule_type: block.get_str("comp_id", i),
            src_atom_id: block.get_str("atom_id_1", i),
            dst_atom_id: block.get_str("atom_id_2", i),
            multiplicity: match block.get_str("value_order", i).as_str() {
                "DOUB" => 2,
                "TRIP" => 3,
                _ => 1,
            },
        })
        .collect()
}

pub fn parse() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data/mmcif/T44.cif");

    let content = std::fs::read_to_string(path).unwrap();
    let mut lines = content.lines().map(|s| s.to_string());

    let blocks = parse_blocks(&mut lines);
    let atoms = parse_atoms(&blocks);
    let bonds = parse_ligand_bonds(&blocks);

    let block = &blocks["_struct_conf"];
    let helices: Vec<Helix> = (0..block.num_rows)
        .map(|i| Helix {
            helix_type: block.get_str("conf_type_id", i),
            start_sequence_id: block.get_str("beg_label_seq_id", i),
            end_sequence_id: block.get_str("end_label_seq_id", i),
            start_chain_id: block.get_str("beg_label_asym_id", i),
            end_chain_id: block.get_str("end_label_asym_id", i),
        })
        .collect();

    let block = &blocks["_struct_sheet_range"];
    let sheets: Vec<Sheet> = (0..block.num_rows)
        .map(|i| Sheet {
            sheet_id: block.get_str("sheet_id", i),
            id: block.get_str("id", i),
            start_sequence_id: block.get_str("beg_label_seq_id", i),
            end_sequence_id: block.get_str("end_label_seq_id", i),
            start_chain_id: block.get_str("beg_label_asym_id", i),
            end_chain_id: block.get_str("end_label_asym_id", i),
        })
        .collect();

    let block = &blocks["_struct_sheet_order"];
    let arrows: Vec<Arrow> = (0..block.num_rows)
        .map(|i| Arrow {
            sheet_id: block.get_str("sheet_id", i),
            adjacent_strand_id1: block.get_str("range_id_1", i),
            adjacent_strand_id2: block.get_str("range_id_2", i),
            parallel: block.get_str("sense", i) == "parallel",
        })
        .collect();

    println!("{:#?}", atoms);
    println!("{:#?}", bonds);
}
