use std::collections::HashMap;
use std::convert::TryFrom;
use std::rc::Rc;

use itertools::Itertools;
use multimap::MultiMap;
use nom::branch::alt;
use nom::bytes::streaming::{escaped, is_not};
use nom::character::streaming::none_of;
use nom::combinator::map;
use nom::multi::{many0, many_m_n, separated_list0};
use nom::number::streaming::le_u16;
use nom::sequence::{delimited, separated_pair};
use nom::{
    bytes::streaming::{tag, take},
    number::streaming::{le_f32, le_i16, le_i32, le_i8, le_u24, le_u32, le_u8},
    sequence::tuple,
    IResult,
};

use crate::record::{BcfRecord, RawBcfRecord};
use crate::types::{
    Header, HeaderContig, HeaderFilter, HeaderFormat, HeaderInfo, HeaderKey, HeaderValue, InfoKey,
    InfoNumber, RawVec, Text, TypeDescriptor, TypeKind, TypedVec, Version, MISSING_QUAL,
};

/// The first 5 bytes in a BCF file are b"BCF" followed by two bytes
/// which encode major and minor version.
pub(crate) fn bcf_version(input: &[u8]) -> IResult<&[u8], Version> {
    let (input, _bcf) = tag(b"BCF")(input)?;
    let (input, major) = le_u8(input)?;
    let (input, minor) = le_u8(input)?;
    Ok((input, Version { major, minor }))
}

/// The length of the header follows directly after `bcf_version`
/// and is encoded as a 32bit unsigned integer
pub(crate) fn header_length(input: &[u8]) -> IResult<&[u8], u32> {
    let (input, length) = le_u32(input)?;
    Ok((input, length))
}

/// This is a convenience function for reading either of `u8`, `u16` and `u32`
/// while returning a `usize` suitable for indexing purposes
fn read_uint(kind: TypeKind, input: &[u8]) -> IResult<&[u8], usize> {
    assert!(kind == TypeKind::Int8 || kind == TypeKind::Int16 || kind == TypeKind::Int32);
    match kind {
        TypeKind::Int8 => map(le_u8, |v| v as usize)(input),
        TypeKind::Int16 => map(le_u16, |v| v as usize)(input),
        TypeKind::Int32 => map(le_u32, |v| v as usize)(input),
        _ => unreachable!(),
    }
}

/// A `TypeDescriptor` consists of:
/// - a single byte, where the lower 4 bits encode the type (see `TypeKind`),
///   and the upper 4 bits encode the number of elements (of that type) that follow
/// - if the number of elements is `0b1111` (i.e. 15), read another TypeDescriptor
///   which should describe a single integer and read its associated value
///   which gives the *actual* number of elements
pub(crate) fn type_descriptor(input: &[u8]) -> IResult<&[u8], TypeDescriptor> {
    let (input, type_descriptor_byte) = le_u8(input)?;
    let type_kind = type_descriptor_byte & 0b1111;
    let num_elements = (type_descriptor_byte >> 4) & 0b1111;
    let (input, num_elements) = if num_elements == 15 {
        let (
            input,
            TypeDescriptor {
                kind: int,
                num_elements: num_num_elements_ints,
            },
        ) = type_descriptor(input)?;
        assert_eq!(num_num_elements_ints, 1);
        let (input, num_elements) = read_uint(int, input)?;
        (input, num_elements as usize)
    } else {
        (input, num_elements as usize)
    };
    Ok((
        input,
        TypeDescriptor {
            kind: TypeKind::try_from(type_kind).unwrap(),
            num_elements,
        },
    ))
}

/// A "typed string" is just a sequence of characters/bytes
pub(crate) fn typed_string(input: &[u8]) -> IResult<&[u8], Text> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    assert_eq!(kind, TypeKind::String);
    let (input, string) = take(num_elements)(input)?;
    Ok((input, string.into()))
}

