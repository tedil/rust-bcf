use std::time::Instant;

use anyhow::Result;
use counter::Counter;
use itertools::Itertools;
use rust_htslib::bcf::{Read, Reader};

// use crate::parser::parse;
use rust_bcf::reader::RawBcfRecords;
use rust_bcf::record::Record;

fn main() -> Result<()> {
    let path = &std::env::args().collect_vec()[1];

    let mut now = Instant::now();
    let records = RawBcfRecords::from_path(path).unwrap();
    let _counts: Counter<_> = records
        .map(|record| record.chrom().to_owned())
        // .map(|record| record.info(b"platforms").integer().unwrap()[0])
        .collect();
    dbg!(now.elapsed());

    now = Instant::now();
    let mut htslib_reader = Reader::from_path(path)?;
    let _htslib_counts: Counter<_> = htslib_reader
        .records()
        .map(|record| {
            let record = record.unwrap();
            // record.unpack();
            // record.alleles()[0].len()
            record.rid().unwrap_or(0)
        })
        .collect();
    dbg!(now.elapsed());

    Ok(())
}
