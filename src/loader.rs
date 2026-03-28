use glam::Vec3;
use indexmap::IndexMap;
use memchr::memchr;
use memchr::memmem::find_iter;
use memmap::{Mmap, MmapOptions};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use crate::tessellate::{Atom, Bond, BondType, Chain, Molecule, Structure};

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

    fn string(&self) -> Result<String, String> {
        match self {
            Token::Value(s) => Ok(s.to_string()),
            _ => Err("Unexpected token".to_string()),
        }
    }

    fn f32(&self) -> Result<f32, String> {
        match self {
            Token::Value(s) => s.parse::<f32>().map_err(|_| "Invalid number".to_string()),
            _ => Err("Unexpected token".to_string()),
        }
    }
}

#[derive(Default, Debug)]
struct Table {
    // keys are sorted by insertion order
    columns: IndexMap<String, Vec<Token>>,
    num_rows: usize,
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
            .filter(|i| i - 0 == 0 || bytes[i - 1] == b'\n')
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
struct ComponentInstance {
    start: usize,
    length: usize,
    atoms: HashMap<String, usize>,
}

impl FileLoader for MMCIFLoader {
    fn parse_file(&mut self, path: &Path) -> Result<Structure, String> {
        self.open_file(path)?;
        self.parse_block(None)?;

        let mut atoms: Vec<Atom> = Vec::new();
        let mut components: HashMap<String, Vec<ComponentInstance>> = HashMap::new();

        // Parse atoms
        if let Ok(t) = self.get_table(None, "chem_comp_atom") {
            for i in 0..t.num_rows {
                if !t.columns.contains_key("pdbx_model_Cartn_x_ideal") {
                    break;
                }
                atoms.push(Atom {
                    chain_id: String::new(),
                    sequence_id: String::new(),
                    component_name: t.columns["comp_id"][i].string()?,
                    atom_id: t.columns["atom_id"][i].string()?,
                    element: t.columns["type_symbol"][i].string()?,
                    is_ligand: true,
                    position: glam::Vec3::new(
                        t.columns["pdbx_model_Cartn_x_ideal"][i].f32()?,
                        t.columns["pdbx_model_Cartn_y_ideal"][i].f32()?,
                        t.columns["pdbx_model_Cartn_z_ideal"][i].f32()?,
                    ),
                });
            }
        }

        if let Ok(t) = self.get_table(None, "atom_site") {
            for i in 0..t.num_rows {
                atoms.push(Atom {
                    chain_id: t.columns["label_asym_id"][i].string()?,
                    sequence_id: t.columns["label_seq_id"][i].string()?,
                    component_name: t.columns["label_comp_id"][i].string()?,
                    atom_id: t.columns["label_atom_id"][i].string()?,
                    element: t.columns["type_symbol"][i].string()?,
                    is_ligand: t.columns["group_PDB"][i].string()? == "HETATM",
                    position: glam::Vec3::new(
                        t.columns["Cartn_x"][i].f32()?,
                        t.columns["Cartn_y"][i].f32()?,
                        t.columns["Cartn_z"][i].f32()?,
                    ),
                });
            }
        }

        // Sort atoms by chain, sequence id and component name
        atoms.sort_by(|a: &Atom, b: &Atom| {
            a.chain_id
                .cmp(&b.chain_id)
                .then(a.sequence_id.cmp(&b.sequence_id))
                .then(a.component_name.cmp(&b.component_name))
        });

        // Map each component to each of its instances, while
        // mapping atom ids to indexes in the atom list
        let mut prev_comp = String::new();
        for (index, atom) in atoms.iter().enumerate() {
            let n = atom.component_name.clone();

            if n != prev_comp {
                // new component...
                let c = ComponentInstance {
                    start: index,
                    length: 0,
                    atoms: HashMap::new(),
                };
                components.entry(n.clone()).or_default().push(c);
            }

            let current = components.entry(n.clone()).or_default().last_mut().unwrap();
            current.atoms.insert(atom.atom_id.clone(), index);
            current.length += 1;

            prev_comp = n;
        }

        // Parse bonds
        let mut bonds: Vec<Bond> = Vec::new();
        if let Ok(t) = self.get_table(None, "chem_comp_bond") {
            for i in 0..t.num_rows {
                let component_id = t.columns["comp_id"][i].string()?;
                let src_id = t.columns["atom_id_1"][i].string()?;
                let dst_id = t.columns["atom_id_2"][i].string()?;
                let bond_type = match t.columns["value_order"][i]
                    .string()?
                    .to_lowercase()
                    .as_str()
                {
                    "sing" => BondType::Single,
                    "doub" => BondType::Double,
                    "trip" => BondType::Triple,
                    x => return Err(format!("Unkonwn bond type {x}")),
                };

                for instance in &components[&component_id] {
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

        Ok(Structure {
            atoms,
            bonds,
            ..Default::default()
        })
    }
}
