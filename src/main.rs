mod parser;
mod types;

use crate::parser::parse;
use anyhow::Result;
// use bgzip::BGZFReader;
use itertools::Itertools;
use std::io::{BufRead, BufReader, Read};

const BCF_MAJOR_VERSION: u8 = 2;
const BCF_MINOR_VERSION: u8 = 2;

fn main() -> Result<()> {
    let path = &std::env::args().collect_vec()[1];
    // let mut reader = BGZFReader::new(std::fs::File::open(path)?);
    let mut reader = BufReader::new(std::fs::File::open(path)?);
    {
        let mut buffer = Vec::with_capacity(5000);
        let num_bytes = reader.read_to_end(&mut buffer)?;
        dbg!(num_bytes);
        let records = parse(&buffer)?;
        // dbg!(&records);
    }
    Ok(())
}
