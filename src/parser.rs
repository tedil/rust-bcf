use std::collections::HashMap;
use std::convert::TryFrom;
use std::str::FromStr;

use anyhow::Result;
use itertools::Itertools;
use nom::branch::alt;
use nom::bytes::streaming::take_while;
use nom::bytes::streaming::{escaped, escaped_transform, is_not};
use nom::character::is_digit;
use nom::character::streaming::{alpha1, alphanumeric1, multispace0, none_of};
use nom::character::streaming::{anychar, digit1};
use nom::combinator::{complete, eof, map, opt, recognize};
use nom::lib::std::str::Utf8Error;
use nom::multi::{many0, many1, many_m_n, separated_list0};
use nom::sequence::{delimited, preceded, separated_pair, terminated};
use nom::{
    bytes::streaming::{tag, take, take_while_m_n},
    combinator::map_res,
    number::streaming::{le_f32, le_i16, le_i32, le_i8, le_u24, le_u32, le_u8},
    sequence::tuple,
    IResult, Parser,
};

use crate::types::{
    BcfRecord, HeaderKey, HeaderValue, InfoKey, InfoNumber, InfoType, TypeDescriptor, TypeKind,
    TypedVec, Version,
};

fn bcf_version(input: &[u8]) -> IResult<&[u8], Version> {
    let (input, bcf) = tag(b"BCF")(input)?;
    assert_eq!(bcf, b"BCF");
    let (input, major) = le_u8(input)?;
    let (input, minor) = le_u8(input)?;
    Ok((input, Version { major, minor }))
}

fn header_length(input: &[u8]) -> IResult<&[u8], u32> {
    let (input, length) = le_u32(input)?;
    Ok((input, length))
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
        let (input, num_elements) = le_u8(input)?;
        assert!(int == TypeKind::Int8 || int == TypeKind::Int16 || int == TypeKind::Int32);
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

fn typed_string(input: &[u8]) -> IResult<&[u8], String> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    assert_eq!(kind, TypeKind::String);
    let (input, string) = take(num_elements)(input)?;
    Ok((input, String::from_utf8(string.to_vec()).unwrap()))
}

fn read_string(length: usize, input: &[u8]) -> IResult<&[u8], String> {
    let (input, string) = take(length)(input)?;
    Ok((input, String::from_utf8(string.to_vec()).unwrap()))
}

fn typed_int8s(input: &[u8]) -> IResult<&[u8], Vec<i8>> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    assert_eq!(kind, TypeKind::Int8);
    let (input, data) = many_m_n(num_elements, num_elements, le_i8)(input)?;
    Ok((input, data))
}

fn typed_int16s(input: &[u8]) -> IResult<&[u8], Vec<i16>> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    assert_eq!(kind, TypeKind::Int16);
    let (input, data) = many_m_n(num_elements, num_elements, le_i16)(input)?;
    Ok((input, data))
}

fn typed_int32s(input: &[u8]) -> IResult<&[u8], Vec<i32>> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    assert!(kind == TypeKind::Int32 || kind == TypeKind::Missing);
    if kind == TypeKind::Missing {
        return Ok((input, vec![]));
    }
    let (input, data) = many_m_n(num_elements, num_elements, le_i32)(input)?;
    Ok((input, data))
}

fn typed_f32s(input: &[u8]) -> IResult<&[u8], Vec<f32>> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    assert_eq!(kind, TypeKind::Float32);
    let (input, data) = many_m_n(num_elements, num_elements, le_f32)(input)?;
    Ok((input, data))
}

fn typed_vec(input: &[u8]) -> IResult<&[u8], TypedVec> {
    let (input, TypeDescriptor { kind, num_elements }) = type_descriptor(input)?;
    let (input, vec) = match kind {
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
            let (input, data) = many_m_n(num_elements, num_elements, le_u8)(input)?;
            let data = String::from_utf8(data.to_vec()).unwrap();
            (
                input,
                TypedVec::String(data.split(',').map(str::to_owned).collect_vec()),
            )
        }
    };
    Ok((input, vec))
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

fn record(input: &[u8]) -> IResult<&[u8], BcfRecord> {
    let (input, (l_shared, l_indiv, chrom, pos, rlen, qual, n_info, n_allele, n_sample, n_fmt)) =
        tuple((
            le_u32, le_u32, le_i32, le_i32, le_i32, le_f32, le_i16, le_i16, le_u24, le_u8,
        ))(input)?;
    let (input, id) = typed_string(input)?;
    let (input, (alleles, filters)) = tuple((
        many_m_n(n_allele as usize, n_allele as usize, typed_string),
        typed_int32s,
    ))(input)?;
    let (input, info) = info(n_info, input)?;
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
            format: None,
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

fn string(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(
        tag("\""),
        escaped(none_of("\\\""), '\\', alt((tag("\\"), tag("\"")))),
        tag("\""),
    )(input)
}

fn keys_and_values(input: &[u8]) -> IResult<&[u8], Vec<(&str, &str)>> {
    fn key_value(input: &[u8]) -> IResult<&[u8], (&str, &str)> {
        let (input, (key, value)) =
            separated_pair(is_not("<,=\n"), tag(b"="), alt((string, is_not(">,=\n"))))(input)?;
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
        "FILTER" => {
            HeaderValue::Filter(delimited(tag("<"), header_value_mapping, tag(">"))(value)?.1)
        }
        _ => HeaderValue::String(std::str::from_utf8(value).unwrap()),
    };
    // dbg!(&key, &value);
    Ok((input, (key, value)))
}

fn header(header_length: u32, input: &[u8]) -> IResult<&[u8], &[u8]> {
    let (input, header) = take(header_length)(input)?;
    // let foo = std::str::from_utf8(header).unwrap().split("\n").collect_vec();
    // dbg!(&foo);
    let (header, entries) = many0(header_entry)(header)?;
    dbg!(entries);
    Ok((input, header))
}

pub fn parse(input: &[u8]) -> Result<BcfRecord> {
    let (input, version) = bcf_version(input).unwrap();
    let (input, header_length) = header_length(input).unwrap();
    let (input, header) = header(header_length, input).unwrap();
    let (input, record) = record(input).unwrap();
    Ok(record)
}
