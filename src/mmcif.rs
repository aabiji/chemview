use glam::Vec3;
use indexmap::IndexMap;
use memchr::memchr;
use memchr::memmem::find_iter;
use memmap::{Mmap, MmapOptions};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use crate::mesh::CompoundMeshInfo;
use crate::pipeline::{CompoundPipeline, ViewType};

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

    fn string(&self) -> Result<String, &'static str> {
        match self {
            Token::Value(s) => Ok(s.to_string()),
            _ => Err("Unexpected token"),
        }
    }

    fn f32(&self) -> Result<f32, &'static str> {
        match self {
            Token::Value(s) => s.parse::<f32>().map_err(|_| "Invalid number"),
            _ => Err("Unexpected token"),
        }
    }
}

#[derive(Default, Debug)]
struct Table {
    columns: IndexMap<String, Vec<Token>>,
    num_rows: usize,
}

struct DataBlock {
    tables: HashMap<String, Table>,
    start_offset: usize,
    end_offset: usize,
}

pub struct Parser {
    data_blocks: HashMap<String, DataBlock>,
    mmap: Option<Mmap>,
}

impl Parser {
    pub fn default() -> Self {
        Self {
            mmap: None,
            data_blocks: HashMap::new(),
        }
    }

    pub fn new(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let mmap = unsafe { MmapOptions::new().map(&file).map_err(|e| e.to_string())? };

        let mut parser = Self {
            mmap: Some(mmap),
            data_blocks: HashMap::new(),
        };
        parser.find_datablocks();
        Ok(parser)
    }

    // First pass: scan the file for offsets to data blocks
    fn find_datablocks(&mut self) {
        let needle = "data_";
        let bytes: &[u8] = self.mmap.as_ref().unwrap();

        let mut offsets: Vec<usize> = find_iter(bytes, needle.as_bytes()).collect();
        offsets.push(bytes.len());

        for i in (0..offsets.len()).step_by(2) {
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

    pub fn parse_block(&mut self, name: Option<&str>) -> Result<(), String> {
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

        let bytes = self.mmap.as_ref().unwrap().to_vec();
        while i < end {
            let token = Self::next_token(&mut i, &bytes, false);

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
                        table.num_rows += 1;
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
                            Self::next_token(&mut i, &bytes, false)
                        };
                        table
                            .columns
                            .entry(keys[key_idx].clone())
                            .or_default()
                            .push(t);
                    }

                    // Is last row?
                    let next_is_value =
                        matches!(Self::next_token(&mut i, &bytes, true), Token::Value(_));
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

#[derive(Debug)]
struct Bond {
    src_id: String,
    dst_id: String,
    multiplicity: usize,
}

#[derive(Debug)]
struct Atom {
    atom_id: String,
    element: String,
    position: Vec3,
}

struct Chain {
    residues: HashMap<String, Vec<Atom>>, // Sequence id to residue atoms
}

#[derive(Default, Debug)]
struct Ligand {
    atoms: Vec<Atom>,
    bonds: Vec<Bond>,
}

pub struct MMCIFLoader {
    parser: Parser,
    chains: HashMap<String, Chain>,   // Chain ID to chain
    ligands: HashMap<String, Ligand>, // Ligand ID to ligand
}

impl MMCIFLoader {
    pub fn init() -> Result<Self, String> {
        // TODO: load the CCD here
        Ok(Self {
            parser: Parser::default(),
            chains: HashMap::new(),
            ligands: HashMap::new(),
        })
    }
}

impl CompoundPipeline for MMCIFLoader {
    fn parse_file(&mut self, path: &Path) -> Result<(), String> {
        self.parser = Parser::new(path)?;

        self.parser.parse_block(None)?;
        let t1 = self.parser.get_table(None, "chem_comp_bond")?;
        let t2 = self.parser.get_table(None, "chem_comp_atom")?;

        for i in 0..t1.num_rows {
            let id = t1.columns["comp_id"][i].string()?;
            let ligand = self.ligands.entry(id).or_default();

            ligand.bonds.push(Bond {
                src_id: t1.columns["atom_id_1"][i].string()?,
                dst_id: t1.columns["atom_id_2"][i].string()?,
                multiplicity: match t1.columns["value_order"][i].string()?.as_str() {
                    "DOUB" => 2,
                    "TRIP" => 3,
                    "SING" => 1,
                    value => return Err(format!("Unimplemented bond type {value}")),
                },
            });
        }

        for i in 0..t2.num_rows {
            let id = t2.columns["comp_id"][i].string()?;
            let ligand = self.ligands.entry(id).or_default();

            ligand.atoms.push(Atom {
                // TODO: what about pdbx_component_atom_id
                atom_id: t2.columns["atom_id"][i].string()?,
                // TODO: what about pdbx_component_comp_id
                element: t2.columns["type_symbol"][i].string()?,
                position: glam::Vec3::new(
                    t2.columns["pdbx_model_Cartn_x_ideal"][i].f32()?,
                    t2.columns["pdbx_model_Cartn_y_ideal"][i].f32()?,
                    t2.columns["pdbx_model_Cartn_z_ideal"][i].f32()?,
                ),
            });
        }

        dbg!(&self.ligands);

        Ok(())
    }

    fn compute_mesh_info(&mut self, camera_front: Vec3, view: &ViewType) -> CompoundMeshInfo {
        todo!("Generate spheres and cylinders");
    }
}
