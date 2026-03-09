use std::collections::HashMap;
use std::path::PathBuf;

use glam::Vec3;

fn parse_vec3(items: &Vec<&str>, indexes: &HashMap<String, usize>, attrs: &[&str]) -> Option<Vec3> {
    let mut values = Vec::new();

    for attr in attrs {
        if !indexes.contains_key(*attr) {
            return None;
        }
        values.push(items[indexes[*attr]].to_string());
    }

    assert!(values.len() == 3);

    Some(Vec3::new(
        values[0].parse::<f32>().unwrap(),
        values[1].parse::<f32>().unwrap(),
        values[2].parse::<f32>().unwrap(),
    ))
}

/*
Things to parse:

`_chem_comp` -> Each chemical component in file
`_atom_site` -> Experimental atom coordinates (not always present -> choose if present)
`_pdbx_struct_oper_list` -> Transformation matrices for generating the biological assembly
`_atom_sites` -> Maps fractionalization coordinates to cartesian coordinates (legacy, always assume coordinates are cartesian)
`_struct_sheet_range` -> Start and end residues for each beta strand
`_struct_conf` -> Start and end residues for each alpha helices
`_struct_sheet_order` -> Parallel vs anti parallel arrows
`_pdbx_poly_seq_scheme` -> Correct residue ordering for spline
`_entity` -> Defines entities by ID
`_pdbx_struct_assembly_gen` -> Which chains to apply which operations to
`_struct_conn` -> More info on bonds like disulfide bonds

Note:
- Only pick one conformation from `_atom_site.label_alt_id`
- Filter out Hydrogen
- Only pick one `_atom_site.pdbx_PDB_model_num` (if it even exists)
- Color differently based off of whether `_atom_site.pdbx_PDB_model_num` is `ATOM` or `HEATM`
- Ignore water
- Skip zero occupancy atoms
- Must use algorithm to determine peptide backbone bonds

*/

pub trait Extractor {
    fn parse_line(&mut self, items: &Vec<&str>, indexes: &HashMap<String, usize>);

    fn debug(&self);
}

#[derive(Default, Debug)]
enum SteroConfig {
    None,
    E,
    Z,
}

#[derive(Default, Debug)]
struct Atom {
    component_id: String,
    atom_id: String,
    element: String,
    aromatic: bool,
    leaving: bool,
    part_of_ligand: bool,
    position: Vec3,
    charge: i32,
    sequence_pos: usize,
    chain_id: String,
}

#[derive(Default, Debug)]
struct Bond {
    component_id: String,
    src_atom_id: String,
    dst_atom_id: String,
    multiplicy: u32,
    aromatic: bool,
    stereo: SteroConfig,
}

struct Helix {
    type_id: String,
    src_seq_index: usize,
    dst_seq_index: usize,
    src_chain_id: String,
    dst_chain_id: String,
    start_insertion_code: String,
    end_insertion_code: String,
}

#[derive(Default, Debug)]
struct AtomList {
    atoms: Vec<Atom>,
}

impl BlockParser {
    fn parse_chem_comp_atom(&mut self, items: &Vec<&str>, indexes: &HashMap<String, usize>) {
        let get = |attr: &str| -> String {
            if indexes.contains_key(attr) {
                return items[indexes[attr]].to_string();
            }
            String::from("0")
        };

        self.atoms.push(Atom {
            component_id: get("comp_id"),
            atom_id: get("atom_id"),
            element: get("type_symbol"),
            aromatic: get("aromatic_flag") == "Y",
            leaving: get("pdbx_leaving_atom_flag") == "Y",
            charge: get("aromatic_flag").parse::<i32>().unwrap_or(0),
            position: parse_vec3(
                items,
                indexes,
                &[
                    "pdbx_model_Cartn_x_ideal",
                    "pdbx_model_Cartn_y_ideal",
                    "pdbx_model_Cartn_z_ideal",
                ],
            )
            .unwrap_or(
                parse_vec3(
                    items,
                    indexes,
                    &["model_Cartn_x", "model_Cartn_z", "model_Cartn_z"],
                )
                .unwrap_or(Vec3::ZERO),
            ),
            ..Atom::default()
        });
    }

    fn parse_atom_site(&mut self, items: &Vec<&str>, indexes: &HashMap<String, usize>) {
        let get = |attr: &str| -> String {
            if indexes.contains_key(attr) {
                return items[indexes[attr]].to_string();
            }
            String::from("0")
        };

        let model_num = get("pdbx_PDB_model_num").parse::<i32>().unwrap_or(0);
        let occupancy = get("occupancy").parse::<f32>().unwrap_or(0.0);
        let alternate_conformation = get("alt_id");
        if occupancy != 1.0 || model_num != 1 || alternate_conformation != "A" {
            return; // FIXME: Only get the highest occupancy/conformation
        }

        self.atoms.push(Atom {
            element: get("type_symbol"),
            atom_id: get("label_atom_id"),
            component_id: get("label_comp_id"),
            sequence_pos: get("label_seq_id").parse::<usize>().unwrap_or(0),
            chain_id: get("label_asym_id"),
            part_of_ligand: get("group_PDB") == "HETATM",
            position: parse_vec3(items, indexes, &["Cartn_x", "Cartn_y", "Cartn_z"])
                .unwrap_or(Vec3::ZERO),
            ..Atom::default()
        });
    }

