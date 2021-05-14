pub(crate) mod parser;
pub mod reader;
pub mod record;
pub mod types;

pub use reader::BcfRecords;
pub use record::BcfRecord;
pub use record::Record;

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
    fn test_info_flag() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records
            .next()
            .map(|record| assert_eq!(record.has_flag(b"FLAG"), true));
        records
            .next()
            .map(|record| assert_eq!(record.has_flag(b"FLAG"), false));
    }

    #[test]
    fn test_info_single_integer() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let field = record.info(b"INT").unwrap();
            let values = field.integer();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], 1)
        });
    }

    #[test]
    fn test_info_single_float() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let field = record.info(b"FLOAT").unwrap();
            let values = field.float();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], 0.5)
        });
    }

    #[test]
    fn test_info_single_string() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let field = record.info(b"STRING").unwrap();
            let values = field.string();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], b"String")
        });
    }

    #[test]
    fn test_info_two_integers() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let field = record.info(b"INT2").unwrap();
            let values = field.integer();
            assert_eq!(values.len(), 2);
            assert_eq!(values, [1, 2]);
        });
    }

    #[test]
    fn test_info_two_floats() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let field = record.info(b"FLOAT2").unwrap();
            let values = field.float();
            assert_eq!(values.len(), 2);
            assert_eq!(values, [0.5, 1.0]);
        });
    }

    #[test]
    fn test_info_two_strings() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let field = record.info(b"STRING2").unwrap();
            let values = field.string();
            assert_eq!(values.len(), 2);
            assert_eq!(values, vec![b"String1", b"String2"])
        });
    }

    #[test]
    fn test_info_n_alt_alleles_integers() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let field = record.info(b"INTA").unwrap();
            let values = field.integer();
            assert_eq!(values.len(), record.alt_alleles().len());
            assert_eq!(values, [1]);
        });
    }

    #[test]
    fn test_info_n_alleles_integers() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let field = record.info(b"INTR").unwrap();
            let values = field.integer();
            assert_eq!(values.len(), record.alt_alleles().len() + 1);
            assert_eq!(values, [1, 2]);
        });
    }

    #[test]
    fn test_info_variable_number_integers() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let field = record.info(b"INTX").unwrap();
            let values = field.integer();
            assert_eq!(values.len(), 4);
            assert_eq!(values, [1, 2, 3, 4]);
        });
    }

    #[test]
    fn test_format_single_integer() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let samples = record.format(b"INT").unwrap();
            let field = &samples[0];
            let values = field.integer();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], 1);
        });
    }

    #[test]
    fn test_format_n_genotypes_integer() {
        let mut records = BcfRecords::from_path("resources/types.bcf").unwrap();
        records.next().map(|record| {
            let samples = record.format(b"INTG").unwrap();
            let field = &samples[0];
            let values = field.integer();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], 1);
        });
    }
}
