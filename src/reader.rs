use crate::parser;
use crate::record::BcfRecord;
use crate::types::Header;
use nom::lib::std::mem::size_of;
use std::io::Read;
use std::path::Path;
use std::rc::Rc;
#[cfg(feature = "sync")]
use std::sync::Arc;

const BCF_MAJOR_VERSION: u8 = 2;
const BCF_MINOR_VERSION: u8 = 2;

pub struct BcfRecords<R: Read> {
    #[cfg(not(feature = "sync"))]
    header: Rc<Header>,
    #[cfg(feature = "sync")]
    header: Arc<Header>,
    length_buf: [u8; size_of::<u32>() * 2],
    record_buf: Vec<u8>,
    inner: Box<R>,
}

impl<R: Read> BcfRecords<R> {
    pub fn header(&self) -> &Header {
        self.header.as_ref()
    }
}

impl BcfRecords<Box<dyn Read>> {
    pub fn from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let (reader, _format) = niffler::from_path(path)?;
        Self::new(reader)
    }
}

impl<R: Read> BcfRecords<R> {
    pub fn new(mut reader: R) -> anyhow::Result<Self> {
        let mut input = [0u8; 5];
        reader.read_exact(&mut input)?;
        let (input, version) = parser::bcf_version(&input).unwrap();
        assert!(input.is_empty());
        assert_eq!(version.major, BCF_MAJOR_VERSION);
        assert_eq!(version.minor, BCF_MINOR_VERSION);

        let mut input = [0u8; size_of::<u32>()];
        reader.read_exact(&mut input)?;
        let (input, header_length) = parser::header_length(&input).unwrap();
        assert!(input.is_empty());

        let mut input = vec![0u8; header_length as usize];
        reader.read_exact(&mut input)?;
        let (input, header) = parser::header(header_length, &input).unwrap();
        assert!(input.is_empty());

        Ok(Self {
            #[cfg(not(feature = "sync"))]
            header: Rc::new(header),
            #[cfg(feature = "sync")]
            header: Arc::new(header),
            length_buf: [0u8; size_of::<u32>() * 2],
            record_buf: Vec::new(),
            inner: Box::new(reader),
        })
    }
}

impl<R: Read> Iterator for BcfRecords<R> {
    type Item = BcfRecord;

    fn next(&mut self) -> Option<Self::Item> {
        if self.inner.read_exact(&mut self.length_buf).is_err() {
            return None;
        };
        let (_, (l_shared, l_indiv)) = parser::record_length(&self.length_buf).unwrap();
        self.record_buf
            .resize(l_shared as usize + l_indiv as usize, 0);
        self.inner.read_exact(&mut self.record_buf).unwrap();
        let (_, record) = parser::raw_record_from_length(
            l_shared,
            l_indiv,
            self.header.clone(),
            &self.record_buf,
        )
        .unwrap();
        Some(record)
    }
}
