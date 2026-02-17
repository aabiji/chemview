use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;

mod parser;

fn main() -> io::Result<()> {
    let src_dir = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(src_dir).join("data/h2.sdf");
    let mut file = File::open(path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    match parser::parse_compound(&contents) {
        Ok(compound) => println!("{:?}", compound),
        Err(err) => println!("ERROR: {}", err),
    }

    Ok(())
}
