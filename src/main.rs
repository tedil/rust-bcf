use anyhow::Result;
// use bgzip::BGZFReader;
use counter::Counter;
use itertools::Itertools;

// use crate::parser::parse;
use crate::parser::BcfRecords;

mod parser;
mod record;
mod types;

fn main() -> Result<()> {
    let path = &std::env::args().collect_vec()[1];
    let records = BcfRecords::from_path(path);
    let header = records.header();
    dbg!(&header);
    let counts: Counter<_> = records
        .map(|record| record.ref_allele().len())
        // .map(|record| record.info(b"platforms").integer().unwrap()[0])
        .collect();
    dbg!(&counts.most_common_ordered()[..]);

    Ok(())
}
