use std::borrow::Cow;

use multimap::MultiMap;
use nom::lib::std::collections::HashMap;
use num_enum::TryFromPrimitive;
use std::str::FromStr;
use strum::EnumString;

use crate::parser;

const MISSING_QUAL: f32 = 0x7F800001 as f32;

#[derive(Debug)]
pub struct BcfRecord<'a> {
    pub(crate) chrom: u32,
    pub(crate) pos: u32,
    pub(crate) id: Option<&'a str>,
    pub(crate) ref_allele: &'a str,
    pub(crate) alt_alleles: Vec<&'a str>,
    pub(crate) qual: Option<f32>,
    pub(crate) filter: Vec<usize>, // pointer into header dict
    pub(crate) info: Vec<(InfoKey, TypedVec<'a>)>,
    pub(crate) format: Option<Vec<(FormatKey, Vec<TypedVec<'a>>)>>,
    pub(crate) header: Option<&'a Header<'a>>,
}

#[derive(Debug)]
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
pub enum TypedVec<'a> {
    Missing,
    Int8(Vec<i8>),
    Int16(Vec<i16>),
    Int32(Vec<i32>),
    Float32(Vec<f32>),
    String(Cow<'a, str>),
}

#[derive(Debug)]
pub struct Header<'a> {
    pub(crate) meta: MultiMap<&'a str, HeaderValue<'a>>,
    pub(crate) info: Vec<HeaderInfo<'a>>,
    pub(crate) format: Vec<HeaderFormat<'a>>,
    pub(crate) contigs: Vec<HeaderContig<'a>>,
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
    Filter(HeaderFilter<'a>),
    Format(HeaderFormat<'a>),
    Contig(HeaderContig<'a>),
}

#[derive(Debug)]
pub struct HeaderInfo<'a> {
    id: &'a str,
    number: InfoNumber,
    kind: InfoType,
    description: &'a str,
    // may be empty
    source: &'a str,
    // may be empty
    version: &'a str,
    idx: usize,
    additional: HashMap<&'a str, &'a str>,
}

impl<'a> From<Vec<(&'a str, &'a str)>> for HeaderInfo<'a> {
    fn from(data: Vec<(&'a str, &'a str)>) -> Self {
        let mut h: HashMap<_, _> = data.into_iter().collect();
        let mut header_info = HeaderInfo {
            id: h.remove("ID").expect("ID is mandatory"),
            number: parser::info_number(h.remove("Number").expect("Number is mandatory"))
                .unwrap()
                .1,
            kind: InfoType::from_str(h.remove("Type").expect("Type is mandatory")).unwrap(),
            description: h.remove("Description").expect("Description is mandatory"),
            source: h.remove("Source").unwrap_or(&""),
            version: h.remove("Version").unwrap_or(&""),
            idx: str::parse(h.remove("IDX").unwrap_or(&"0")).unwrap(),
            additional: Default::default(),
        };
        header_info.additional = h;
        header_info
    }
}

#[derive(Debug)]
pub struct HeaderFormat<'a> {
    id: &'a str,
    number: InfoNumber,
    kind: InfoType,
    description: &'a str,
    idx: usize,
}

impl<'a> From<Vec<(&'a str, &'a str)>> for HeaderFormat<'a> {
    fn from(data: Vec<(&'a str, &'a str)>) -> Self {
        let mut h: HashMap<_, _> = data.into_iter().collect();
        HeaderFormat {
            id: h.remove("ID").expect("ID is mandatory"),
            number: parser::info_number(h.remove("Number").expect("Number is mandatory"))
                .unwrap()
                .1,
            kind: InfoType::from_str(h.remove("Type").expect("Type is mandatory")).unwrap(),
            description: h.remove("Description").expect("Description is mandatory"),
            idx: str::parse(h.remove("IDX").unwrap_or(&"0")).unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct HeaderContig<'a> {
    id: &'a str,
    length: Option<usize>,
    additional: HashMap<&'a str, &'a str>,
}

impl<'a> From<Vec<(&'a str, &'a str)>> for HeaderContig<'a> {
    fn from(data: Vec<(&'a str, &'a str)>) -> Self {
        let mut h: HashMap<_, _> = data.into_iter().collect();
        let mut header_info = HeaderContig {
            id: h.remove("ID").expect("ID is mandatory"),
            length: h.remove("length").map(|s| s.parse().ok()).flatten(),
            additional: Default::default(),
        };
        header_info.additional = h;
        header_info
    }
}

#[derive(Debug)]
pub struct HeaderFilter<'a> {
    id: &'a str,
    description: &'a str,
}

impl<'a> From<Vec<(&'a str, &'a str)>> for HeaderFilter<'a> {
    fn from(data: Vec<(&'a str, &'a str)>) -> Self {
        let mut h: HashMap<_, _> = data.into_iter().collect();
        HeaderFilter {
            id: h.remove("ID").expect("ID is mandatory"),
            description: h.remove("Description").expect("Description is mandatory"),
        }
    }
}
