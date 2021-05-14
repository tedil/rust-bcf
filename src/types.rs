use multimap::MultiMap;
use nom::lib::std::collections::HashMap;
use num_enum::TryFromPrimitive;
use std::str::FromStr;
use strum::EnumString;

use crate::parser;
use nom::combinator::map;
use nom::multi::many0;
use nom::number::complete::{le_f32, le_i16, le_i32, le_i8};
use nom::IResult;

pub(crate) const MISSING_QUAL: u32 = 0x7F800001;

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
pub enum RawVec<'a> {
    Missing,
    Int8(&'a [u8]),
    Int16(&'a [u8]),
    Int32(&'a [u8]),
    Float32(&'a [u8]),
    UString(&'a [u8]),
}

impl<'a> From<RawVec<'a>> for TypedVec {
    fn from(raw: RawVec<'a>) -> Self {
        match raw {
            RawVec::Missing => TypedVec::Missing,
            RawVec::Int8(input) => {
                fn parse(input: &[u8]) -> IResult<&[u8], Vec<i32>> {
                    let (input, data) = many0(map(le_i8, Into::into))(input)?;
                    Ok((input, data))
                }
                let (input, data) = parse(input).unwrap();
                assert!(input.is_empty());
                TypedVec::Int32(data)
            }
            RawVec::Int16(input) => {
                fn parse(input: &[u8]) -> IResult<&[u8], Vec<i32>> {
                    let (input, data) = many0(map(le_i16, Into::into))(input)?;
                    Ok((input, data))
                }
                let (input, data) = parse(input).unwrap();
                assert!(input.is_empty());
                TypedVec::Int32(data)
            }
            RawVec::Int32(input) => {
                fn parse(input: &[u8]) -> IResult<&[u8], Vec<i32>> {
                    let (input, data) = many0(le_i32)(input)?;
                    Ok((input, data))
                }
                let (input, data) = parse(input).unwrap();
                assert!(input.is_empty());
                TypedVec::Int32(data)
            }
            RawVec::Float32(input) => {
                fn parse(input: &[u8]) -> IResult<&[u8], Vec<f32>> {
                    let (input, data) = many0(le_f32)(input)?;
                    Ok((input, data))
                }
                let (input, data) = parse(input).unwrap();
                assert!(input.is_empty());
                TypedVec::Float32(data)
            }
            RawVec::UString(input) => TypedVec::UString(input.into()),
        }
    }
}

impl TypedVec {
    pub fn integer(&self) -> &[i32] {
        match self {
            TypedVec::Int32(v) => v.as_slice(),
            _ => unreachable!(),
        }
    }

    pub fn float(&mut self) -> &[f32] {
        match self {
            TypedVec::Float32(v) => v.as_slice(),
            _ => unreachable!(),
        }
    }

    pub fn flag(&self) -> bool {
        // from the VCFv4.3 spec:
        // """
        // Flags values — which can only appear in INFO fields — in BCF2 should be encoded by any non-reserved value.
        // The recommended best practice is to encode the value as an 1-element INT8 (type 0x11) with value of 1 to indicate present
        // """
        // Note the term "recommended best practice"; basically, the value for a flag may be anything,
        // so even TypeDescriptor { kind: Missing, num_elements: 0 } is valid.
        // That means that, if we can successfully call `flag()`, it has to be present.
        // We recommend using `record.has_flag(b"some_flag")` instead.
        true
    }

    pub fn string(&self) -> Vec<&[u8]> {
        match self {
            TypedVec::UString(v) => v.split(|c| *c == b',').collect(),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub struct Header {
    pub(crate) meta: MultiMap<String, HeaderValue>,
    pub(crate) info: HashMap<usize, HeaderInfo>,
    pub(crate) info_tag_to_offset: HashMap<String, usize>,
    pub(crate) format: HashMap<usize, HeaderFormat>,
    pub(crate) format_tag_to_offset: HashMap<String, usize>,
    pub(crate) contigs: Vec<HeaderContig>,
    pub(crate) samples: Vec<Sample>,
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

pub type Sample = String;

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
    pub(crate) id: String,
    number: InfoNumber,
    kind: InfoType,
    description: String,
    pub(crate) idx: usize,
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
    pub(crate) id: String,
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
