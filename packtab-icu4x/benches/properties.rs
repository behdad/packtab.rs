use criterion::{black_box, criterion_group, criterion_main, Criterion};
use icu_collections::codepointtrie::TrieValue;
use icu_properties::props::{GeneralCategory, Script};
use icu_properties::CodePointMapData;

include!(concat!(env!("OUT_DIR"), "/generated_benches.rs"));

fn corpus() -> Vec<u32> {
    let mut cps = Vec::new();
    cps.extend(0..=0x7f);
    cps.extend((0x80..=0x7ff).step_by(7));
    cps.extend((0x800..=0xffff).step_by(113));
    cps.extend((0x1_0000..=0x10_ffff).step_by(997));
    cps.push(0x11_0000);
    cps
}

fn bench_gc(c: &mut Criterion) {
    let data = corpus();
    let icu = CodePointMapData::<GeneralCategory>::new();

    c.bench_function("gc/icu4x", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                let value = if cp > char::MAX as u32 {
                    icu.get32(u32::MAX).to_u32()
                } else {
                    icu.get32(cp).to_u32()
                };
                acc ^= value;
            }
            black_box(acc)
        })
    });

    c.bench_function("gc/packtab", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= gc_lookup(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("gc/packtab-unsafe", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= gc_lookup_unsafe(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("gc/packtab-c9", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= gc_lookup_c9(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("gc/packtab-c5", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= gc_lookup_c5(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("gc/packtab-c5-unsafe", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= gc_lookup_c5_unsafe(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("gc/packtab-c9-unsafe", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= gc_lookup_c9_unsafe(black_box(cp));
            }
            black_box(acc)
        })
    });
}

fn bench_script(c: &mut Criterion) {
    let data = corpus();
    let icu = CodePointMapData::<Script>::new();

    c.bench_function("script/icu4x", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                let value = if cp > char::MAX as u32 {
                    icu.get32(u32::MAX).to_u32()
                } else {
                    icu.get32(cp).to_u32()
                };
                acc ^= value;
            }
            black_box(acc)
        })
    });

    c.bench_function("script/packtab", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= script_lookup(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("script/packtab-unsafe", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= script_lookup_unsafe(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("script/packtab-c9", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= script_lookup_c9(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("script/packtab-c5", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= script_lookup_c5(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("script/packtab-c5-unsafe", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= script_lookup_c5_unsafe(black_box(cp));
            }
            black_box(acc)
        })
    });

    c.bench_function("script/packtab-c9-unsafe", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for &cp in &data {
                acc ^= script_lookup_c9_unsafe(black_box(cp));
            }
            black_box(acc)
        })
    });
}

criterion_group!(benches, bench_gc, bench_script);
criterion_main!(benches);