/// Similar to `read_uint`, but: We're reading *signed* integers here, which are subsequently used
/// as a *positive* offset into the header dictionary. I found no explanation as to why this choice
/// was made in the BCF specs.
fn typed_int(input: &[u8]) -> IResult<&[u8], usize> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    assert_eq!(num_elements, 1);
    let (input, value) = match kind {
        TypeKind::Int8 => {
            let (input, val) = le_i8(input)?;
            (input, val as usize)
        }
        TypeKind::Int16 => {
            let (input, val) = le_i16(input)?;
            (input, val as usize)
        }
        TypeKind::Int32 => {
            let (input, val) = le_i32(input)?;
            (input, val as usize)
        }
        x => panic!("Expected typed int, got {:?}", x),
    };
    Ok((input, value))
}

/// Read a vector of ints, again to be used as positive offsets; only used in the context of FILTER.
pub(crate) fn typed_ints(input: &[u8]) -> IResult<&[u8], Vec<usize>> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    match kind {
        TypeKind::Missing => Ok((input, vec![])),
        TypeKind::Int32 => many_m_n(num_elements, num_elements, map(le_i32, |v| v as usize))(input),
        TypeKind::Int16 => many_m_n(num_elements, num_elements, map(le_i16, |v| v as usize))(input),
        TypeKind::Int8 => many_m_n(num_elements, num_elements, map(le_i8, |v| v as usize))(input),
        other => panic!("Unsupported FILTER type: {:?}", other),
    }
}

fn raw_vec_from_td<'a, 'b>(
    type_descriptor: &'b TypeDescriptor,
    input: &'a [u8],
) -> IResult<&'a [u8], RawVec<'a>> {
    let num_elements = type_descriptor.num_elements;
    let (input, vec) = match type_descriptor.kind {
        TypeKind::Missing => (input, RawVec::Missing),
        TypeKind::Int8 => {
            let (data, input) = input.split_at(std::mem::size_of::<i8>() * num_elements);
            (input, RawVec::Int8(data))
        }
        TypeKind::Int16 => {
            let (data, input) = input.split_at(std::mem::size_of::<i16>() * num_elements);
            (input, RawVec::Int16(data))
        }
        TypeKind::Int32 => {
            let (data, input) = input.split_at(std::mem::size_of::<i32>() * num_elements);
            (input, RawVec::Int32(data))
        }
        TypeKind::Float32 => {
            let (data, input) = input.split_at(std::mem::size_of::<f32>() * num_elements);
            (input, RawVec::Int32(data))
        }
        TypeKind::String => {
            let (data, input) = input.split_at(std::mem::size_of::<u8>() * num_elements);
            (input, RawVec::UString(data))
        }
    };
    Ok((input, vec))
}

/// Reads the values described by `type_descriptor` and returns a `TypedVec` containing those values.
fn typed_vec_from_td<'a, 'b>(
    type_descriptor: &'b TypeDescriptor,
    input: &'a [u8],
) -> IResult<&'a [u8], TypedVec> {
    let num_elements = type_descriptor.num_elements;
    let (input, vec) = match type_descriptor.kind {
        TypeKind::Missing => (input, TypedVec::Missing),
        TypeKind::Int8 => {
            let (input, data) = many_m_n(num_elements, num_elements, map(le_i8, i32::from))(input)?;
            (input, TypedVec::Int32(data))
        }
        TypeKind::Int16 => {
            let (input, data) =
                many_m_n(num_elements, num_elements, map(le_i16, i32::from))(input)?;
            (input, TypedVec::Int32(data))
        }
        TypeKind::Int32 => {
            let (input, data) = many_m_n(num_elements, num_elements, le_i32)(input)?;
            (input, TypedVec::Int32(data))
        }
        TypeKind::Float32 => {
            let (input, data) = many_m_n(num_elements, num_elements, le_f32)(input)?;
            (input, TypedVec::Float32(data))
        }
        TypeKind::String => {
            // let (input, data) = many_m_n(num_elements, num_elements, le_u8)(input)?;
            // let data = String::from_utf8(data.to_vec()).unwrap();
            let (data, input) = input.split_at(num_elements);
            (
                input,
                // TypedVec::String(data.split(',').map(str::to_owned).collect_vec()),
                TypedVec::UString(data.into()),
            )
        }
    };
    Ok((input, vec))
}

