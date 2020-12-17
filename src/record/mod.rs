mod eager;
mod lazy;
use crate::types::{Text, TypedVec};
pub use eager::BcfRecord;
pub use lazy::RawBcfRecord;

pub trait Record {
    fn chrom(&self) -> &str;

    fn pos(&self) -> u32;

    fn ref_allele(&self) -> Text;

    fn alt_alleles(&self) -> Vec<Text>;

    fn qual(&self) -> Option<f32>;

    fn filters(&self) -> Vec<&str>;

    fn info(&self, tag: &[u8]) -> Option<TypedVec>;

    fn format(&self, tag: &[u8]) -> Option<Vec<TypedVec>>;
}
