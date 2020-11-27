use crate::types::{FormatKey, Header, HeaderValue, InfoKey, Text, TypedVec};

#[derive(Debug)]
pub struct BcfRecord {
    pub(crate) chrom: u32,
    pub(crate) pos: u32,
    pub(crate) id: Option<Text>,
    pub(crate) ref_allele: Text,
    pub(crate) alt_alleles: Vec<Text>,
    pub(crate) qual: Option<f32>,
    pub(crate) filter: Vec<usize>,
    // pointer into header dict
    pub(crate) info: Vec<(InfoKey, TypedVec)>,
    pub(crate) format: Option<Vec<(FormatKey, Vec<TypedVec>)>>,
    pub(crate) header: Rc<Header>,
}

impl BcfRecord {
    pub fn info<'a>(&'a self, tag: &'a [u8]) -> Info<'_> {
        Info { record: self, tag }
    }

    pub fn ref_allele(&self) -> &[u8] {
        &self.ref_allele
    }

    pub fn filters(&self) -> Vec<&String> {
        let filters = self.header.meta.get_vec("FILTER").unwrap();
        self.filter
            .iter()
            .map(|&i| {
                let value = &filters[i];
                if let HeaderValue::Filter(f) = value {
                    &f.id
                } else {
                    unreachable!()
                }
            })
            .collect()
    }
}

/// Info tag representation.
#[derive(Debug)]
pub struct Info<'a> {
    record: &'a BcfRecord,
    tag: &'a [u8],
}

use std::rc::Rc;

impl<'a> Info<'a> {
    /// Info tag.
    pub fn tag(&self) -> String {
        std::str::from_utf8(self.tag).unwrap().to_owned()
    }

    fn data(&self) -> Option<&TypedVec> {
        self.record
            .info
            .iter()
            .filter_map(|(offset, data)| {
                let idx = self.record.header.tag_to_offset[&self.tag()];
                if idx == *offset {
                    Some(data)
                } else {
                    None
                }
            })
            .next()
    }

    /// Get integers from tag. `None` if tag not present in record.
    ///
    /// Import `bcf::record::Numeric` for missing value handling.
    pub fn integer(&mut self) -> Option<&[i32]> {
        let data = self.data();
        data.and_then(|data| match data {
            TypedVec::Missing => None,
            TypedVec::Int32(v) => Some(v.as_slice()),
            TypedVec::Float32(_) => None,
            TypedVec::UString(_) => None,
        })
    }

    /// Get floats from tag. `None` if tag not present in record.
    ///
    /// Import `bcf::record::Numeric` for missing value handling.
    pub fn float(&mut self) -> Option<&[f32]> {
        let data = self.data();
        data.and_then(|data| match data {
            TypedVec::Missing => None,
            TypedVec::Int32(_) => None,
            TypedVec::Float32(v) => Some(v.as_slice()),
            TypedVec::UString(_) => None,
        })
    }

    /// Get flags from tag. `false` if not set.
    pub fn flag(&mut self) -> Option<bool> {
        unimplemented!()
    }

    /// Get strings from tag. `None` if tag not present in record.
    pub fn string(&mut self) -> Option<Vec<&[u8]>> {
        let data = self.data();
        data.and_then(|data| match data {
            TypedVec::Missing => None,
            TypedVec::Int32(_) => None,
            TypedVec::Float32(_) => None,
            TypedVec::UString(v) => Some(v.split(|c| *c == b',').collect()),
        })
    }
}
