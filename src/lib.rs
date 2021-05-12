pub(crate) mod parser;
pub mod reader;
pub mod record;
pub mod types;

pub use reader::BcfRecords;
pub use record::BcfRecord;

#[cfg(test)]
mod test {
    use super::reader::BcfRecords;
    use crate::record::Record;

    #[test]
    fn test_samples() {
        let records = BcfRecords::from_path("resources/example.uncompressed.bcf").unwrap();
        assert_eq!(
            records.header().samples,
            vec!["HG001", "INTEGRATION", "HG003"]
        );
    }

    #[test]
    fn test_id() {
        let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
        records
            .next()
            .map(|record| assert_eq!(record.id(), b"TestId123"));
    }

    #[test]
    fn test_ref_allele() {
        let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
        records
            .next()
            .map(|record| assert_eq!(record.ref_allele(), b"G"));
    }

    #[test]
    fn test_alt_alleles() {
        let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
        records
            .next()
            .map(|record| assert_eq!(record.alt_alleles(), vec![b"A"]));
    }
}