/// First reads a `TypeDescriptor`, then the value(s) described by this type descriptor.
fn typed_vec(input: &[u8]) -> IResult<&[u8], TypedVec> {
    let (input, type_descriptor) = type_descriptor(input)?;
    typed_vec_from_td(&type_descriptor, input)
}

pub(crate) fn raw_info_pair(input: &[u8]) -> IResult<&[u8], (InfoKey, RawVec)> {
    let (input, td) = type_descriptor(input)?;
    assert_eq!(td.num_elements, 1);
    let (input, info_key_offset) = match td.kind {
        TypeKind::Int8 => {
            let (input, val) = le_i8(input)?;
            (input, val as InfoKey)
        }
        TypeKind::Int16 => {
            let (input, val) = le_i16(input)?;
            (input, val as InfoKey)
        }
        TypeKind::Int32 => {
            let (input, val) = le_i32(input)?;
            (input, val as InfoKey)
        }
        _ => panic!("The offset into the header dictionary for INFO keys must be an integer"),
    };
    let (input, td) = type_descriptor(input)?;
    let (input, data) = raw_vec_from_td(&td, input)?;
    Ok((input, (info_key_offset, data)))
}

/// Reads a `(InfoKey, TypedVec)` pair.
pub(crate) fn info_pair(input: &[u8]) -> IResult<&[u8], (InfoKey, TypedVec)> {
    let (input, type_descriptor) = type_descriptor(input)?;
    assert_eq!(type_descriptor.num_elements, 1);
    let (input, info_key_offset) = match type_descriptor.kind {
        TypeKind::Int8 => {
            let (input, val) = le_i8(input)?;
            (input, val as InfoKey)
        }
        TypeKind::Int16 => {
            let (input, val) = le_i16(input)?;
            (input, val as InfoKey)
        }
        TypeKind::Int32 => {
            let (input, val) = le_i32(input)?;
            (input, val as InfoKey)
        }
        _ => panic!("The offset into the header dictionary for INFO keys must be an integer"),
    };
    let (input, data) = typed_vec(input)?;
    Ok((input, (info_key_offset, data)))
}

/// Reads all INFO entries for a record
pub(crate) fn info(n_info: i16, input: &[u8]) -> IResult<&[u8], Vec<(InfoKey, TypedVec)>> {
    let n_info = n_info as usize;
    many_m_n(n_info, n_info, info_pair)(input)
}

type FormatKey = usize;

pub(crate) fn genotype_field(
    n_sample: u32,
    input: &[u8],
) -> IResult<&[u8], (usize, Vec<TypedVec>)> {
    let n_sample = n_sample as usize;
    let (input, fmt_key_offset) = typed_int(input)?;
    let (input, data_type) = type_descriptor(input)?;
    let mut input = input;
    let mut sample_values = Vec::with_capacity(n_sample);
    for _ in 0..n_sample {
        let r = typed_vec_from_td(&data_type, input)?;
        input = r.0;
        sample_values.push(r.1);
    }
    Ok((input, (fmt_key_offset as FormatKey, sample_values)))
}

pub(crate) fn raw_genotype_field(
    n_sample: u32,
    input: &[u8],
) -> IResult<&[u8], (usize, Vec<RawVec>)> {
    let n_sample = n_sample as usize;
    let (input, fmt_key_offset) = typed_int(input)?;
    let (input, data_type) = type_descriptor(input)?;
    let mut input = input;
    let mut sample_values = Vec::with_capacity(n_sample);
    for _ in 0..n_sample {
        let r = raw_vec_from_td(&data_type, input)?;
        input = r.0;
        sample_values.push(r.1);
    }
    Ok((input, (fmt_key_offset as FormatKey, sample_values)))
}

