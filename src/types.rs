use multimap::MultiMap;
use nom::lib::std::collections::HashMap;
use num_enum::TryFromPrimitive;
use std::str::FromStr;
use strum::EnumString;

use crate::parser;
use std::rc::Rc;

const MISSING_QUAL: f32 = 0x7F800001 as f32;

pub(crate) type Text = Vec<u8>;
// pub(crate) type TextSlice<'a> = &'a [u8];

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
    // Reserved4 = 4,
    Float32 = 5,
    // Reserved6 = 6,
    String = 7,
}

// The first value must be a typed atomic integer giving the offset of the INFO field key into the dictionary.
pub type InfoKey = usize;
pub type FormatKey = usize;

#[derive(Debug)]
pub enum TypedVec {
    Missing,
    Int32(Vec<i32>),
    Float32(Vec<f32>),
    UString(Vec<u8>),
}

#[derive(Debug)]
pub struct Header {
    pub(crate) meta: MultiMap<String, HeaderValue>,
    pub(crate) info: HashMap<usize, HeaderInfo>,
    pub(crate) tag_to_offset: HashMap<String, usize>,
    pub(crate) format: Vec<HeaderFormat>,
    pub(crate) contigs: Vec<HeaderContig>,
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
pub enum HeaderValue {
    String(String),
    Info(HeaderInfo),
    Filter(HeaderFilter),
    Format(HeaderFormat),
    Contig(HeaderContig),
}

#[derive(Debug)]
pub struct HeaderInfo {
    pub(crate) id: String,
    number: InfoNumber,
    kind: InfoType,
    description: String,
    // may be empty
    source: String,
    // may be empty
    version: String,
    pub(crate) idx: usize,
    additional: HashMap<String, String>,
}

impl<'a> From<Vec<(&'a str, &'a str)>> for HeaderInfo {
    fn from(data: Vec<(&'a str, &'a str)>) -> Self {
        let mut h: HashMap<_, _> = data.into_iter().collect();
        let mut header_info = HeaderInfo {
            id: h.remove("ID").expect("ID is mandatory").into(),
            number: parser::info_number(h.remove("Number").expect("Number is mandatory"))
                .unwrap()
                .1,
            kind: InfoType::from_str(h.remove("Type").expect("Type is mandatory")).unwrap(),
            description: h
                .remove("Description")
                .expect("Description is mandatory")
                .into(),
            source: h.remove("Source").unwrap_or(&"").into(),
            version: h.remove("Version").unwrap_or(&"").into(),
            idx: str::parse(h.remove("IDX").unwrap_or(&"0")).unwrap(),
            additional: Default::default(),
        };
        header_info.additional = h.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        header_info
    }
}

#[derive(Debug)]
pub struct HeaderFormat {
    id: String,
    number: InfoNumber,
    kind: InfoType,
    description: String,
    idx: usize,
}

impl<'a> From<Vec<(&'a str, &'a str)>> for HeaderFormat {
    fn from(data: Vec<(&'a str, &'a str)>) -> Self {
        let mut h: HashMap<_, _> = data.into_iter().collect();
        HeaderFormat {
            id: h.remove("ID").expect("ID is mandatory").into(),
            number: parser::info_number(h.remove("Number").expect("Number is mandatory"))
                .unwrap()
                .1,
            kind: InfoType::from_str(h.remove("Type").expect("Type is mandatory")).unwrap(),
            description: h
                .remove("Description")
                .expect("Description is mandatory")
                .into(),
            idx: str::parse(h.remove("IDX").unwrap_or(&"0")).unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct HeaderContig {
    id: String,
    length: Option<usize>,
    additional: HashMap<String, String>,
}

impl<'a> From<Vec<(&'a str, &'a str)>> for HeaderContig {
    fn from(data: Vec<(&'a str, &'a str)>) -> Self {
        let mut h: HashMap<_, _> = data.into_iter().collect();
        let mut header_info = HeaderContig {
            id: h.remove("ID").expect("ID is mandatory").into(),
            length: h.remove("length").map(|s| s.parse().ok()).flatten(),
            additional: Default::default(),
        };
        header_info.additional = h.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        header_info
    }
}

#[derive(Debug)]
pub struct HeaderFilter {
    pub(crate) id: String,
    description: String,
}

impl<'a> From<Vec<(&'a str, &'a str)>> for HeaderFilter {
    fn from(data: Vec<(&'a str, &'a str)>) -> Self {
        let mut h: HashMap<_, _> = data.into_iter().collect();
        HeaderFilter {
            id: h.remove("ID").expect("ID is mandatory").into(),
            description: h
                .remove("Description")
                .expect("Description is mandatory")
                .into(),
        }
    }
}