    fn parse_struct_conf(&mut self, items: &Vec<&str>, indexes: &HashMap<String, usize>) {
        let get = |attr: &str| -> String {
            if indexes.contains_key(attr) {
                return items[indexes[attr]].to_string();
            }
            String::from("0")
        };

        self.helices.push(Helix {
            type_id: get("conf_type_id"),
            src_seq_index: get("beg_label_seq_id").parse::<usize>().unwrap_or(0),
            dst_seq_index: get("end_label_seq_id").parse::<usize>().unwrap_or(0),
            src_chain_id: get("beg_label_asym_id"),
            dst_chain_id: get("end_label_asym_id"),
            start_insertion_code: get("pdbx_beg_PDB_ins_code"),
            end_insertion_code: get("pdbx_end_PDB_ins_code"),
        });

        self.strands.push(Strand {
            beta_sheet_id: get("sheet_id"),
            strand_index: get("id").parse::<usize>().unwrap_or(0),
            dst_seq_index: get("end_label_seq_id").parse::<usize>().unwrap_or(0),
            src_chain_id: get("beg_label_asym_id"),
            dst_chain_id: get("end_label_asym_id"),
            start_insertion_code: get("pdbx_beg_PDB_ins_code"),
            end_insertion_code: get("pdbx_end_PDB_ins_code"),
        });

        self.strand_arrows.push(StrandArrow {
            beta_sheet_id: get("sheet_id"),
            src_id: get("range_id_1").parse::<usize>().unwrap_or(0),
            dst_id: get("range_id_2").parse::<usize>().unwrap_or(0),
            parallel: get("sense") == "anti-parallel",
        });
    }

    fn parse_bond(&mut self, items: &Vec<&str>, indexes: &HashMap<String, usize>) {
        let get = |attr: &str| -> String {
            if indexes.contains_key(attr) {
                return items[indexes[attr]].to_string();
            }
            String::from("0")
        };

        self.bonds.push(Bond {
            component_id: get("comp_id"),
            src_atom_id: get("atom_id_1"),
            dst_atom_id: get("atom_id_2"),
            aromatic: get("aromatic_flag") == "Y",
            stereo: match get("stereo_config").as_str() {
                "E" => SteroConfig::E,
                "z" => SteroConfig::Z,
                _ => SteroConfig::None,
            },
            multiplicy: match get("value_order").as_str() {
                "DOUB" | "doub" => 2,
                "TRIP" | "trip" => 3,
                "QUAD" | "quad" => 4,
                // TODO: handle more bond types: delocaized, aromatic, etc
                _ => 1,
            },
        });
    }

    fn debug(&self) {
        println!("{:?}", *self);
    }
}

fn parse_blocks<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    extractors: &mut HashMap<String, Box<dyn Extractor>>,
) {
    let mut category = "";
    let mut in_table = false;
    let mut column_index = 0;
    let mut col_indexes: HashMap<String, usize> = HashMap::new();

    loop {
        let line = lines.next();
        if let None = line {
            // End of file
            break;
        }
        let line = line.unwrap();

        if line.starts_with('#') {
            // Block demarcator, stop parsing block
            in_table = false;
            col_indexes.clear();
            column_index = 0;
            category = "";
        } else if line.starts_with("loop_") {
            // Start parsing table
            in_table = true;
        } else if line.starts_with('_') {
            let items: Vec<&str> = line.split_ascii_whitespace().collect();
            let tag_items: Vec<&str> = items[0].split(".").collect();
            category = tag_items[0];

            if in_table {
                // Get the index for a column
                let attribute = tag_items[1].to_string();
                col_indexes.insert(attribute, column_index);
                column_index += 1;
            } else if extractors.contains_key(category) {
                // Parse key/value pair
                extractors
                    .get_mut(category)
                    .unwrap()
                    .parse_line(&items, &col_indexes);
            }
        } else if extractors.contains_key(category) {
            // Parse table row
            let items: Vec<&str> = line.split_ascii_whitespace().collect();
            extractors
                .get_mut(category)
                .unwrap()
                .parse_line(&items, &col_indexes);
        }
    }
}

pub fn parse() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data/mmcif/T44.cif");

    let mut extractors: HashMap<String, Box<dyn Extractor>> = HashMap::new();
    extractors.insert(String::from("_chem_comp_atom"), Box::new(Atom::default()));

    let content = std::fs::read_to_string(path).unwrap();
    let mut lines = content.lines();

    parse_blocks(&mut lines, &mut extractors);

    for (_, extractor) in extractors {
        extractor.debug();
    }
}
