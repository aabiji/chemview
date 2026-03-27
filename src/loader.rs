use glam::Vec3;
use indexmap::IndexMap;
use memchr::memchr;
use memchr::memmem::find_iter;
use memmap::{Mmap, MmapOptions};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::Path;

use crate::tessellate::{Atom, Bond, BondType, Molecule, Structure};

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
            ligands: vec![Molecule {
                atoms,
                bonds,
                name: String::new(),
            }],
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
struct ParseMaps {
    // Map IDS to indexes
    atoms: HashMap<(String, String, String), usize>,
    residues: HashMap<(String, String), usize>,
    chains: HashMap<String, usize>,
    // map residue to parent chains
    residue_parents: HashMap<String, HashSet<String>>,
}

impl ParseMaps {
    fn get_ligand_index(&mut self, ligand_id: &str, structure: &mut Structure) -> usize {
        let index = if !self.chains.contains_key(ligand_id) {
            let index = structure.ligands.len();
            self.chains.insert(ligand_id.to_string(), index);
            structure.ligands.push(Molecule::default());
            index
        } else {
            self.chains[ligand_id]
        };
        index
    }

    fn get_chain_indexes(
        &mut self,
        chain_id: &str,
        residue_id: &str,
        structure: &mut Structure,
    ) -> (usize, usize) {
        let chain_index = if !self.chains.contains_key(chain_id) {
            let index = structure.chains.len();
            self.chains.insert(chain_id.to_string(), index);
            structure.chains.push(Vec::new());
            index
        } else {
            self.chains[chain_id]
        };

        let k = (chain_id.to_string(), residue_id.to_string());
        let residue_index = if !self.residues.contains_key(&k) {
            let index = structure.chains[chain_index].len();
            self.residues.insert(k, index);
            structure.chains[chain_index].push(Molecule::default());
            index
        } else {
            self.residues[&k]
        };

        (chain_index, residue_index)
    }

    fn get_atom(&self, chain_id: &str, residue_id: &str, atom_id: &str) -> Result<usize, String> {
        self.atoms
            .get(&(
                chain_id.to_string(),
                residue_id.to_string(),
                atom_id.to_string(),
            ))
            .copied()
            .ok_or_else(|| String::from("Bond pair not found"))
    }

    fn is_residue(&self, id: &String) -> bool {
        self.residue_parents.contains_key(id)
    }
}

#[derive(Default)]
pub struct MMCIFLoader {
    data_blocks: HashMap<String, DataBlock>,
    mmap: Option<Mmap>,
    maps: ParseMaps,
}

