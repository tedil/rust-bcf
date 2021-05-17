use std::mem::size_of;
use std::ops::Range;
use std::rc::Rc;

use nom::multi::many_m_n;
use nom::number::streaming::{le_f32, le_i16, le_i32, le_u24};
use nom::IResult;

use crate::parser::{raw_genotype_field, raw_info_pair, type_descriptor, typed_ints, typed_string};
use crate::types::{
    Header, HeaderValue, Text, TypeDescriptor, TypeKind, TypedVec, MISSING_FLOAT, NAN_FLOAT,
};
use itertools::Itertools;
use nom::number::complete::le_u8;
#[cfg(feature = "sync")]
use std::sync::Arc;

pub trait Record {
    fn id(&self) -> Text;

    fn chrom(&self) -> &str;

    fn pos(&self) -> u32;

    fn ref_allele(&self) -> Text;

    fn alt_alleles(&self) -> Vec<Text>;

    fn qual(&self) -> Option<f32>;

    fn filters(&self) -> Vec<&str>;

    fn info(&self, tag: &[u8]) -> Option<TypedVec>;

    fn format(&self, tag: &[u8]) -> Option<Vec<TypedVec>>;

    fn genotypes(&self) -> Vec<Vec<GenotypeAllele>>;

    fn has_flag(&self, tag: &[u8]) -> bool;
}

#[cfg(feature = "sync")]
unsafe impl Sync for BcfRecord {}

#[cfg(feature = "sync")]
unsafe impl Sync for Header {}

#[derive(Debug)]
pub struct BcfRecord {
    pub(crate) shared: Vec<u8>,
    pub(crate) format: Vec<u8>,
    #[cfg(not(feature = "sync"))]
    pub(crate) header: Rc<Header>,
    #[cfg(feature = "sync")]
    pub(crate) header: Arc<Header>,
    id_start_bytepos: usize,
    allele_start_bytepos: usize,
}

const S_I16: usize = size_of::<i16>();
const S_I32: usize = size_of::<i32>();
const S_U32: usize = size_of::<u32>();
const S_F32: usize = size_of::<f32>();

const TYPE_DESCRIPTOR_LENGTH: usize = size_of::<u8>();
const CHROM_BYTE_RANGE: Range<usize> = 0..S_I32;
const POS_BYTE_RANGE: Range<usize> = S_I32..S_I32 * 2;
const QUAL_BYTE_RANGE: Range<usize> = S_I32 * 3..S_I32 * 3 + S_F32;

