use rust_bcf::reader::BcfRecords;
use rust_bcf::record::Record;
use std::path::Path;

pub fn format_dp<P: AsRef<Path>>(path: P) {
    let records = BcfRecords::from_path(path).unwrap();
    records.for_each(|record| {
        record.format(b"DP").map(|v| v[0].integer()[0]).unwrap_or(0);
    });
}

pub fn info_callsets<P: AsRef<Path>>(path: P) {
    let records = BcfRecords::from_path(path).unwrap();
    records.for_each(|record| {
        record.info(b"callsets").map(|v| v.integer()[0]);
    });
}

pub fn qual<P: AsRef<Path>>(path: P) {
    let records = BcfRecords::from_path(path).unwrap();
    records.for_each(|record| {
        record.qual();
    });
}

pub fn chrom<P: AsRef<Path>>(path: P) {
    let records = BcfRecords::from_path(path).unwrap();
    records.for_each(|record| {
        record.chrom();
    });
}
