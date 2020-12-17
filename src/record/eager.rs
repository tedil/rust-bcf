use std::rc::Rc;

use crate::record::Record;
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

impl Record for BcfRecord {
    fn chrom(&self) -> &str {
        &self.header.contigs[self.chrom as usize].id
    }

    fn pos(&self) -> u32 {
        self.pos
    }

    fn ref_allele(&self) -> Text {
        self.ref_allele.clone()
    }

    fn alt_alleles(&self) -> Vec<Text> {
        self.alt_alleles.clone()
    }

    fn qual(&self) -> Option<f32> {
        self.qual
    }

    fn filters(&self) -> Vec<&str> {
        let filters = self.header.meta.get_vec("FILTER").unwrap();
        self.filter
            .iter()
            .map(|&i| {
                let value = &filters[i];
                if let HeaderValue::Filter(f) = value {
                    f.id.as_ref()
                } else {
                    unreachable!()
                }
            })
            .collect()
    }

    fn info(&self, tag: &[u8]) -> Option<TypedVec> {
        unimplemented!()
    }

    fn format(&self, tag: &[u8]) -> Option<Vec<TypedVec>> {
        unimplemented!()
    }
}
