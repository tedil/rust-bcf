use crate::parser;
use crate::record::{BcfRecord, RawBcfRecord};
use crate::types::Header;
use nom::lib::std::mem::size_of;
use std::error::Error;
use std::io::Read;
use std::path::Path;
use std::rc::Rc;

const BCF_MAJOR_VERSION: u8 = 2;
const BCF_MINOR_VERSION: u8 = 2;

pub struct BcfRecords<R: Read> {
    header: Rc<Header>,
    length_buf: [u8; size_of::<u32>() * 2],
    record_buf: Vec<u8>,
    inner: Box<R>,
}

impl BcfRecords<Box<dyn Read>> {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let (reader, _format) = niffler::from_path(path)?;
        Self::new(reader)
    }
}

impl<R: Read> BcfRecords<R> {
    pub fn new(mut reader: R) -> Result<Self, Box<dyn Error>> {
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
            header: Rc::new(header),
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
        let (_, record) =
            parser::record_from_length(l_shared, l_indiv, self.header.clone(), &self.record_buf)
                .unwrap();
        Some(record)
    }
}

pub struct RawBcfRecords<R: Read> {
    header: Rc<Header>,
    length_buf: [u8; size_of::<u32>() * 2],
    record_buf: Vec<u8>,
    inner: Box<R>,
}

impl<R: Read> RawBcfRecords<R> {
    pub fn header(&self) -> &Header {
        self.header.as_ref()
    }
}

impl RawBcfRecords<Box<dyn Read>> {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let (reader, _format) = niffler::from_path(path)?;
        Self::new(reader)
    }
}

impl<R: Read> RawBcfRecords<R> {
    pub fn new(mut reader: R) -> Result<Self, Box<dyn Error>> {
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
            header: Rc::new(header),
            length_buf: [0u8; size_of::<u32>() * 2],
            record_buf: Vec::new(),
            inner: Box::new(reader),
        })
    }
}

impl<R: Read> Iterator for RawBcfRecords<R> {
    type Item = RawBcfRecord;

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
