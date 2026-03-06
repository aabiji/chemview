use std::collections::HashMap;
use std::path::PathBuf;

struct DataBlock {
    tabular_data: HashMap<String, Vec<String>>,
}

fn parse_datablock<T: Iterator>(iter: &T) {}

fn parse_key_value_block(iter: &mut impl Iterator<Item = String>) -> (HashMap<String, String>) {
    let mut pairs = HashMap::new();

    loop {
        let line = iter.next().unwrap();
        if line.starts_with("#") {
            break;
        }
        let mut split = line.split_whitespace();
        pairs.insert(
            split.next().unwrap().to_string(),
            split.next().unwrap().to_string(),
        );
    }

    pairs
}

fn parse_loop_block(iter: &mut impl Iterator<Item = String>) -> (Vec<String>, Vec<String>) {
    let mut labels: Vec<String> = Vec::new();
    let mut rows: Vec<String> = Vec::new();

    loop {
        let line = iter.next().unwrap();

        if line.starts_with("#") {
            break;
        } else if line.starts_with("_") {
            labels.push(line);
        } else {
            let cells: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
            rows.extend_from_slice(&cells);
        }
    }

    (labels, rows)
}

// IDEA: use recursive descent parsing. we should be able to unify the key/value and the tabular in
// the same way
pub fn parse() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data/mmcif/T44.cif");

    let contents = std::fs::read_to_string(path).unwrap();
    parse_datablock(&contents.lines());

    println!("Parsing mmCIF :)");
}
