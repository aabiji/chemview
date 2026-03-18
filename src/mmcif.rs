use glam::Vec3;
use memchr::memchr;
use memchr::memmem::find_iter;
use memmap::{Mmap, MmapOptions};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::PathBuf;

use crate::pipeline::CompoundPipeline;
use crate::shape::CompoundMeshInfo;

#[derive(Debug)]
enum Token {
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

type Table = BTreeMap<String, Vec<Token>>;

#[derive(Debug)]
pub struct DataBlock {
    pub tables: HashMap<String, Table>,
    start_offset: usize,
    end_offset: usize,
}

#[derive(Debug)]
pub struct Parser {
    pub data_blocks: HashMap<String, DataBlock>,
    mmap: Mmap,
}

impl Parser {
    pub fn new(filename: &str) -> Result<Self, String> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("data");
        path.push("mmcif");
        path.push(filename);

        let file = File::open(path).map_err(|e| e.to_string())?;
        let mmap = unsafe { MmapOptions::new().map(&file).map_err(|e| e.to_string())? };

        let mut parser = Self {
            mmap: mmap,
            data_blocks: HashMap::new(),
        };
        parser.find_datablocks();
        Ok(parser)
    }

    // First pass: scan the file for offsets to data blocks
    fn find_datablocks(&mut self) {
        let needle = "data_";
        let bytes: &[u8] = &self.mmap;

        let mut offsets: Vec<usize> = find_iter(&bytes, needle.as_bytes()).collect();
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
                let token = Token::new(&substring(start + 1, *i - 1));
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

    pub fn parse_block(&mut self, name: &str) -> Result<(), String> {
        let block = self
            .data_blocks
            .get_mut(name)
            .ok_or_else(|| format!("{} not found", name))
            .unwrap();
        let (mut i, end) = (block.start_offset, block.end_offset);

        let mut in_table = false;
        let mut prev_column = String::new();
        let mut prev_block = String::new();

        let bytes = self.mmap.to_vec();
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
                        .insert(column, Vec::new());
                }
                Token::Value(_) => {
                    // Dictionary value
                    if !in_table {
                        block
                            .tables
                            .entry(prev_block.clone())
                            .or_default()
                            .get_mut(&prev_column)
                            .unwrap()
                            .push(token);
                        continue;
                    }

                    // Table row
                    // Remeber that this works because keys are sorted by their insertion order
                    let keys: Vec<String> = block.tables[&prev_block].keys().cloned().collect();
                    for key in keys {
                        block
                            .tables
                            .get_mut(&prev_block.clone())
                            .unwrap()
                            .entry(key)
                            .or_default()
                            .push(Self::next_token(&mut i, &bytes, false));
                    }

                    // Last row?
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
}

struct MMCIFLoader {
    parser: Parser,
}

impl CompoundPipeline for MMCIFLoader {
    fn init() -> Result<Self, String> {
        Ok(())
    }

    fn parse_file(&mut self, path: &PathBuf) -> Result<(), String> {
        Ok(())
    }

    fn compute_mesh_info(&mut self, camera_front: Vec3, use_waal_radius: bool) -> CompoundMeshInfo {
    }
}
