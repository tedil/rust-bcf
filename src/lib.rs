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

    #[test]
    fn test_info_platforms() {
        let records = BcfRecords::from_path("resources/example.uncompressed.bcf").unwrap();
        let platforms_sum = records
            .map(|record| {
                record
                    .info(b"platforms")
                    .map(|values| values.integer()[0] as usize)
            })
            .flatten()
            .sum::<usize>();
        assert_eq!(platforms_sum, 3028);
    }

    #[test]
    fn test_flag() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records
            .next()
            .map(|record| assert_eq!(record.has_flag(b"FLAG"), true));
        records
            .next()
            .map(|record| assert_eq!(record.has_flag(b"FLAG"), false));
    }
}
