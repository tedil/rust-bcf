use std::mem::size_of;
use std::ops::Range;
use std::rc::Rc;

use nom::multi::many_m_n;
use nom::number::streaming::{le_f32, le_i16, le_i32, le_u24};
use nom::IResult;

use crate::parser::{
    genotype_field, raw_genotype_field, raw_info_pair, type_descriptor, typed_ints, typed_string,
};
use crate::record::Record;
use crate::types::{Header, HeaderValue, Text, TypeDescriptor, TypeKind, TypedVec, MISSING_QUAL};
use itertools::Itertools;
use nom::number::complete::{le_u32, le_u8};

#[derive(Debug)]
pub struct RawBcfRecord {
    pub(crate) shared: Vec<u8>,
    pub(crate) format: Vec<u8>,
    pub(crate) header: Rc<Header>,
    allele_start_bytepos: usize,
}

const S_I16: usize = size_of::<i16>();
const S_I32: usize = size_of::<i32>();
const S_U8: usize = size_of::<u8>();
const S_U32: usize = size_of::<u32>();
const S_F32: usize = size_of::<f32>();

const TYPE_DESCRIPTOR_LENGTH: usize = size_of::<u8>();
const CHROM_BYTE_RANGE: Range<usize> = 0..S_I32;
const POS_BYTE_RANGE: Range<usize> = S_I32..S_I32 * 2;
const QUAL_BYTE_RANGE: Range<usize> = S_I32 * 3..S_I32 * 3 + S_F32;

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

    fn n_info(&self) -> usize {
        fn n_info_from_shared(shared: &[u8]) -> IResult<&[u8], i16> {
            let (remaining, v) = le_i16(&shared[S_I32 * 3 + S_F32..S_I32 * 3 + S_F32 + S_I16])?;
            Ok((remaining, v))
        }
        n_info_from_shared(&self.shared).unwrap().1 as usize
    }

    fn n_fmt_n_sample(&self) -> (usize, usize) {
        fn n_fmt_n_sample_from_shared(shared: &[u8]) -> IResult<&[u8], (u32, u8)> {
            let (remaining, n_sample) = le_u24(&shared[S_I32 * 3 + S_F32 + S_I16 * 2..])?;
            let (remaining, n_fmt) = le_u8(remaining)?;
            Ok((remaining, (n_sample, n_fmt)))
        }
        let (n_sample, n_fmt) = n_fmt_n_sample_from_shared(&self.shared).unwrap().1;
        (n_fmt as usize, n_sample as usize)
    }

    fn alleles(&self) -> (Vec<Text>, usize) {
        let n_allele = self.n_alleles();
        let start = self.allele_start_bytepos;
        fn alleles_from_shared(shared: &[u8], n_allele: usize) -> IResult<&[u8], Vec<Text>> {
            let (remaining, v) =
                many_m_n(n_allele as usize, n_allele as usize, typed_string)(shared).unwrap();
            Ok((remaining, v))
        }
        let (remaining, alleles) = alleles_from_shared(&self.shared[start..], n_allele).unwrap();
        let byte_pos_after_alleles = self.shared.len() - remaining.len();
        (alleles, byte_pos_after_alleles)
    }
}

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
            let (shared, _ref_allele) = typed_string(shared).unwrap();
            let (remaining, v) =
                many_m_n(n_allele - 1, n_allele - 1, typed_string)(shared).unwrap();
            Ok((remaining, v))
        }
        alleles_from_shared(&self.shared[start..], n_allele)
            .unwrap()
            .1
    }

    fn qual(&self) -> Option<f32> {
        fn qual_from_shared(shared: &[u8]) -> IResult<&[u8], f32> {
            let (remaining, v) = le_f32(&shared[QUAL_BYTE_RANGE])?;
            Ok((remaining, v))
        }
        let qual = qual_from_shared(&self.shared).unwrap().1;
        if qual.is_nan() && qual.to_bits() & 0b0000_0000_0100_0000_0000_0000_0000_0000 != 0
            || qual.to_bits() == MISSING_QUAL
        {
            None
        } else {
            Some(qual)
        }
    }

    fn filters(&self) -> Vec<&str> {
        // lazy access requires "reading" and discarding the alleles, since these have unknown size
        let (_, byte_pos) = self.alleles();

        let (_, filter_ids) = typed_ints(&self.shared[byte_pos..]).unwrap();
        let filters = self.header.meta.get_vec("FILTER").unwrap();
        filter_ids
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
        // lazy access requires "reading" and discarding the alleles, since these have unknown size
        let (_, byte_pos) = self.alleles();
        // … same goes for filters, since these have unknown size as well
        let (input, _) = typed_ints(&self.shared[byte_pos..]).unwrap();

        let n_info = self.n_info();
        let tag = std::str::from_utf8(tag).unwrap().to_owned();
        let mut input = input;
        // then read the tag-index-in-header for each info field …
        (0..n_info as usize)
            .map(|_| {
                // (note that raw_info_pair does not do type conversion between byteslice and
                // requested type)
                let (i, info) = raw_info_pair(input).unwrap();
                input = i;
                info
            })
            .filter_map(|(offset, data)| {
                // … and check if it corresponds to the tag we're looking for
                self.header.info_tag_to_offset.get(&tag).and_then(|&idx| {
                    if idx == offset {
                        // convert RawVec to TypedVec
                        Some(data.into())
                    } else {
                        None
                    }
                })
            })
            .next()
    }

    fn format(&self, tag: &[u8]) -> Option<Vec<TypedVec>> {
        if self.format.is_empty() {
            return None;
        }
        let (n_fmt, n_sample) = self.n_fmt_n_sample();

        let tag = std::str::from_utf8(tag).unwrap().to_owned();
        let mut input = &self.format[..];
        (0..n_fmt as usize)
            .map(|_| {
                // (note that raw_info_pair does not do type conversion between byteslice and
                // requested type)
                let (i, fmt) = raw_genotype_field(n_sample as u32, input).unwrap();
                input = i;
                fmt
            })
            .filter_map(|(offset, data)| {
                // … and check if it corresponds to the tag we're looking for
                self.header.format_tag_to_offset.get(&tag).and_then(|&idx| {
                    if idx == offset {
                        // convert RawVec to TypedVec
                        Some(data.into_iter().map(Into::into).collect_vec())
                    } else {
                        None
                    }
                })
            })
            .next()
    }
}
