use nom::lib::std::collections::HashMap;
use num_enum::TryFromPrimitive;
use strum::EnumString;

use crate::parser::HeaderInfo;

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

#[derive(Debug)]
pub struct TypeDescriptor {
    pub(crate) kind: TypeKind,
    pub(crate) num_elements: usize,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum TypeKind {
    Missing = 0,
    Int8 = 1,
    Int16 = 2,
    Int32 = 3,
    Reserved4 = 4,
    Float32 = 5,
    Reserved6 = 6,
    String = 7,
}

// The first value must be a typed atomic integer giving the offset of the INFO field key into the dictionary.
pub type InfoKey = usize;
pub type FormatKey = usize;

#[derive(Debug)]
pub enum TypedVec {
    Missing,
    Int8(Vec<i8>),
    Int16(Vec<i16>),
    Int32(Vec<i32>),
    Float32(Vec<f32>),
    String(Vec<String>),
}

#[derive(Debug)]
pub struct Header<'a> {
    meta: HashMap<&'a str, &'a str>,
    info: Vec<HeaderInfo<'a>>,
}

pub type HeaderKey<'a> = &'a str;

#[derive(Debug, Eq, PartialEq, EnumString)]
pub enum InfoType {
    Integer,
    Float,
    Flag,
    Character,
    String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum InfoNumber {
    Count(usize),
    Alleles,
    AlternateAlleles,
    Genotypes,
    Unknown,
}

#[derive(Debug)]
pub enum HeaderValue<'a> {
    String(&'a str),
    Info(HeaderInfo<'a>),
    Filter(HashMap<&'a str, &'a str>),
    Format(HashMap<&'a str, &'a str>),
    Contig(HashMap<&'a str, &'a str>),
}
