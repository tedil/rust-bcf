use std::time::Instant;

use anyhow::Result;
use counter::Counter;
use itertools::Itertools;
use rust_htslib::bcf::{Read, Reader};

// use crate::parser::parse;
use rust_bcf::reader::RawBcfRecords;
use rust_bcf::record::Record;
use rust_bcf::types::TypedVec;
use std::io::BufRead;

fn main() -> Result<()> {
    let path = &std::env::args().collect_vec()[1];

    let mut now = Instant::now();
    let records = RawBcfRecords::from_path(path).unwrap();
    let _counts: Counter<_> = records
        // .map(|record| record.chrom().to_owned())
        .map(|record| {
            record
                .info(b"callsets")
                .map(|v| v.integer()[0])
                .unwrap_or(0)
        })
        // .map(|mut record| {
        //     String::from_utf8(
        //         record
        //             .info(b"ANN")
        //             .map(|v| v.string())
        //             .unwrap_or(vec![b""])[0]
        //             .split(|c| *c == b'|')
        //             .next()
        //             .unwrap()
        //             .to_vec(),
        //     )
        //     .unwrap()
        // })
        .collect();
    dbg!(now.elapsed());
    dbg!(&_counts.most_common_ordered()[..5]);

    now = Instant::now();
    let mut htslib_reader = Reader::from_path(path)?;
    let _htslib_counts: Counter<_> = htslib_reader
        .records()
        .map(|record| {
            let record = record.unwrap();
            // record.unpack();
            // record.alleles()[0].len()
            // record.rid().unwrap_or(0)
            // if let Ok(Some(v)) = record.info(b"ANN").string() {
            //     String::from_utf8(v[0].split(|c| *c == b'|').next().unwrap().to_vec()).unwrap()
            // } else {
            //     "".into()
            // }
            if let Ok(Some(v)) = record.info(b"callsets").integer() {
                v[0]
            } else {
                0
            }
        })
        .collect();
    dbg!(now.elapsed());
    dbg!(&_htslib_counts.most_common_ordered()[..5]);

    Ok(())
}
