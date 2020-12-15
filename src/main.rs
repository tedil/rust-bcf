use anyhow::Result;
// use bgzip::BGZFReader;
use counter::Counter;
use itertools::Itertools;

// use crate::parser::parse;
use crate::parser::{BcfRecords, RawBcfRecords};

use crate::record::Record;
use rust_htslib::bcf::{Read, Reader};
use std::time::Instant;

mod parser;
mod record;
mod types;

fn main() -> Result<()> {
    let path = &std::env::args().collect_vec()[1];
    // let records = BcfRecords::from_path(path);

    let mut now = Instant::now();
    let records = RawBcfRecords::from_path(path).unwrap();
    let counts: Counter<_> = records
        .map(|record| record.ref_allele().len())
        // .map(|record| record.info(b"platforms").integer().unwrap()[0])
        .collect();
    dbg!(now.elapsed());

    now = Instant::now();
    let mut htslib_reader = Reader::from_path(path)?;
    let htslib_counts: Counter<_> = htslib_reader
        .records()
        .map(|record| {
            let mut record = record.unwrap();
            // record.unpack();
            record.alleles()[0].len()
        })
        .collect();
    dbg!(now.elapsed());

    Ok(())
}
