use glam::{Mat4, Vec3, Vec4};
use indexmap::IndexMap;
use itertools::Itertools;
use memchr::memchr;
use memchr::memmem::find_iter;
use memmap::{Mmap, MmapOptions};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use crate::tessellate::{Atom, Bond, BondType, SecondaryStructure, SecondaryType, Structure};

pub trait FileLoader: Send {
    fn parse_file(&mut self, path: &Path) -> Result<Structure, String>;
}

fn split(lines: &str, sep: char, strip: bool) -> Vec<&str> {
    lines
        .split(sep)
        .filter(|x| !strip || !x.is_empty())
        .collect()
}

fn parse<T: std::str::FromStr>(v: &Vec<&str>, index: usize) -> Result<T, String> {
    let element = v.get(index).ok_or(String::from("Missing value"))?;
    element
        .parse::<T>()
        .map_err(|_| String::from("Invalid value"))
}

pub struct SDFLoader {}

impl FileLoader for SDFLoader {
    fn parse_file(&mut self, path: &Path) -> Result<Structure, String> {
        let contents = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
        let lines = split(&contents, '\n', false);

        let count_line = parse::<String>(&lines, 3)?;
        let count_fields = split(&count_line, ' ', true);
        let num_atoms = parse::<usize>(&count_fields, 0)?;
        let num_bonds = parse::<usize>(&count_fields, 1)?;

        let mut atoms: Vec<Atom> = Vec::new();
        let mut bonds: Vec<Bond> = Vec::new();

        for i in 0..num_atoms {
            let line = parse::<String>(&lines, 4 + i)?;
            let fields = split(&line, ' ', true);
            atoms.push(Atom {
                chain_id: String::new(),
                sequence_id: String::new(),
                component_name: String::new(),
                atom_id: String::new(),
                is_ligand: true,
                position: Vec3::new(
                    parse::<f32>(&fields, 0)?,
                    parse::<f32>(&fields, 1)?,
                    parse::<f32>(&fields, 2)?,
                ),
                element: parse::<String>(&fields, 3)?,
            });
        }

        for i in 0..num_bonds {
            let line = parse::<String>(&lines, 4 + num_atoms + i)?;
            let fields = split(&line, ' ', true);
            bonds.push(Bond {
                src: parse::<usize>(&fields, 0)? - 1,
                dst: parse::<usize>(&fields, 1)? - 1,
                bond_type: match parse::<usize>(&fields, 2)? {
                    1 => BondType::Single,
                    2 => BondType::Double,
                    3 => BondType::Triple,
                    m => return Err(format!("Unreconized bond type: {m}")),
                },
            });
        }

        Ok(Structure {
            atoms,
            bonds,
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone)]
pub enum Token {
    TableStart,
    Label((String, String)), // block name, attribute name
    Value(String),
    Eof,
}

impl Token {
    fn new(data: &str) -> Token {
        if data == "loop_" {
            return Token::TableStart;
        }

        if let Some(rest) = data.strip_prefix('_') {
            let dot = rest.find('.').unwrap();
            return Token::Label((rest[..dot].to_string(), rest[dot + 1..].to_string()));
        }

        Token::Value(data.to_string())
    }
}

#[derive(Default, Debug)]
struct Table {
    // keys are sorted by insertion order
    columns: IndexMap<String, Vec<Token>>,
    num_rows: usize,
}

impl Table {
    fn string(&self, column: &str, i: usize) -> Result<String, String> {
        match &self.columns[column][i] {
            Token::Value(s) => Ok(s.to_string()),
            _ => Err("Unexpected token".to_string()),
        }
    }

    fn f32(&self, column: &str, i: usize) -> Result<f32, String> {
        match &self.columns[column][i] {
            Token::Value(s) => s.parse::<f32>().map_err(|_| "Invalid number".to_string()),
            _ => Err("Unexpected token".to_string()),
        }
    }
}

#[derive(Default, Debug)]
struct DataBlock {
    tables: HashMap<String, Table>,
    start_offset: usize,
    end_offset: usize,
}

#[derive(Default)]
pub struct MMCIFLoader {
    data_blocks: HashMap<String, DataBlock>,
    mmap: Option<Mmap>,
}

impl MMCIFLoader {
    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        self.mmap = Some(unsafe { MmapOptions::new().map(&file).map_err(|e| e.to_string())? });
        self.scan_datablocks();
        Ok(())
    }

    // First pass: scan the file for offsets to data blocks
    fn scan_datablocks(&mut self) {
        let needle = "data_";
        let bytes: &[u8] = self.mmap.as_ref().unwrap();

        let mut offsets: Vec<usize> = find_iter(bytes, needle.as_bytes())
            .filter(|i| *i - 0 == 0 || bytes[i - 1] == b'\n')
            .collect();
        offsets.push(bytes.len());

        for i in 0..offsets.len() - 1 {
            let start = offsets[i] + needle.len();
            let end = offsets[i + 1];

            let name_len = memchr(b'\n', &bytes[start..]).unwrap();
            let name_slice = &bytes[start..start + name_len];
            let block_name = std::str::from_utf8(name_slice).unwrap();

            self.data_blocks.insert(
                String::from(block_name),
                DataBlock {
                    end_offset: end,
                    start_offset: start + name_len,
                    tables: HashMap::new(),
                },
            );
        }
    }

    fn next_token(i: &mut usize, bytes: &[u8], peek: bool) -> Token {
        let before = *i;
        let substring = |a: usize, b: usize| std::str::from_utf8(&bytes[a..b]).unwrap();

        while *i < bytes.len() {
            // Ignore comments
            if bytes[*i] == b'#' {
                while *i < bytes.len() && bytes[*i] != b'\n' {
                    *i += 1;
                }
                continue;
            }

            // Ignore whitespace
            if bytes[*i].is_ascii_whitespace() {
                *i += 1;
                continue;
            }

            let start = *i;

            if bytes[*i] == b'"' || bytes[*i] == b'\'' || bytes[*i] == b';' {
                // Start quote
                let quote = bytes[*i];
                *i += 1;

                while *i < bytes.len() && bytes[*i] != quote {
                    *i += 1;
                }

                *i += 1; // End quote
                let token = Token::new(substring(start + 1, *i - 1));
                if peek {
                    *i = before;
                }
                return token;
            } else {
                while *i < bytes.len() && !bytes[*i].is_ascii_whitespace() && bytes[*i] != b'#' {
                    *i += 1;
                }

                let token = Token::new(substring(start, *i));
                if peek {
                    *i = before;
                }
                return token;
            }
        }

        if peek {
            *i = before;
        }
        Token::Eof
    }

    fn parse_block(&mut self, name: Option<&str>) -> Result<(), String> {
        let key = match name {
            Some(s) => s,
            // Parse the first data block by default
            None => &self
                .data_blocks
                .keys()
                .next()
                .ok_or("No data blocks found")?
                .clone(),
        };

        let block = self
            .data_blocks
            .get_mut(key)
            .ok_or_else(|| format!("{} not found", key))?;
        let (mut i, end) = (block.start_offset, block.end_offset);

        let mut in_table = false;
        let mut prev_column = String::new();
        let mut prev_block = String::new();

        let bytes = self.mmap.as_ref().unwrap();
        while i < end {
            let token = Self::next_token(&mut i, bytes, false);

            match token {
                Token::TableStart | Token::Eof => in_table = true,
                Token::Label((block_name, column)) => {
                    prev_column = column.clone();
                    prev_block = block_name.clone();
                    block
                        .tables
                        .entry(block_name)
                        .or_default()
                        .columns
                        .insert(column, Vec::new());
                }
                Token::Value(_) => {
                    // Dictionary value
                    if !in_table {
                        let table = block.tables.entry(prev_block.clone()).or_default();
                        table.columns.get_mut(&prev_column).unwrap().push(token);
                        table.num_rows = 1;
                        continue;
                    }

                    // Table row
                    // Remember that this works because keys are sorted by their insertion order
                    let table = block.tables.entry(prev_block.clone()).or_default();
                    let keys: Vec<String> = table.columns.keys().cloned().collect();
                    table.num_rows += 1;

                    for key_idx in 0..keys.len() {
                        let t = if key_idx == 0 {
                            token.clone()
                        } else {
                            Self::next_token(&mut i, bytes, false)
                        };
                        table
                            .columns
                            .entry(keys[key_idx].clone())
                            .or_default()
                            .push(t);
                    }

                    // Is last row?
                    let next_is_value =
                        matches!(Self::next_token(&mut i, bytes, true), Token::Value(_));
                    if in_table && !next_is_value {
                        in_table = false;
                    }
                }
            }
        }

        Ok(())
    }

    fn get_table(&self, block_id: Option<&str>, table_id: &str) -> Result<&Table, String> {
        let key = match block_id {
            // In most files, there will only be one datablock,
            // so it can be used as the default datablock
            None => self
                .data_blocks
                .keys()
                .next()
                .ok_or(String::from("File contains no blocks"))?,
            Some(b_id) => b_id,
        };

        self.data_blocks[key]
            .tables
            .get(table_id)
            .ok_or(format!("{table_id} not found in {key}"))
    }
}

// Generates all (chain, transform) pairs needed to build a biological assembly
// from an mmCIF `oper_expression` and `asym_id_list`.
//
// The oper_expression describes which operations from `pdbx_struct_oper_list` to apply,
// in one of three forms:
//   - "1"         → single operation
//   - "1,2,3"     → each operation independently (one copy of chains per op)
//   - "(1,2)(3,4)"→ Cartesian product: one copy per combination (1×3, 1×4, 2×3, 2×4)
//
// Ranges like "(1-60)" are expanded into the full list of operation IDs.
// For product expressions, each combination is multiplied into a single matrix.
// Every resulting matrix is paired with every chain in `chains`.
fn generate_chain_copies(
    expression: &str,
    chains: &Vec<String>,
    transforms: &HashMap<usize, Mat4>,
) -> Vec<(String, Mat4)> {
    let is_sequence = !expression.starts_with("(");

    let groups = if is_sequence {
        // Sequence form "1,2,3": wrap each ID in its own group so the
        // Cartesian product below treats them as independent operations.
        expression
            .split(',')
            .map(|s| vec![s.parse::<usize>().unwrap()])
            .collect()
    } else {
        // Parenthesized form "(1,2)(3-5)": each (...) is one group.
        // Strip the leading "(" (+1) and stop before the closing ")".
        // Ranges like "3-5" are expanded to [3, 4, 5].
        let mut i = 0;
        let mut lists = Vec::new();

        while i < expression.len() {
            let end = i + expression[i..].find(')').unwrap();
            let group = &expression[i + 1..end];

            let is_range = group.contains("-");
            let values: Vec<usize> = group
                .split(if is_range { "-" } else { "," })
                .map(|s| s.parse::<usize>().unwrap())
                .collect();

            lists.push(if is_range {
                (values[0]..=values[1]).collect::<Vec<usize>>()
            } else {
                values
            });
            i = end + 1;
        }

        lists
    };

    // Build the Cartesian product across all groups, then fold each
    // combination into a single matrix via left-to-right multiplication.
    // A sequence "1,2,3" has groups [[1],[2],[3]] so no multiplication occurs.
    let mut chain_copies: Vec<(String, Mat4)> = Vec::new();

    for combo in groups.into_iter().multi_cartesian_product() {
        let mut result = Mat4::IDENTITY;
        for id in &combo {
            result *= transforms[id]
        }

        for chain in chains {
            chain_copies.push((chain.clone(), result));
        }
    }

    chain_copies
}

#[derive(Default, Debug)]
struct Strand {
    seq_offset: usize,
    atoms: HashMap<String, usize>, // atom id to indexes
}

// (chain_id, seq_id) to strand
type Component = HashMap<(String, String), Strand>;

impl FileLoader for MMCIFLoader {
    fn parse_file(&mut self, path: &Path) -> Result<Structure, String> {
        self.open_file(path)?;
        self.parse_block(None)?;

        let mut atoms: Vec<Atom> = Vec::new();

        // Parse atoms
        if let Ok(t) = self.get_table(None, "chem_comp_atom") {
            for i in 0..t.num_rows {
                if !t.columns.contains_key("pdbx_model_Cartn_x_ideal") {
                    break;
                }
                atoms.push(Atom {
                    chain_id: String::new(),
                    sequence_id: String::new(),
                    component_name: t.string("comp_id", i)?,
                    atom_id: t.string("atom_id", i)?,
                    element: t.string("type_symbol", i)?,
                    is_ligand: true,
                    position: glam::Vec3::new(
                        t.f32("pdbx_model_Cartn_x_ideal", i)?,
                        t.f32("pdbx_model_Cartn_y_ideal", i)?,
                        t.f32("pdbx_model_Cartn_z_ideal", i)?,
                    ),
                });
            }
        }

        if let Ok(t) = self.get_table(None, "atom_site") {
            for i in 0..t.num_rows {
                atoms.push(Atom {
                    chain_id: t.string("label_asym_id", i)?,
                    sequence_id: t.string("label_seq_id", i)?,
                    component_name: t.string("label_comp_id", i)?,
                    atom_id: t.string("label_atom_id", i)?,
                    element: t.string("type_symbol", i)?,
                    is_ligand: t.string("group_PDB", i)? == "HETATM",
                    position: glam::Vec3::new(
                        t.f32("Cartn_x", i)?,
                        t.f32("Cartn_y", i)?,
                        t.f32("Cartn_z", i)?,
                    ),
                });
            }
        }

        // Sort atoms by chain, sequence id and component name
        atoms.sort_by(|a: &Atom, b: &Atom| {
            // Ensure that sequences are sorted in ascending order, not lexographic order
            let s_a = a.sequence_id.parse::<i32>().unwrap_or(0);
            let s_b = b.sequence_id.parse::<i32>().unwrap_or(0);
            a.chain_id
                .cmp(&b.chain_id)
                .then(s_a.cmp(&s_b))
                .then(a.component_name.cmp(&b.component_name))
        });

        // Group atom indexes by component, then by chain id and sequence id
        let mut components: HashMap<String, Component> = HashMap::new();
        let mut prev_chain_id = String::new();
        let mut prev_seq_id = String::new();

        for (index, atom) in atoms.iter().enumerate() {
            let current = components
                .entry(atom.component_name.clone())
                .or_default()
                .entry((atom.chain_id.clone(), atom.sequence_id.clone()))
                .or_default();

            // sequence changed
            if atom.sequence_id.clone() != prev_seq_id || atom.chain_id.clone() != prev_chain_id {
                current.seq_offset = index;
            }
            current.atoms.insert(atom.atom_id.clone(), index);

            prev_seq_id = atom.sequence_id.clone();
            prev_chain_id = atom.chain_id.clone();
        }

        // Parse bonds
        let mut bonds: Vec<Bond> = Vec::new();
        if let Ok(t) = self.get_table(None, "chem_comp_bond") {
            for i in 0..t.num_rows {
                let component_id = t.string("comp_id", i)?;
                let src_id = t.string("atom_id_1", i)?;
                let dst_id = t.string("atom_id_2", i)?;
                let bond_type = match t.string("value_order", i)?.to_lowercase().as_str() {
                    "sing" => BondType::Single,
                    "doub" => BondType::Double,
                    "trip" => BondType::Triple,
                    x => return Err(format!("Unkonwn bond type {x}")),
                };

                for instance in components[&component_id].values() {
                    if !instance.atoms.contains_key(&src_id)
                        || !instance.atoms.contains_key(&dst_id)
                    {
                        continue;
                    }
                    bonds.push(Bond {
                        src: instance.atoms[&src_id],
                        dst: instance.atoms[&dst_id],
                        bond_type,
                    });
                }
            }
        }

        if let Ok(t) = self.get_table(None, "pdbx_struct_sheet_hbond") {
            for i in 0..t.num_rows {
                let component1 = t.string("range_1_label_comp_id", i)?;
                let chain1 = t.string("range_1_label_asym_id", i)?;
                let seq1 = t.string("range_1_label_seq_id", i)?;
                let atom1 = t.string("range_1_label_atom_id", i)?;

                let component2 = t.string("range_2_label_comp_id", i)?;
                let chain2 = t.string("range_2_label_asym_id", i)?;
                let seq2 = t.string("range_2_label_seq_id", i)?;
                let atom2 = t.string("range_2_label_atom_id", i)?;

                bonds.push(Bond {
                    src: components[&component1][&(chain1, seq1)].atoms[&atom1],
                    dst: components[&component2][&(chain2, seq2)].atoms[&atom2],
                    bond_type: BondType::HBond,
                });
            }
        }

        let mut secondary: Vec<SecondaryStructure> = Vec::new();

        // Parse helixes
        if let Ok(t) = self.get_table(None, "struct_conf") {
            for i in 0..t.num_rows {
                let comp_start = t.string("beg_label_comp_id", i)?;
                let chain_start = t.string("beg_label_asym_id", i)?;
                let seq_start = t.string("beg_label_seq_id", i)?;
                let comp_end = t.string("end_label_comp_id", i)?;
                let chain_end = t.string("end_label_asym_id", i)?;
                let seq_end = t.string("end_label_seq_id", i)?;
                secondary.push(SecondaryStructure {
                    struct_type: match t.string("conf_type_id", i)?.as_str() {
                        _ => SecondaryType::AlphaHelix, // FIXME!
                    },
                    start: components[&comp_start][&(chain_start, seq_start)].seq_offset,
                    end: components[&comp_end][&(chain_end, seq_end)].seq_offset,
                });
            }
        }

        // Parse sheets
        if let Ok(t) = self.get_table(None, "struct_sheet_range") {
            for i in 0..t.num_rows {
                let comp_start = t.string("beg_label_comp_id", i)?;
                let chain_start = t.string("beg_label_asym_id", i)?;
                let seq_start = t.string("beg_label_seq_id", i)?;
                let comp_end = t.string("end_label_comp_id", i)?;
                let chain_end = t.string("end_label_asym_id", i)?;
                let seq_end = t.string("end_label_seq_id", i)?;
                secondary.push(SecondaryStructure {
                    struct_type: SecondaryType::BetaSheet,
                    start: components[&comp_start][&(chain_start, seq_start)].seq_offset,
                    end: components[&comp_end][&(chain_end, seq_end)].seq_offset,
                });
            }
        }

        // Parse assemblies
        let mut transforms: HashMap<usize, Mat4> = HashMap::new();

        if let Ok(t) = self.get_table(None, "pdbx_struct_oper_list") {
            for i in 0..t.num_rows {
                *transforms.entry(t.f32("id", i)? as usize).or_default() = Mat4::from_cols(
                    Vec4::new(
                        t.f32("matrix[1][1]", i)?,
                        t.f32("matrix[2][1]", i)?,
                        t.f32("matrix[3][1]", i)?,
                        0.0,
                    ),
                    Vec4::new(
                        t.f32("matrix[1][2]", i)?,
                        t.f32("matrix[2][2]", i)?,
                        t.f32("matrix[3][2]", i)?,
                        0.0,
                    ),
                    Vec4::new(
                        t.f32("matrix[1][3]", i)?,
                        t.f32("matrix[2][3]", i)?,
                        t.f32("matrix[3][3]", i)?,
                        0.0,
                    ),
                    Vec4::new(
                        t.f32("vector[1]", i)?,
                        t.f32("vector[2]", i)?,
                        t.f32("vector[3]", i)?,
                        1.0,
                    ),
                );
            }
        }

        let mut chain_copies: Vec<(String, Mat4)> = Vec::new();

        if let Ok(t) = self.get_table(None, "pdbx_struct_assembly_gen") {
            for i in 0..t.num_rows {
                let chains = t
                    .string("asym_id_list", i)?
                    .split(',')
                    .map(|s| s.to_string())
                    .collect();
                chain_copies =
                    generate_chain_copies(&t.string("oper_expression", i)?, &chains, &transforms);
            }
        }

        Ok(Structure {
            atoms,
            bonds,
            secondary,
            chain_copies,
        })
    }
}
