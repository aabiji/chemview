use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
enum Token {
    TableStart,
    Label((String, String)), // block name, attribute name
    Value(String),
    DataTag,
    Eof,
}

impl Token {
    fn new(data: &str) -> Token {
        if data == "loop_" {
            return Token::TableStart;
        } else if data.starts_with("data") {
            return Token::DataTag;
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

    fn num(&self) -> Result<f32, &'static str> {
        match self {
            Token::Value(s) => s.parse::<f32>().map_err(|_| "Invalid number"),
            _ => Err("Unexpected token"),
        }
    }
}

#[derive(Debug)]
pub enum SecondaryStructureType {
    Helix,
    BetaSheet,
}

#[derive(Debug)]
pub struct SecondaryStructure {
    struct_type: SecondaryStructureType,
    chain_beg_id: String,
    chain_end_id: String,
    sequence_beg_id: String,
    sequence_end_id: String,
}

#[derive(Debug)]
pub struct Transformation {
    chains: Vec<u32>, // id of the chains to apply the transform to
    matrix: glam::Mat4,
}

#[derive(Debug)]
pub struct Atom {
    component_id: String,
    chain_id: String,
    sequence_id: String,
    atom_id: String,
    element: String,
    position: glam::Vec3,
}

#[derive(Debug)]
pub struct Bond {
    component_id: String,
    src_atom_id: String,
    dst_atom_id: String,
    multiplicity: usize,
}

#[derive(Debug)]
pub struct Structure {
    pub atoms: Vec<Atom>,
    pub assemblies: HashMap<String, Transformation>,
    pub structures: Vec<SecondaryStructure>,
    pub ligand_bonds: Vec<Bond>,
    i: usize,
}

impl Structure {
    pub fn new(path: &PathBuf) -> Result<Structure, String> {
        let mut s = Structure {
            atoms: Vec::new(),
            ligand_bonds: Vec::new(),
            assemblies: HashMap::new(),
            structures: Vec::new(),
            i: 0,
        };
        s.parse(&std::fs::read_to_string(path).unwrap())?;
        Ok(s)
    }

    fn next_token(&mut self, content: &str, peek: bool) -> Token {
        let bytes = content.as_bytes();
        let before = self.i;

        while self.i < bytes.len() {
            // Ignore comments
            if bytes[self.i] == b'#' {
                while self.i < bytes.len() && bytes[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }

            // Ignore whitespace
            if bytes[self.i].is_ascii_whitespace() {
                self.i += 1;
                continue;
            }

            let start = self.i;

            if bytes[self.i] == b'"' || bytes[self.i] == b'\'' || bytes[self.i] == b';' {
                // Start quote
                let quote = bytes[self.i];
                self.i += 1;

                while self.i < bytes.len() && bytes[self.i] != quote {
                    self.i += 1;
                }

                self.i += 1; // End quote
                let token = Token::new(&content[start + 1..self.i - 1]);
                if peek {
                    self.i = before;
                }
                return token;
            } else {
                while self.i < bytes.len()
                    && !bytes[self.i].is_ascii_whitespace()
                    && bytes[self.i] != b'#'
                {
                    self.i += 1;
                }

                let token = Token::new(&content[start..self.i]);
                if peek {
                    self.i = before;
                }
                return token;
            }
        }

        if peek {
            self.i = before;
        }
        Token::Eof
    }

    fn parse_block(
        &mut self,
        block_name: &str,
        columns: &HashMap<String, usize>,
        values: &[Token],
    ) -> Result<(), String> {
        let get = |name: &str| -> Result<&Token, String> {
            let index = columns
                .get(name)
                .ok_or_else(|| format!("{block_name}.{name} not found {:?}", columns))?;
            Ok(&values[*index])
        };

        match block_name {
            "atom_site" => {
                self.atoms.push(Atom {
                    component_id: String::new(),
                    atom_id: get("atom_id")?.string()?,
                    chain_id: get("label_asym_id")?.string()?,
                    sequence_id: get("label_seq_id")?.string()?,
                    element: get("type_symbol")?.string()?,
                    position: glam::Vec3::new(
                        get("Cartn_x")?.num()?,
                        get("Cartn_y")?.num()?,
                        get("Cartn_z")?.num()?,
                    ),
                });
            }
            "chem_comp_atom" => {
                self.atoms.push(Atom {
                    sequence_id: String::new(),
                    chain_id: String::new(),
                    atom_id: get("atom_id")?.string()?,
                    component_id: get("comp_id")?.string()?,
                    element: get("type_symbol")?.string()?,
                    position: glam::Vec3::new(
                        get("pdbx_model_Cartn_x_ideal")?.num()?,
                        get("pdbx_model_Cartn_y_ideal")?.num()?,
                        get("pdbx_model_Cartn_z_ideal")?.num()?,
                    ),
                });
            }
            "chem_comp_bond" => {
                self.ligand_bonds.push(Bond {
                    component_id: get("comp_id")?.string()?,
                    src_atom_id: get("atom_id_1")?.string()?,
                    dst_atom_id: get("atom_id_2")?.string()?,
                    multiplicity: match get("value_order")?.string()?.as_str() {
                        "SING" => 1,
                        "DOUB" => 2,
                        "TRIP" => 3,
                        _ => todo!(),
                    },
                });
            }
            "struct_conf" | "struct_sheet_range" => {
                self.structures.push(SecondaryStructure {
                    struct_type: match get("conf_type_id")?.string()?.as_str() {
                        "STRN" => SecondaryStructureType::BetaSheet,
                        "HELX_P" => SecondaryStructureType::Helix,
                        _ => todo!(),
                    },
                    chain_beg_id: get("beg_label_asym_id")?.string()?,
                    chain_end_id: get("end_label_asym_id")?.string()?,
                    sequence_beg_id: get("end_label_seq_id")?.string()?,
                    sequence_end_id: get("end_label_seq_id")?.string()?,
                });
            }
            "pdbx_struct_assembly" => todo!(),
            "pdbx_struct_assembly_gen" => todo!(),
            "pdbx_struct_sheet_hbond" => todo!(),
            "pdbx_struct_oper_list" => todo!(),
            _ => {}
        }

        Ok(())
    }

    fn parse(&mut self, content: &str) -> Result<(), String> {
        let mut in_table = false;
        let mut block_name = String::new();

        let mut columns: HashMap<String, usize> = HashMap::new();
        let mut values: Vec<Token> = Vec::new();

        while self.i < content.len() {
            let token = self.next_token(content, false);

            match token {
                Token::DataTag => {}
                Token::TableStart | Token::Eof => {
                    // End of dictionary
                    if !columns.is_empty() {
                        self.parse_block(&block_name, &columns, &values)?;
                    }

                    in_table = true;
                    columns.clear();
                    values.clear();
                }
                Token::Label((block, column)) => {
                    block_name = block.clone();
                    columns.insert(column, columns.len());
                }
                Token::Value(_) => {
                    // Dictionary value
                    if !in_table {
                        values.push(token);
                        continue;
                    }

                    // Tablw row
                    let mut row = vec![token];
                    for _ in 0..columns.len() - 1 {
                        row.push(self.next_token(content, false));
                    }
                    self.parse_block(&block_name, &columns, &row)?;

                    // Last row?
                    let next_is_value = matches!(self.next_token(content, true), Token::Value(_));
                    if in_table && !next_is_value {
                        in_table = false;
                        columns.clear();
                    }
                }
            }
        }

        Ok(())
    }
}
