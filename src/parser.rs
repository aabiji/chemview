use std::collections::HashMap;
use std::path::PathBuf;

use glam::Vec3;

fn to_vec3(a: &str, b: &str, c: &str) -> Vec3 {
    Vec3::new(
        a.parse::<f32>().unwrap(),
        b.parse::<f32>().unwrap(),
        c.parse::<f32>().unwrap(),
    )
}

/*
Things to parse:

`_chem_comp` -> Each chemical component in file
`_chem_comp_atom` -> Ideal atom coordinates (always present)
`_chem_comp_bond` -> Atom bonding
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
    fn parse_line(&mut self, line_items: &Vec<&str>, attribute_indices: &HashMap<String, usize>);

    fn debug(&self);
}

// Each field stores a list of fields, instead of the user storing a list of Atom.
// This is done to make sequential reads more cache friendly.
#[derive(Default, Debug)]
struct Atom {
    ids: Vec<String>,
    positions: Vec<Vec3>,
}

impl Extractor for Atom {
    fn parse_line(&mut self, line_items: &Vec<&str>, attribute_indices: &HashMap<String, usize>) {
        if let Some(i) = attribute_indices.get("comp_id") {
            self.ids.push(line_items[*i].to_string());
        }

        // TODO: panic in case these don't exist
        self.positions.push(to_vec3(
            &line_items[attribute_indices["model_Cartn_x"]],
            &line_items[attribute_indices["model_Cartn_y"]],
            &line_items[attribute_indices["model_Cartn_z"]],
        ));
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
    let mut attribute_indices: HashMap<String, usize> = HashMap::new();

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
            attribute_indices.clear();
            column_index = 0;
            category = "";
        } else if line.starts_with("loop_") {
            // Start parsing table
            in_table = true;
        } else if line.starts_with('_') {
            let line_items: Vec<&str> = line.split_ascii_whitespace().collect();
            let tag_items: Vec<&str> = line_items[0].split(".").collect();
            category = tag_items[0];

            if in_table {
                // Get the index for an attribute
                let attribute = tag_items[1].to_string();
                attribute_indices.insert(attribute, column_index);
                column_index += 1;
            } else if extractors.contains_key(category) {
                // Parse key/value pair
                extractors
                    .get_mut(category)
                    .unwrap()
                    .parse_line(&line_items, &attribute_indices);
            }
        } else if extractors.contains_key(category) {
            // Parse table row
            let line_items: Vec<&str> = line.split_ascii_whitespace().collect();
            extractors
                .get_mut(category)
                .unwrap()
                .parse_line(&line_items, &attribute_indices);
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