impl MMCIFLoader {
    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        self.mmap = Some(unsafe { MmapOptions::new().map(&file).map_err(|e| e.to_string())? });
        self.scan_datablocks();
        Ok(())
    }

    pub fn get_sequence_bonds(&mut self, data_block: &str) -> Result<Vec<Bond>, String> {
        if self
            .data_blocks
            .get(data_block)
            .ok_or_else(|| format!("{data_block} not found"))?
            .tables
            .is_empty()
        {
            self.parse_block(Some(data_block))?;
        }
        let t = self.get_table(Some(data_block), "chem_comp_bond")?;
        Ok(Vec::new())
        /*
        (0..t.num_rows)
            .map(|i| -> Result<Bond, String> {
                let id = t.columns["comp_id"][i].string()?;
                Ok(Bond {
                    src: AtomKey::from_ligand(id.clone(), t.columns["atom_id_1"][i].string()?),
                    dst: AtomKey::from_ligand(id.clone(), t.columns["atom_id_2"][i].string()?),
                    multiplicity: match t.columns["value_order"][i].string()?.as_str() {
                        "DOUB" => 2,
                        "TRIP" => 3,
                        "SING" => 1,
                        value => return Err(format!("Unimplemented bond type {value}")),
                    },
                })
            })
            .collect()
            */
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

    fn parse_atoms(&mut self, structure: &mut Structure) -> Result<(), String> {
        let num_rows = { self.get_table(None, "chem_comp_atom")?.num_rows };
        for i in 0..num_rows {
            let (ligand_id, atom_id, atom) = {
                let t = self.get_table(None, "chem_comp_atom")?;
                let ligand_id = t.columns["comp_id"][i].string()?;
                let atom_id = t.columns["atom_id"][i].string()?;

                let atom = Atom {
                    element: t.columns["type_symbol"][i].string()?,
                    position: glam::Vec3::new(
                        t.columns["pdbx_model_Cartn_x_ideal"][i].f32()?,
                        t.columns["pdbx_model_Cartn_y_ideal"][i].f32()?,
                        t.columns["pdbx_model_Cartn_z_ideal"][i].f32()?,
                    ),
                };
                (ligand_id, atom_id, atom)
            };

            let index = self.maps.get_ligand_index(&ligand_id, structure);
            let atom_index = structure.ligands[index].atoms.len();
            self.maps
                .atoms
                .insert((ligand_id.clone(), String::new(), atom_id), atom_index);

            structure.ligands[index].atoms.push(atom);
        }

        let num_rows = { self.get_table(None, "atom_site")?.num_rows };
        for i in 0..num_rows {
            let (chain_id, residue_name, residue_id, atom_id, atom) = {
                let t = self.get_table(None, "atom_site")?;

                let chain_id = t.columns["label_asym_id"][i].string()?;
                let residue_name = t.columns["label_comp_id"][i].string()?;
                let residue_id = t.columns["label_seq_id"][i].string()?;
                let atom_id = t.columns["label_atom_id"][i].string()?;

                let atom = Atom {
                    element: t.columns["type_symbol"][i].string()?,
                    position: glam::Vec3::new(
                        t.columns["Cartn_x"][i].f32()?,
                        t.columns["Cartn_y"][i].f32()?,
                        t.columns["Cartn_z"][i].f32()?,
                    ),
                };
                (chain_id, residue_name, residue_id, atom_id, atom)
            };

            let (chain_index, residue_index) =
                self.maps
                    .get_chain_indexes(&chain_id, &residue_id, structure);
            let atom_index = structure.chains[chain_index][residue_index].atoms.len();
            self.maps
                .atoms
                .insert((chain_id.clone(), residue_id.clone(), atom_id), atom_index);

            let residue = &mut structure.chains[chain_index][residue_index];
            residue.name = residue_name.clone();
            residue.atoms.push(atom);
            self.maps
                .residue_parents
                .entry(residue_id)
                .or_default()
                .insert(chain_id);
        }

        Ok(())
    }
}

impl FileLoader for MMCIFLoader {
    fn parse_file(&mut self, path: &Path) -> Result<Structure, String> {
        let mut structure: Structure = Structure::default();

        self.open_file(path)?;
        self.parse_block(None)?;
        self.parse_atoms(&mut structure)?;

        let num_rows = { self.get_table(None, "chem_comp_bond")?.num_rows };
        for i in 0..num_rows {
            let (id, src_id, dst_id, bond_type_str) = {
                let t = self.get_table(None, "chem_comp_bond")?;
                let id = t.columns["comp_id"][i].string()?;
                let src_id = t.columns["atom_id_1"][i].string()?;
                let dst_id = t.columns["atom_id_2"][i].string()?;
                let bond_type_str = t.columns["value_order"][i].string()?.to_lowercase();
                (id, src_id, dst_id, bond_type_str)
            };

            if self.maps.is_residue(&id) {
                let chains = { self.maps.residue_parents[&id].clone() };

                for chain_id in chains {
                    let (chain_index, residue_index) =
                        self.maps.get_chain_indexes(&chain_id, &id, &mut structure);
                    structure.chains[chain_index][residue_index]
                        .bonds
                        .push(Bond {
                            src: self.maps.get_atom(&chain_id, &id, &src_id)?,
                            dst: self.maps.get_atom(&chain_id, &id, &dst_id)?,
                            bond_type: match bond_type_str.as_str() {
                                "doub" => BondType::Double,
                                "trip" => BondType::Triple,
                                "sing" => BondType::Single,
                                value => return Err(format!("Unimplemented bond type {value}")),
                            },
                        });
                }
            } else {
                let index = self.maps.get_ligand_index(&id, &mut structure);
                structure.ligands[index].bonds.push(Bond {
                    src: self.maps.get_atom(&id, &String::new(), &src_id)?,
                    dst: self.maps.get_atom(&id, &String::new(), &dst_id)?,
                    bond_type: match bond_type_str.as_str() {
                        "doub" => BondType::Double,
                        "trip" => BondType::Triple,
                        "sing" => BondType::Single,
                        value => return Err(format!("Unimplemented bond type {value}")),
                    },
                });
            }
        }

        Ok(structure)
    }
}
