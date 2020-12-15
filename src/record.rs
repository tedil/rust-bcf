//
// let (input, (chrom, pos, _rlen, qual, n_info, n_allele, n_sample, n_fmt)) = tuple((
//         le_i32, le_i32, le_i32, le_f32, le_i16, le_i16, le_u24, le_u8,
//     ))(input)?;
//     let (input, id) = typed_string(input)?;
//     let (input, (alleles, filters)) = tuple((
//         many_m_n(n_allele as usize, n_allele as usize, typed_string),
//         typed_ints,
//     ))(input)?;
//     let (input, info) = info(n_info, input)?;
use std::mem::size_of;
use std::rc::Rc;

use nom::error::ErrorKind;
use nom::number::streaming::{le_f32, le_i16, le_i32};
use nom::IResult;

use crate::parser::{type_descriptor, typed_string};
use crate::types::{
    FormatKey, Header, HeaderValue, InfoKey, Text, TypeDescriptor, TypeKind, TypedVec,
};
use nom::multi::many_m_n;
use std::ops::Range;

pub trait Record {
    fn chrom(&self) -> &str;

    fn pos(&self) -> u32;

    fn ref_allele(&self) -> Text;

    fn alt_alleles(&self) -> Vec<Text>;

    fn qual(&self) -> f32;

    fn filters(&self) -> Vec<&str>;

    fn info<'a>(&'a self, tag: &'a [u8]) -> Info<'_>;
}

#[derive(Debug)]
pub struct RawBcfRecord {
    pub(crate) shared: Vec<u8>,
    pub(crate) format: Vec<u8>,
    pub(crate) header: Rc<Header>,
    allele_start_bytepos: usize,
}

const S_I16: usize = size_of::<i16>();
const S_I32: usize = size_of::<i32>();
const S_U32: usize = size_of::<u32>();
const S_F32: usize = size_of::<f32>();
const TYPE_DESCRIPTOR_LENGTH: usize = size_of::<u8>();

impl RawBcfRecord {
    pub(crate) fn new(shared: Vec<u8>, format: Vec<u8>, header: Rc<Header>) -> Self {
        // The list of alleles starts right after ID
        let mut allele_start_bytepos = S_I32 + S_I32 + S_I32 + S_F32 + S_I16 + S_I16 + S_U32;
        // however, ID is a "typed string" in bcf-speak, so we have to read the type descriptor (1 byte)
        // to know long the ID is (and then skip those bytes)
        let (_, TypeDescriptor { kind, num_elements }) =
            type_descriptor(&shared[allele_start_bytepos..]).unwrap();
        assert_eq!(kind, TypeKind::String);
        allele_start_bytepos += TYPE_DESCRIPTOR_LENGTH + num_elements;
        Self {
            shared,
            format,
            header,
            allele_start_bytepos,
        }
    }

    fn n_alleles(&self) -> usize {
        fn n_alleles_from_shared(shared: &[u8]) -> IResult<&[u8], i16> {
            let (remaining, v) =
                le_i16(&shared[S_I32 * 3 + S_F32 + S_I16..S_I32 * 4 + S_F32 + S_I16])?;
            Ok((remaining, v))
        }
        n_alleles_from_shared(&self.shared).unwrap().1 as usize
    }
}

const CHROM_BYTE_RANGE: Range<usize> = 0..S_I32;
const POS_BYTE_RANGE: Range<usize> = S_I32..S_I32 * 2;
const QUAL_BYTE_RANGE: Range<usize> = S_I32 * 3..S_I32 * 3 + S_F32;

impl Record for RawBcfRecord {
    fn chrom(&self) -> &str {
        fn chrom_from_shared(shared: &[u8]) -> IResult<&[u8], i32> {
            let (remaining, v) = le_i32(&shared[CHROM_BYTE_RANGE])?;
            Ok((remaining, v))
        }
        let idx = chrom_from_shared(&self.shared).unwrap().1 as usize;
        &self.header.contigs[idx].id
    }

    fn pos(&self) -> u32 {
        fn pos_from_shared(shared: &[u8]) -> IResult<&[u8], u32> {
            let (remaining, v) = le_i32(&shared[POS_BYTE_RANGE])?;
            Ok((remaining, v as u32))
        }
        pos_from_shared(&self.shared).unwrap().1
    }

    fn ref_allele(&self) -> Text {
        let (_, ref_allele) = typed_string(&self.shared[self.allele_start_bytepos..]).unwrap();
        ref_allele
    }

    fn alt_alleles(&self) -> Vec<Text> {
        let n_allele = self.n_alleles();
        let start = self.allele_start_bytepos;
        fn alleles_from_shared(shared: &[u8], n_allele: usize) -> IResult<&[u8], Vec<Text>> {
            let (remaining, v) =
                many_m_n(n_allele as usize, n_allele as usize, typed_string)(shared).unwrap();
            Ok((remaining, v))
        }
        alleles_from_shared(&self.shared[start..], n_allele)
            .unwrap()
            .1
    }

    fn qual(&self) -> f32 {
        fn qual_from_shared(shared: &[u8]) -> IResult<&[u8], f32> {
            let (remaining, v) = le_f32(&shared[QUAL_BYTE_RANGE])?;
            Ok((remaining, v))
        }
        qual_from_shared(&self.shared).unwrap().1
    }

    fn filters(&self) -> Vec<&str> {
        unimplemented!()
    }

    fn info<'a>(&'a self, tag: &'a [u8]) -> Info<'_> {
        unimplemented!()
    }
}

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

    fn ref_allele(&self) -> Vec<u8> {
        self.ref_allele.to_vec()
    }

    fn alt_alleles(&self) -> Vec<Text> {
        unimplemented!()
    }

    fn qual(&self) -> f32 {
        unimplemented!()
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

    fn info<'a>(&'a self, tag: &'a [u8]) -> Info<'_> {
        Info { record: self, tag }
    }
}

/// Info tag representation.
#[derive(Debug)]
pub struct Info<'a> {
    record: &'a BcfRecord,
    tag: &'a [u8],
}

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
