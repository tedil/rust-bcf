mod parser;
mod types;

use crate::parser::parse;
use anyhow::Result;
// use bgzip::BGZFReader;
use std::io::{BufRead, BufReader, Read};

const BCF_MAJOR_VERSION: u8 = 2;
const BCF_MINOR_VERSION: u8 = 2;

fn main() -> Result<()> {
    let path = "/home/till/projects/varlociraptor/bug/patient31.obs.uncompressed.bcf";
    // let mut reader = BGZFReader::new(std::fs::File::open(path)?);
    let mut reader = BufReader::new(std::fs::File::open(path)?);
    {
        let mut buffer = Vec::with_capacity(5000);
        let num_bytes = reader.read_to_end(&mut buffer)?;
        dbg!(num_bytes);
        let record = parse(&buffer)?;
        // dbg!(&record);
    }
    Ok(())
}