impl BcfRecord {
    pub(crate) fn new(
        shared: Vec<u8>,
        format: Vec<u8>,
        #[cfg(not(feature = "sync"))] header: Rc<Header>,
        #[cfg(feature = "sync")] header: Arc<Header>,
    ) -> Self {
        // The list of alleles starts right after ID
        let id_start_bytepos = S_I32 + S_I32 + S_I32 + S_F32 + S_I16 + S_I16 + S_U32;
        // however, ID is a "typed string" in bcf-speak, so we have to read the type descriptor (1 byte)
        // to know how long the ID is (and then skip those bytes)
        let (_, TypeDescriptor { kind, num_elements }) =
            type_descriptor(&shared[id_start_bytepos..]).unwrap();
        assert_eq!(kind, TypeKind::String);
        let allele_start_bytepos = id_start_bytepos + TYPE_DESCRIPTOR_LENGTH + num_elements;
        Self {
            shared,
            format,
            header,
            id_start_bytepos,
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

impl Record for BcfRecord {
    /// Returns the ID of this record. If not set (equivalent to `.` in VCF), return `b""`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_bcf::BcfRecords;
    /// use rust_bcf::Record;
    ///
    /// let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
    /// for record in records {
    ///     assert_eq!(record.id(), b"TestId123");
    /// }
    /// ```
    fn id(&self) -> Text {
        let (_, id) = typed_string(&self.shared[self.id_start_bytepos..]).unwrap();
        id
    }

    /// Returns the target sequence identifier of this record, i.e. CHROM.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_bcf::BcfRecords;
    /// use rust_bcf::Record;
    ///
    /// let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
    /// for record in records {
    ///     assert_eq!(record.chrom(), "chr1")
    /// }
    /// ```
    fn chrom(&self) -> &str {
        fn chrom_from_shared(shared: &[u8]) -> IResult<&[u8], i32> {
            let (remaining, v) = le_i32(&shared[CHROM_BYTE_RANGE])?;
            Ok((remaining, v))
        }
        let idx = chrom_from_shared(&self.shared).unwrap().1 as usize;
        &self.header.contigs[idx].id
    }

    /// Returns the position of this record, i.e. POS, 0-based.
    ///
    /// Note that BCF is 0-based, while VCF is 1-based.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_bcf::BcfRecords;
    /// use rust_bcf::Record;
    ///
    /// let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
    /// for record in records {
    ///     assert_eq!(record.pos(), 817185)
    /// }
    /// ```
    fn pos(&self) -> u32 {
        fn pos_from_shared(shared: &[u8]) -> IResult<&[u8], u32> {
            let (remaining, v) = le_i32(&shared[POS_BYTE_RANGE])?;
            Ok((remaining, v as u32))
        }
        pos_from_shared(&self.shared).unwrap().1
    }

    /// Returns the reference allele of this record, i.e. REF.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_bcf::BcfRecords;
    /// use rust_bcf::Record;
    ///
    /// let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
    /// for record in records {
    ///     assert_eq!(record.ref_allele(), b"G")
    /// }
    /// ```
    fn ref_allele(&self) -> Text {
        let (_, ref_allele) = typed_string(&self.shared[self.allele_start_bytepos..]).unwrap();
        ref_allele
    }

    /// Returns the alternative alleles of this record, i.e. ALT.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_bcf::BcfRecords;
    /// use rust_bcf::Record;
    ///
    /// let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
    /// for record in records {
    ///     assert_eq!(record.alt_alleles(), [b"A"])
    /// }
    /// ```
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

    /// Returns the quality value of this record, i.e. QUAL.
    /// If not set (equivalent to `.` in VCF), return `None`.
    /// # Examples
    ///
    /// ```
    /// use rust_bcf::BcfRecords;
    /// use rust_bcf::Record;
    ///
    /// let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
    /// for record in records {
    ///     assert_eq!(record.qual(), Some(50.0))
    /// }
    /// ```
    fn qual(&self) -> Option<f32> {
        fn qual_from_shared(shared: &[u8]) -> IResult<&[u8], f32> {
            let (remaining, v) = le_f32(&shared[QUAL_BYTE_RANGE])?;
            Ok((remaining, v))
        }
        let qual = qual_from_shared(&self.shared).unwrap().1;
        if qual.is_nan() && qual.to_bits() & 0b0000_0000_0100_0000_0000_0000_0000_0000 != 0
            || qual.to_bits() == MISSING_FLOAT
        {
            None
        } else if qual.to_bits() == NAN_FLOAT {
            Some(f32::NAN)
        } else {
            Some(qual)
        }
    }

    /// Returns the list of filters for this record, i.e. FILTER.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_bcf::BcfRecords;
    /// use rust_bcf::Record;
    ///
    /// let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
    /// for record in records {
    ///     assert_eq!(record.filters(), ["PASS"])
    /// }
    /// ```
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

    /// For a given INFO tag, return its contents.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_bcf::BcfRecords;
    /// use rust_bcf::Record;
    ///
    /// let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
    /// for record in records {
    ///     assert_eq!(record.info(b"platforms").map(|value| value.integer()[0]), Some(3))
    /// }
    /// ```
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

    /// For a given INFO tag, return its contents.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_bcf::BcfRecords;
    /// use rust_bcf::Record;
    ///
    /// let mut records = BcfRecords::from_path("resources/example.id.bcf").unwrap();
    /// let sample = 1;
    /// for record in records {
    ///     assert_eq!(record.format(b"DP").map(|samples| samples[sample].integer()[0]), Some(1301))
    /// }
    /// ```
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

    fn genotypes(&self) -> Vec<Vec<GenotypeAllele>> {
        let gts = self.format(b"GT").unwrap_or_else(Vec::new);
        gts.iter()
            .map(|gt| {
                gt.integer()
                    .iter()
                    .cloned()
                    .map(GenotypeAllele::from)
                    .collect()
            })
            .collect()
    }

    fn has_flag(&self, tag: &[u8]) -> bool {
        self.info(tag).is_some()
    }
}

/// Phased or unphased alleles, represented as indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenotypeAllele {
    Unphased(i32),
    Phased(i32),
    UnphasedMissing,
    PhasedMissing,
}

impl From<i32> for GenotypeAllele {
    /// Decode given integer according to BCF standard.
    fn from(encoded: i32) -> Self {
        match (encoded, encoded & 1) {
            (0, 0) => GenotypeAllele::UnphasedMissing,
            (1, 1) => GenotypeAllele::PhasedMissing,
            (e, 1) => GenotypeAllele::Phased((e >> 1) - 1),
            (e, 0) => GenotypeAllele::Unphased((e >> 1) - 1),
            _ => panic!("unexpected phasing type"),
        }
    }
}

impl GenotypeAllele {
    /// Get the index into the list of alleles.
    pub fn index(self) -> Option<u32> {
        match self {
            GenotypeAllele::Unphased(i) | GenotypeAllele::Phased(i) => Some(i as u32),
            GenotypeAllele::UnphasedMissing | GenotypeAllele::PhasedMissing => None,
        }
    }
}
