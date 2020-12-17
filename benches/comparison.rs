mod vs;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use vs::rust_bcf;
use vs::rust_htslib;

const PATH: &str = "resources/example.uncompressed.bcf";

fn benchmark_chrom(c: &mut Criterion) {
    let path = PATH;
    let mut group = c.benchmark_group("CHROM");
    group.bench_with_input(BenchmarkId::new("RUST_BCF", path), &path, |b, &path| {
        b.iter(|| rust_bcf::chrom(path))
    });
    group.bench_with_input(BenchmarkId::new("RUST_HTSLIB", path), &path, |b, &path| {
        b.iter(|| rust_htslib::chrom(path))
    });
}

fn benchmark_qual(c: &mut Criterion) {
    let path = PATH;
    let mut group = c.benchmark_group("QUAL");
    group.bench_with_input(BenchmarkId::new("RUST_BCF", path), &path, |b, &path| {
        b.iter(|| rust_bcf::qual(path))
    });
    group.bench_with_input(BenchmarkId::new("RUST_HTSLIB", path), &path, |b, &path| {
        b.iter(|| rust_htslib::qual(path))
    });
}

fn benchmark_format(c: &mut Criterion) {
    let path = PATH;
    let mut group = c.benchmark_group("FORMAT['DP'][0][0]");
    group.bench_with_input(BenchmarkId::new("RUST_BCF", path), &path, |b, &path| {
        b.iter(|| rust_bcf::format_dp(path))
    });
    group.bench_with_input(BenchmarkId::new("RUST_HTSLIB", path), &path, |b, &path| {
        b.iter(|| rust_htslib::format_dp(path))
    });
}

fn benchmark_info(c: &mut Criterion) {
    let path = PATH;
    let mut group = c.benchmark_group("INFO['callsets'][0]");
    group.bench_with_input(BenchmarkId::new("RUST_BCF", path), &path, |b, &path| {
        b.iter(|| rust_bcf::info_callsets(path))
    });
    group.bench_with_input(BenchmarkId::new("RUST_HTSLIB", path), &path, |b, &path| {
        b.iter(|| rust_htslib::info_callsets(path))
    });
}

criterion_group!(benches, benchmark_chrom, benchmark_qual, benchmark_format, benchmark_info);
criterion_main!(benches);