/// A record's length in bytes is given via the first two `u32`s, the first of which
/// is `l_shared`, i.e. the number of bytes used for storing everything from `CHROM` to the end of
/// `INFO`, the second of which is `l_indiv` which corresponds to the `FORMAT` entries.
pub(crate) fn record_length(input: &[u8]) -> IResult<&[u8], (u32, u32)> {
    tuple((le_u32, le_u32))(input)
}

/// Given `l_shared` and `l_indiv`, read the actual data defining the record.
/// Note that this actually parses everything (in contrast to htslib)
pub(crate) fn record_from_length(
    _l_shared: u32,
    l_indiv: u32,
    header: Rc<Header>,
    input: &[u8],
) -> IResult<&[u8], BcfRecord> {
    let (input, (chrom, pos, _rlen, qual, n_info, n_allele, n_sample, n_fmt)) = tuple((
        le_i32, le_i32, le_i32, le_f32, le_i16, le_i16, le_u24, le_u8,
    ))(input)?;
    let (input, id) = typed_string(input)?;
    let (input, (alleles, filters)) = tuple((
        many_m_n(n_allele as usize, n_allele as usize, typed_string),
        typed_ints,
    ))(input)?;
    let (input, info) = info(n_info, input)?;
    let (input, format) = if l_indiv > 0 {
        let (input, format) = many_m_n(n_fmt as usize, n_fmt as usize, |d| {
            genotype_field(n_sample, d)
        })(input)?;
        (input, Some(format))
    } else {
        (input, None)
    };
    Ok((
        input,
        BcfRecord {
            chrom: chrom as u32,
            pos: pos as u32,
            id: Some(id),
            ref_allele: alleles[0].clone(),
            alt_alleles: if alleles.len() > 1 {
                alleles[1..].to_vec()
            } else {
                vec![]
            },
            qual: if qual.is_nan()
                && qual.to_bits() & 0b0000_0000_0100_0000_0000_0000_0000_0000 != 0
                || qual.to_bits() == MISSING_QUAL
            {
                None
            } else {
                Some(qual)
            },
            filter: filters,
            info,
            format,
            header,
        },
    ))
}

pub(crate) fn raw_record_from_length(
    l_shared: u32,
    l_indiv: u32,
    header: Rc<Header>,
    input: &[u8],
) -> IResult<&[u8], RawBcfRecord> {
    let (shared, input) = input.split_at(l_shared as usize);
    let (l_indiv, input) = input.split_at(l_indiv as usize);
    Ok((
        input,
        RawBcfRecord::new(shared.to_vec(), l_indiv.to_vec(), header),
    ))
}

// -- functions for parsing the text header --

fn parse_usize(input: &str) -> usize {
    input.parse().unwrap()
}

/// This parses the INFO number char to `InfoNumber`
pub(crate) fn info_number(input: &str) -> IResult<&str, InfoNumber> {
    let r: IResult<&str, usize> = map(nom::character::complete::digit1, parse_usize)(input);
    if let Ok((input, number)) = r {
        Ok((input, InfoNumber::Count(number)))
    } else {
        let (input, char) = alt((nom::character::complete::alpha1, tag(".")))(input)?;
        let number = match char {
            "A" => InfoNumber::AlternateAlleles,
            "R" => InfoNumber::Alleles,
            "G" => InfoNumber::Genotypes,
            "." => InfoNumber::Unknown,
            x => panic!("Unknown Number type {}", x),
        };
        Ok((input, number))
    }
}

/// This reads a delimited string with `'\'` as the escape character.
fn delimited_string(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(
        tag("\""),
        escaped(none_of("\\\""), '\\', alt((tag("\\"), tag("\"")))),
        tag("\""),
    )(input)
}

