use std::collections::HashMap;
use std::convert::TryFrom;
use std::iter::FromIterator;
use std::str::FromStr;

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

use crate::types::{
    BcfRecord, Header, HeaderKey, HeaderValue, InfoKey, InfoNumber, InfoType, TypeDescriptor,
    TypeKind, TypedVec, Version,
};

fn bcf_version(input: &[u8]) -> IResult<&[u8], Version> {
    let (input, _bcf) = tag(b"BCF")(input)?;
    let (input, major) = le_u8(input)?;
    let (input, minor) = le_u8(input)?;
    Ok((input, Version { major, minor }))
}

fn header_length(input: &[u8]) -> IResult<&[u8], u32> {
    let (input, length) = le_u32(input)?;
    Ok((input, length))
}

fn read_int(kind: TypeKind, input: &[u8]) -> IResult<&[u8], usize> {
    assert!(kind == TypeKind::Int8 || kind == TypeKind::Int16 || kind == TypeKind::Int32);
    match kind {
        TypeKind::Int8 => map(le_u8, |v| v as usize)(input),
        TypeKind::Int16 => map(le_u16, |v| v as usize)(input),
        TypeKind::Int32 => map(le_u32, |v| v as usize)(input),
        _ => unreachable!(),
    }
}

fn type_descriptor(input: &[u8]) -> IResult<&[u8], TypeDescriptor> {
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
        let (input, num_elements) = read_int(int, input)?;
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

fn typed_string(input: &[u8]) -> IResult<&[u8], &str> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    assert_eq!(kind, TypeKind::String);
    let (input, string) = take(num_elements)(input)?;
    Ok((input, std::str::from_utf8(string).unwrap()))
}

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

fn typed_ints(input: &[u8]) -> IResult<&[u8], Vec<usize>> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    match kind {
        TypeKind::Missing => Ok((input, vec![])),
        TypeKind::Int32 => map(many_m_n(num_elements, num_elements, le_i32), |v| {
            v.into_iter().map(|s| s as usize).collect()
        })(input),
        TypeKind::Int16 => map(many_m_n(num_elements, num_elements, le_i16), |v| {
            v.into_iter().map(|s| s as usize).collect()
        })(input),
        TypeKind::Int8 => map(many_m_n(num_elements, num_elements, le_i8), |v| {
            v.into_iter().map(|s| s as usize).collect()
        })(input),
        other => panic!("Unsupported FILTER type: {:?}", other),
    }
}

fn typed_vec_from_td<'a, 'b>(
    type_descriptor: &'b TypeDescriptor,
    input: &'a [u8],
) -> IResult<&'a [u8], TypedVec<'a>> {
    let num_elements = type_descriptor.num_elements;
    let (input, vec) = match type_descriptor.kind {
        TypeKind::Missing => (input, TypedVec::Missing),
        TypeKind::Int8 => {
            let (input, data) = many_m_n(num_elements, num_elements, le_i8)(input)?;
            (input, TypedVec::Int8(data))
        }
        TypeKind::Int16 => {
            let (input, data) = many_m_n(num_elements, num_elements, le_i16)(input)?;
            (input, TypedVec::Int16(data))
        }
        TypeKind::Int32 => {
            let (input, data) = many_m_n(num_elements, num_elements, le_i32)(input)?;
            (input, TypedVec::Int32(data))
        }
        TypeKind::Reserved4 => {
            unimplemented!()
        }
        TypeKind::Float32 => {
            let (input, data) = many_m_n(num_elements, num_elements, le_f32)(input)?;
            (input, TypedVec::Float32(data))
        }
        TypeKind::Reserved6 => {
            unimplemented!()
        }
        TypeKind::String => {
            // let (input, data) = many_m_n(num_elements, num_elements, le_u8)(input)?;
            // let data = String::from_utf8(data.to_vec()).unwrap();
            let (data, input) = input.split_at(num_elements);
            (
                input,
                // TypedVec::String(data.split(',').map(str::to_owned).collect_vec()),
                TypedVec::String(String::from_utf8_lossy(&data)),
            )
        }
    };
    Ok((input, vec))
}

fn typed_vec(input: &[u8]) -> IResult<&[u8], TypedVec> {
    let (input, type_descriptor) = type_descriptor(input)?;
    typed_vec_from_td(&type_descriptor, input)
}

