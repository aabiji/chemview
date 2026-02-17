#[derive(Debug, PartialEq)]
pub struct Compound {
    moniker: String,
    iupac_name: String,
    is_chiral: bool,
    atoms: Vec<Atom>,
    bonds: Vec<Bond>,
}

#[derive(Debug, PartialEq)]
pub struct Atom {
    x: f32,
    y: f32,
    z: f32,
    element: String,
}

#[derive(Debug, PartialEq)]
pub struct Bond {
    src_index: usize,
    dst_index: usize,
    bond_type: BondType,
    topology: BondTopology,
}

#[derive(Debug, PartialEq)]
pub enum BondType {
    Single,
    Double,
    Triple,
    Aromatic,
    SingleOrDouble,
    SingleOrAromatic,
    DoubleOrAromatic,
    Any,
}

#[derive(Debug, PartialEq)]
pub enum BondTopology {
    RingOrChain,
    Ring,
    Chain,
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

// Parse chemical data from a V2000 SDF file
pub fn parse_compound(contents: &str) -> Result<Compound, String> {
    let lines = split(contents, '\n', false);
    let count_line = parse::<String>(&lines, 3)?;
    let count_fields = split(&count_line, ' ', true);
    let num_atoms = parse::<usize>(&count_fields, 0)?;
    let num_bonds = parse::<usize>(&count_fields, 1)?;

    let mut compound = Compound {
        moniker: String::new(),
        iupac_name: String::new(),
        is_chiral: parse::<u8>(&count_fields, 3)? == 1,
        atoms: Vec::new(),
        bonds: Vec::new(),
    };

    for i in 0..num_atoms {
        let line = parse::<String>(&lines, 4 + i)?;
        let fields = split(&line, ' ', true);
        compound.atoms.push(Atom {
            x: parse::<f32>(&fields, 0)?,
            y: parse::<f32>(&fields, 1)?,
            z: parse::<f32>(&fields, 2)?,
            element: parse::<String>(&fields, 3)?,
        });
    }

    for i in 0..num_bonds {
        let line = parse::<String>(&lines, 4 + num_atoms + i)?;
        let fields = split(&line, ' ', true);
        compound.bonds.push(Bond {
            src_index: parse::<usize>(&fields, 0)? - 1,
            dst_index: parse::<usize>(&fields, 1)? - 1,
            bond_type: match parse::<usize>(&fields, 2)? {
                1 => BondType::Single,
                2 => BondType::Double,
                3 => BondType::Triple,
                4 => BondType::Aromatic,
                5 => BondType::SingleOrDouble,
                6 => BondType::SingleOrAromatic,
                7 => BondType::DoubleOrAromatic,
                _ => BondType::Any,
            },
            topology: match parse::<usize>(&fields, 5)? {
                1 => BondTopology::Ring,
                2 => BondTopology::Chain,
                _ => BondTopology::RingOrChain,
            },
        });
    }

    for i in (5 + num_atoms + num_bonds)..lines.len() {
        if lines[i] == "> <PUBCHEM_IUPAC_NAME>" {
            compound.iupac_name = lines[i + 1].to_string();
        }
        if lines[i] == "> <PUBCHEM_IUPAC_TRADITIONAL_NAME>" {
            compound.moniker = lines[i + 1].to_string();
        }
    }

    Ok(compound)
}

mod tests {
    #[test]
    fn test_parser() {
        use crate::parser::{Atom, Bond, BondTopology, BondType, Compound, parse_compound};
        let content = "783
                -OEChem-02172615072D

            2  1  0     0  0  0  0  0  0999 V2000
            2.0000    0.0000    0.0000 H   0  0  0  0  0  0  0  0  0  0  0  0
            3.0000    0.0000    0.0000 H   0  0  0  0  0  0  0  0  0  0  0  0
            1  2  1  0  0  0  0
            M  END
        ";
        let expected = Ok(Compound {
            moniker: String::new(),
            iupac_name: String::new(),
            is_chiral: false,
            atoms: vec![
                Atom {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0,
                    element: "H".to_string(),
                },
                Atom {
                    x: 3.0,
                    y: 0.0,
                    z: 0.0,
                    element: "H".to_string(),
                },
            ],
            bonds: vec![Bond {
                src_index: 0,
                dst_index: 1,
                bond_type: BondType::Single,
                topology: BondTopology::RingOrChain,
            }],
        });
        assert!(parse_compound(content) == expected);
    }
}