/// This reads `key=value` pairs (in the header)
fn keys_and_values(input: &[u8]) -> IResult<&[u8], Vec<(&str, &str)>> {
    fn key_value(input: &[u8]) -> IResult<&[u8], (&str, &str)> {
        let (input, (key, value)) = separated_pair(
            is_not("<,=\n"),
            tag(b"="),
            alt((delimited_string, is_not(">,=\n"))),
        )(input)?;
        Ok((
            input,
            (
                std::str::from_utf8(key).unwrap(),
                std::str::from_utf8(value).unwrap(),
            ),
        ))
    }
    separated_list0(tag(","), key_value)(input)
}

fn header_value_mapping(input: &[u8]) -> IResult<&[u8], HashMap<&str, &str>> {
    let (input, mapping) = keys_and_values(input)?;
    Ok((input, mapping.into_iter().collect()))
}

fn header_line(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(tag(b"##"), is_not("\n"), tag("\n"))(input)
}

fn header_entry(input: &[u8]) -> IResult<&[u8], (HeaderKey, HeaderValue)> {
    let (input, line) = header_line(input)?;
    let (_rest, (key, value)) =
        separated_pair(is_not("="), tag("="), nom::bytes::complete::is_not("\n"))(line)?;
    let key = std::str::from_utf8(key).unwrap();
    let value = match key {
        "INFO" => {
            let data = delimited(tag("<"), header_value_mapping, tag(">"))(value)?.1;
            HeaderValue::Info(HeaderInfo::from(data.into_iter().collect_vec()))
        }
        "FORMAT" => {
            let data = delimited(tag("<"), header_value_mapping, tag(">"))(value)?.1;
            HeaderValue::Format(HeaderFormat::from(data.into_iter().collect_vec()))
        }
        "contig" => {
            let data = delimited(tag("<"), header_value_mapping, tag(">"))(value)?.1;
            HeaderValue::Contig(HeaderContig::from(data.into_iter().collect_vec()))
        }
        "FILTER" => {
            let data = delimited(tag("<"), header_value_mapping, tag(">"))(value)?.1;
            HeaderValue::Filter(HeaderFilter::from(data.into_iter().collect_vec()))
        }
        _ => HeaderValue::String(std::str::from_utf8(value).unwrap().into()),
    };
    Ok((input, (key, value)))
}

pub(crate) fn header(header_length: u32, input: &[u8]) -> IResult<&[u8], Header> {
    let (input, header) = take(header_length)(input)?;
    let (_header, entries) = many0(header_entry)(header)?;
    let mut entries = entries
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect::<MultiMap<_, _>>();
    let info = entries.remove("INFO").unwrap_or_else(Vec::new);
    let format = entries.remove("FORMAT").unwrap_or_else(Vec::new);
    let contigs = entries.remove("contig").unwrap_or_else(Vec::new);

    let info: HashMap<usize, HeaderInfo> = info
        .into_iter()
        .filter_map(|v| match v {
            HeaderValue::Info(info) => Some((info.idx, info)),
            _ => None,
        })
        .collect();
    let info_tag_to_offset = info.iter().map(|(idx, hi)| (hi.id.clone(), *idx)).collect();

    let format: HashMap<usize, HeaderFormat> = format
        .into_iter()
        .filter_map(|v| match v {
            HeaderValue::Format(format) => Some((format.idx, format)),
            _ => None,
        })
        .collect();
    let format_tag_to_offset = format
        .iter()
        .map(|(idx, hi)| (hi.id.clone(), *idx))
        .collect();

    let header = Header {
        meta: entries,
        info,
        info_tag_to_offset,
        contigs: contigs
            .into_iter()
            .filter_map(|v| match v {
                HeaderValue::Contig(contig) => Some(contig),
                _ => None,
            })
            .collect(),
        format,
        format_tag_to_offset,
    };
    Ok((input, header))
}
