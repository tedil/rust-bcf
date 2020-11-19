use std::io::{BufReader, Read};

use anyhow::Result;
// use bgzip::BGZFReader;
use counter::Counter;
use itertools::Itertools;

use crate::parser::parse;

mod parser;
mod types;

fn main() -> Result<()> {
    let path = &std::env::args().collect_vec()[1];
    // let mut reader = BGZFReader::new(std::fs::File::open(path)?);
    let mut reader = BufReader::new(std::fs::File::open(path)?);
    let mut buffer = Vec::with_capacity(5000);
    let num_bytes = reader.read_to_end(&mut buffer)?;
    dbg!(num_bytes);
    let records = parse(&buffer);
    let counts: Counter<_> = records.map(|record| record.ref_allele).collect();
    dbg!(&counts.most_common_ordered()[..10]);

    // dbg!(&records);
    Ok(())
}
