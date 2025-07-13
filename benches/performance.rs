use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustle_facts::{parse_fact_output, ArchitectureFacts};

fn bench_parse_fact_output(c: &mut Criterion) {
    let output = r#"
ARCH=x86_64
SYSTEM=Linux
OS_FAMILY=debian
DISTRIBUTION=ubuntu
"#;

    c.bench_function("parse_fact_output", |b| {
        b.iter(|| parse_fact_output(black_box(output)))
    });
}

fn bench_architecture_normalization(c: &mut Criterion) {
    let architectures = vec!["x86_64", "amd64", "aarch64", "arm64", "armv7l", "custom"];

    c.bench_function("normalize_architecture", |b| {
        b.iter(|| {
            for arch in &architectures {
                ArchitectureFacts::normalize_architecture(black_box(arch));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_parse_fact_output,
    bench_architecture_normalization
);
criterion_main!(benches);
