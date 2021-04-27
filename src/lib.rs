pub(crate) mod parser;
pub mod reader;
pub mod record;
pub mod types;

pub use reader::BcfRecords;
pub use record::BcfRecord;

#[cfg(test)]
mod test {

    use super::reader::BcfRecords;

    #[test]
    fn test_samples() {
        let records = BcfRecords::from_path("resources/example.uncompressed.bcf").unwrap();
        assert_eq!(
            records.header().samples,
            vec!["HG001", "INTEGRATION", "HG003"]
        );
    }
}
