use crate::parser::{FormatKey, InfoKey, TypedVec};

#[repr(C)]
struct BcfString {}

const MISSING_QUAL: f32 = 0x7F800001 as f32;

#[derive(Debug)]
pub struct BcfRecord {
    pub(crate) chrom: u32,
    pub(crate) pos: u32,
    pub(crate) id: Option<String>,
    pub(crate) ref_allele: String,
    pub(crate) alt_alleles: Vec<String>,
    pub(crate) qual: Option<f32>,
    pub(crate) filter: Vec<i32>, // pointer into header dict
    pub(crate) info: Vec<(InfoKey, TypedVec)>,
    pub(crate) format: Option<(FormatKey, TypedVec)>,
}

#[repr(C)]
pub struct Version {
    pub(crate) major: u8,
    pub(crate) minor: u8,
}