fn info_pair(input: &[u8]) -> IResult<&[u8], (InfoKey, TypedVec)> {
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

fn info(n_info: i16, input: &[u8]) -> IResult<&[u8], Vec<(InfoKey, TypedVec)>> {
    let n_info = n_info as usize;
    many_m_n(n_info, n_info, info_pair)(input)
}

type FormatKey = usize;

fn genotype_field(n_sample: u32, input: &[u8]) -> IResult<&[u8], (usize, Vec<TypedVec>)> {
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

fn record<'a>(header: &Header<'a>, input: &'a [u8]) -> IResult<&'a [u8], BcfRecord<'a>> {
    let (input, (l_shared, l_indiv, chrom, pos, rlen, qual, n_info, n_allele, n_sample, n_fmt)) =
        tuple((
            le_u32, le_u32, le_i32, le_i32, le_i32, le_f32, le_i16, le_i16, le_u24, le_u8,
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
            qual: if qual.is_nan() {
                // && qual & 0b0000_0000_0100_0000_0000_0000_0000_0000 == 1 {
                None
            } else {
                Some(qual)
            },
            filter: filters,
            info,
            format,
            header: None,
        },
    ))
}

fn parse_usize(input: &str) -> usize {
    input.parse().unwrap()
}

fn info_number(input: &str) -> IResult<&str, InfoNumber> {
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
            number: info_number(h.remove("Number").expect("Number is mandatory"))
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
            number: info_number(h.remove("Number").expect("Number is mandatory"))
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

fn delimited_string(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(
        tag("\""),
        escaped(none_of("\\\""), '\\', alt((tag("\\"), tag("\"")))),
        tag("\""),
    )(input)
}

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
        _ => HeaderValue::String(std::str::from_utf8(value).unwrap()),
    };
    Ok((input, (key, value)))
}

fn header(header_length: u32, input: &[u8]) -> IResult<&[u8], Header> {
    let (input, header) = take(header_length)(input)?;
    let (_header, entries) = many0(header_entry)(header)?;
    let mut entries = MultiMap::from_iter(entries);
    let info = entries.remove("INFO").unwrap_or_else(|| vec![]);
    let format = entries.remove("FORMAT").unwrap_or_else(|| vec![]);
    let contigs = entries.remove("CONTIG").unwrap_or_else(|| vec![]);
    let header = Header {
        meta: entries,
        info: info
            .into_iter()
            .filter_map(|v| match v {
                HeaderValue::Info(info) => Some(info),
                _ => None,
            })
            .collect(),
        contigs: contigs
            .into_iter()
            .filter_map(|v| match v {
                HeaderValue::Contig(contig) => Some(contig),
                _ => None,
            })
            .collect(),
        format: format
            .into_iter()
            .filter_map(|v| match v {
                HeaderValue::Format(format) => Some(format),
                _ => None,
            })
            .collect(),
    };
    Ok((input, header))
}

const BCF_MAJOR_VERSION: u8 = 2;
const BCF_MINOR_VERSION: u8 = 2;

pub fn parse(input: &[u8]) -> impl Iterator<Item = BcfRecord> + '_ {
    let (input, version) = bcf_version(input).unwrap();
    assert_eq!(version.major, BCF_MAJOR_VERSION);
    assert_eq!(version.minor, BCF_MINOR_VERSION);
    dbg!(version);
    let (input, header_length) = header_length(input).unwrap();
    dbg!(header_length);
    let (input, header) = header(header_length, input).unwrap();
    dbg!(&header);

    struct BcfRecordsIterator<'a> {
        input: &'a [u8],
        header: Header<'a>,
    }

    impl<'a> Iterator for BcfRecordsIterator<'a> {
        type Item = BcfRecord<'a>;

        fn next(&mut self) -> Option<Self::Item> {
            if !self.input.is_empty() {
                let (inp, rec) = record(&self.header, self.input).unwrap();
                self.input = inp;
                Some(rec)
            } else {
                None
            }
        }
    }
    // let (input, records) = many_till(record, eof)(input).unwrap();
    // Ok(records.0)
    BcfRecordsIterator { input, header }
}
