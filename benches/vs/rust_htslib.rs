use rust_htslib::bcf::{Read, Reader};
use std::path::Path;

pub fn format_dp<P: AsRef<Path>>(path: P) {
    let mut reader = Reader::from_path(path).unwrap();
    reader.records().for_each(|record| {
        if let Ok(v) = record.unwrap().format(b"DP").integer() {
            v[0][0]
        } else {
            0
        };
    });
}

pub fn info_callsets<P: AsRef<Path>>(path: P) {
    let mut reader = Reader::from_path(path).unwrap();
    reader.records().for_each(|record| {
        if let Ok(Some(v)) = record.unwrap().info(b"callsets").integer() {
            v[0]
        } else {
            0
        };
    });
}

pub fn qual<P: AsRef<Path>>(path: P) {
    let mut reader = Reader::from_path(path).unwrap();
    reader.records().for_each(|record| {
        record.unwrap().qual();
    });
}

pub fn chrom<P: AsRef<Path>>(path: P) {
    let mut reader = Reader::from_path(path).unwrap();
    reader.records().for_each(|record| {
        record.unwrap().rid();
    });
}
