use crate::codegen::Language;
use crate::layer::{InnerLayerChain, OuterLayerInfo};
use crate::{generate, pack_table, pack_table_all, pick_solution};
use std::process::Command;

fn gen(data: &[i64], default: i64, lang: Language) -> String {
    let (info, best) = pack_table(data, Some(default), 1.0);
    generate(&info, best, "data", lang)
}

fn compile_and_run_c(c_code: &str, data: &[i64], default: i64) {
    let mut checks = String::new();
    for (i, &v) in data.iter().enumerate() {
        checks.push_str(&format!("  assert(data_get({}) == {});\n", i, v));
    }
    for i in data.len()..data.len() + 5 {
        checks.push_str(&format!("  assert(data_get({}) == {});\n", i, default));
    }

    let full = format!(
        "#include <assert.h>\n#include <stdio.h>\n{}\nint main() {{\n{}  printf(\"PASS\\n\");\n  return 0;\n}}\n",
        c_code, checks
    );

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("test.c");
    let bin = dir.path().join("test");
    std::fs::write(&src, &full).unwrap();

    let output = Command::new("cc")
        .args(["-o", bin.to_str().unwrap(), src.to_str().unwrap(), "-std=c99", "-Wall", "-Werror"])
        .output()
        .expect("Failed to run cc");
    assert!(
        output.status.success(),
        "C compilation failed:\n{}\n--- generated code ---\n{}",
        String::from_utf8_lossy(&output.stderr),
        full
    );

    let result = Command::new(&bin).output().expect("Failed to run binary");
    assert!(result.status.success());
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "PASS");
}

fn compile_and_run_rust(rs_code: &str, data: &[i64], default: i64) {
    let mut checks = String::new();
    for (i, &v) in data.iter().enumerate() {
        checks.push_str(&format!(
            "    assert_eq!(data_get({}) as i64, {}i64);\n",
            i, v
        ));
    }
    for i in data.len()..data.len() + 5 {
        checks.push_str(&format!(
            "    assert_eq!(data_get({}) as i64, {}i64);\n",
            i, default
        ));
    }

    let full = format!(
        "{}\nfn main() {{\n{}    println!(\"PASS\");\n}}\n",
        rs_code, checks
    );

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("test.rs");
    let bin = dir.path().join("test");
    std::fs::write(&src, &full).unwrap();

    let output = Command::new("rustc")
        .args(["-o", bin.to_str().unwrap(), src.to_str().unwrap()])
        .output()
        .expect("Failed to run rustc");
    assert!(
        output.status.success(),
        "Rust compilation failed:\n{}\n--- generated code ---\n{}",
        String::from_utf8_lossy(&output.stderr),
        full
    );

    let result = Command::new(&bin).output().expect("Failed to run binary");
    assert!(result.status.success());
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "PASS");
}

fn compile_and_run(code: &str, data: &[i64], default: i64, lang: Language) {
    match lang {
        Language::C => compile_and_run_c(code, data, default),
        Language::Rust { .. } => compile_and_run_rust(code, data, default),
    }
}

// ── pack_table public API ─────────────────────────────────────────

#[test]
fn test_pack_table_list() {
    let (_, _) = pack_table(&[1, 2, 3, 4], Some(0), 1.0);
}

#[test]
fn test_pack_table_all_returns_solutions() {
    let info = pack_table_all(&[1, 2, 3], Some(0));
    assert!(!info.solutions.is_empty());
}

#[test]
fn test_pick_solution_returns_index() {
    let info = pack_table_all(&[1, 2, 3, 4], Some(0));
    let idx = pick_solution(&info.solutions, 1.0);
    assert!(idx < info.solutions.len());
}

#[test]
fn test_compression_zero_picks_flat_solution() {
    let info = pack_table_all(&(0..64).collect::<Vec<_>>(), Some(0));
    let best = pick_solution(&info.solutions, 0.0);
    let solution = &info.solutions[best];
    assert!(!solution.is_palette());
    assert_eq!(solution.bits(), Some(0));
}

#[test]
fn test_compression_ten_picks_minimum_raw_cost() {
    let info = pack_table_all(&(0..64).collect::<Vec<_>>(), Some(0));
    let best = pick_solution(&info.solutions, 10.0);
    assert_eq!(
        info.solutions[best].cost(),
        info.solutions.iter().map(|s| s.cost()).min().unwrap()
    );
}

#[test]
fn test_exact_leading_cull_used_when_trimmed_span_inlines() {
    let data = [vec![0; 17], vec![1, 2, 3, 4]].concat();
    let (info, _best) = pack_table(&data, Some(0), 10.0);
    assert_eq!(info.base, 17);
}

#[test]
fn test_exact_leading_cull_skipped_when_trimmed_span_would_not_inline() {
    let data = [vec![0; 17], (0..32).collect::<Vec<i64>>()].concat();
    let (info, _best) = pack_table(&data, Some(0), 10.0);
    assert_ne!(info.base, 18);
}

#[test]
#[should_panic]
fn test_empty_data_panics() {
    pack_table(&[], Some(0), 1.0);
}

// ── End-to-end code generation and compilation ─────────────────

macro_rules! e2e_test {
    ($name:ident, $data:expr, $default:expr) => {
        mod $name {
            use super::*;

            #[test]
            fn c() {
                let data: Vec<i64> = $data;
                let code = gen(&data, $default, Language::C);
                compile_and_run(&code, &data, $default, Language::C);
            }

            #[test]
            fn rust() {
                let data: Vec<i64> = $data;
                let code = gen(&data, $default, Language::Rust { unsafe_access: false });
                compile_and_run(
                    &code,
                    &data,
                    $default,
                    Language::Rust { unsafe_access: false },
                );
            }

            #[test]
            fn rust_unsafe() {
                let data: Vec<i64> = $data;
                let code = gen(&data, $default, Language::Rust { unsafe_access: true });
                compile_and_run(
                    &code,
                    &data,
                    $default,
                    Language::Rust { unsafe_access: true },
                );
            }
        }
    };
}

e2e_test!(test_small, vec![1, 2, 3, 4], 0);
e2e_test!(test_ascending, (0..32).collect(), 0);
e2e_test!(test_repeated_pattern, vec![0, 1, 2, 3].into_iter().cycle().take(64).collect(), 0);
e2e_test!(test_sparse, {
    let mut d = vec![0i64; 100];
    d[7] = 42;
    d[50] = 99;
    d[99] = 1;
    d
}, 0);
e2e_test!(test_sparse_with_aligned_prefix_defaults, {
    let mut d = vec![0i64; 19];
    d[16] = 5;
    d[17] = 9;
    d[18] = 11;
    d
}, 0);
e2e_test!(test_large_values, vec![0, 1000, 2000, 3000, 4000, 5000], 0);
e2e_test!(test_16bit_values, (0..64).map(|i| i * 100).collect(), 0);
e2e_test!(test_nonzero_default, vec![5, 5, 5, 10, 5], 5);
e2e_test!(test_256_values, (0..256).collect(), 0);
e2e_test!(test_constant_nonzero, vec![42, 42, 42, 42], 0);
e2e_test!(test_two_values, vec![0, 1, 0, 1, 0, 1, 0, 1], 0);
e2e_test!(test_gcd_bake_in, vec![0, 6, 12, 18, 24, 30], 0);
e2e_test!(test_gcd_no_bake_in, vec![0, 128, 256, 384], 0);
e2e_test!(test_gcd_with_bias, vec![100, 106, 112, 118], 0);
e2e_test!(test_bias_bake_in, vec![100, 101, 102, 103, 104, 105], 0);
e2e_test!(test_bias_no_bake_in, vec![1000, 1001, 1002, 1003], 0);

// ── Rust-specific tests ─────────────────────────────────────────

#[test]
fn test_rust_safe_no_unsafe_keyword() {
    let code = gen(&[1, 2, 3, 4], 0, Language::Rust { unsafe_access: false });
    assert!(
        !code.contains("unsafe"),
        "Safe Rust code should not contain 'unsafe'"
    );
}

#[test]
fn test_rust_unsafe_has_get_unchecked() {
    let data: Vec<i64> = (0..256).map(|i| (i * 7) % 256).collect();
    let code = gen(&data, 0, Language::Rust { unsafe_access: true });
    assert!(code.contains("get_unchecked"), "Unsafe Rust should use get_unchecked");
    assert!(code.contains("unsafe"), "Unsafe Rust should contain 'unsafe'");
}

#[test]
fn test_rust_has_static_array() {
    let data: Vec<i64> = (0..256).map(|i| (i * 7) % 256).collect();
    let code = gen(&data, 0, Language::Rust { unsafe_access: false });
    assert!(code.contains("static"), "Rust code should have static arrays");
}

#[test]
fn test_rust_pub_crate_function() {
    let code = gen(&[1, 2, 3], 0, Language::Rust { unsafe_access: false });
    assert!(
        code.contains("pub(crate)"),
        "Rust code should have pub(crate) function"
    );
}

#[test]
fn test_rust_no_include() {
    let code = gen(&[1, 2, 3], 0, Language::Rust { unsafe_access: false });
    assert!(
        !code.contains("#include"),
        "Rust code should not have #include"
    );
}

// ── C-specific tests ────────────────────────────────────────────

#[test]
fn test_c_has_include() {
    let code = gen(&[1, 2, 3], 0, Language::C);
    assert!(code.contains("#include <stdint.h>"));
}

#[test]
fn test_c_has_data_get() {
    let code = gen(&[1, 2, 3], 0, Language::C);
    assert!(code.contains("data_get"));
}

// ── Inlining tests ──────────────────────────────────────────────

#[test]
fn test_small_data_no_array_c() {
    let code = gen(&[1, 2, 3, 4], 0, Language::C);
    assert!(
        !code.contains("data_u8["),
        "Small data should be inlined, not in array"
    );
    assert!(code.contains("data_get"));
}

#[test]
fn test_small_data_no_array_rust() {
    let code = gen(&[1, 2, 3, 4], 0, Language::Rust { unsafe_access: false });
    assert!(
        !code.contains(": [u8;"),
        "Small data should be inlined, not in array"
    );
    assert!(code.contains("data_get"));
}

#[test]
fn test_large_data_has_array() {
    let data: Vec<i64> = (0..256).map(|i| (i * 7) % 256).collect();
    let code = gen(&data, 0, Language::C);
    assert!(code.contains("uint8_t"));
}

// ── Mult bake-in tests ──────────────────────────────────────────

#[test]
fn test_mult_bake_in_small_gcd() {
    let info = OuterLayerInfo::new(&[0, 4, 8, 12], 0);
    assert_eq!(info.mult, 1);
}

#[test]
fn test_mult_no_bake_in_type_change() {
    let info = OuterLayerInfo::new(&[0, 128, 256, 384], 0);
    assert_eq!(info.mult, 128);
}

#[test]
fn test_mult_bake_in_no_mult_in_code() {
    let code = gen(&[0, 6, 12, 18], 0, Language::C);
    assert!(!code.contains("6*"), "Mult should be baked in");
}

#[test]
fn test_mult_no_bake_in_has_mult_in_code() {
    let code = gen(&[0, 128, 256, 384], 0, Language::C);
    assert!(code.contains("128*"), "Mult should be in generated code");
}

// ── Bias bake-in tests ──────────────────────────────────────────

#[test]
fn test_bias_bake_in_small() {
    let info = OuterLayerInfo::new(&[100, 105, 110, 115], 0);
    assert_eq!(info.bias, 0);
}

#[test]
fn test_bias_no_bake_in_type_change() {
    let info = OuterLayerInfo::new(&[1000, 1005, 1010, 1015], 0);
    assert_eq!(info.bias, 1000);
}

#[test]
fn test_bias_bake_in_no_bias_in_code() {
    let code = gen(&[200, 205, 210, 215], 0, Language::C);
    assert!(!code.contains("200+"), "Bias should be baked in");
}

#[test]
fn test_bias_no_bake_in_has_bias_in_code() {
    let code = gen(&[1000, 1005, 1010, 1015], 0, Language::C);
    assert!(code.contains("1000+"), "Bias should be in generated code");
}

// ── Cache optimization tests ────────────────────────────────────────

#[test]
fn test_frequent_pairs_get_lower_ids() {
    // Pattern: (1,2) appears 3 times, (3,4) appears 2 times, (5,6) appears 1 time
    let data = vec![1, 2, 1, 2, 1, 2, 3, 4, 3, 4, 5, 6];
    let chain = InnerLayerChain::new(data);

    // Get the first inner layer which has the mapping
    let layer = &chain.layers[0];
    let mapping = layer.mapping.as_ref().expect("Layer should have a mapping");

    let id_12 = mapping.get((1, 2)).expect("Pair (1,2) should exist");
    let id_34 = mapping.get((3, 4)).expect("Pair (3,4) should exist");
    let id_56 = mapping.get((5, 6)).expect("Pair (5,6) should exist");

    // (1,2) most frequent -> lowest ID
    // (3,4) second -> middle ID
    // (5,6) least frequent -> highest ID
    assert!(id_12 < id_34);
    assert!(id_34 < id_56);
}

#[test]
fn test_equal_frequency_sorted_by_position() {
    // All pairs appear once, so order by position
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let chain = InnerLayerChain::new(data);

    let layer = &chain.layers[0];
    let mapping = layer.mapping.as_ref().expect("Layer should have a mapping");

    let id_12 = mapping.get((1, 2)).expect("Pair (1,2) should exist");
    let id_34 = mapping.get((3, 4)).expect("Pair (3,4) should exist");
    let id_56 = mapping.get((5, 6)).expect("Pair (5,6) should exist");
    let id_78 = mapping.get((7, 8)).expect("Pair (7,8) should exist");

    // All have frequency 1, so order by position
    assert!(id_12 < id_34);
    assert!(id_34 < id_56);
    assert!(id_56 < id_78);
}

#[test]
fn test_mixed_frequency_and_position() {
    // (1,2) appears 2 times at positions 0,2
    // (3,4) appears 2 times at positions 1,3
    // (5,6) appears 1 time at position 4
    let data = vec![1, 2, 3, 4, 1, 2, 3, 4, 5, 6];
    let chain = InnerLayerChain::new(data);

    let layer = &chain.layers[0];
    let mapping = layer.mapping.as_ref().expect("Layer should have a mapping");

    let id_12 = mapping.get((1, 2)).expect("Pair (1,2) should exist");
    let id_34 = mapping.get((3, 4)).expect("Pair (3,4) should exist");
    let id_56 = mapping.get((5, 6)).expect("Pair (5,6) should exist");

    // (1,2) and (3,4) both appear twice, so sorted by position
    // (1,2) appears first (position 0) -> lower ID than (3,4) (position 1)
    assert!(id_12 < id_34);

    // (5,6) appears once -> highest ID
    assert!(id_56 > id_12);
    assert!(id_56 > id_34);
}

// ── Edge cases and boundary conditions ────────────────────────────

#[test]
fn test_single_value() {
    // Single value should work
    let data = vec![42];
    let (info, best) = pack_table(&data, Some(0), 1.0);
    let code = generate(&info, best, "data", Language::C);
    assert!(!code.is_empty());
}

#[test]
fn test_all_same_values() {
    // All identical values should optimize well
    let data = vec![7; 100];
    let (info, best) = pack_table(&data, Some(0), 1.0);
    let solution = &info.solutions[best];
    // Should recognize constant data
    assert_eq!(solution.cost(), 0); // Inlined
}

#[test]
fn test_negative_numbers() {
    // Negative numbers should work
    let data = vec![-5, -3, -1, 0, 1, 3, 5];
    let (info, best) = pack_table(&data, Some(0), 1.0);
    let code = generate(&info, best, "data", Language::C);
    compile_and_run(&code, &data, 0, Language::C);
}

#[test]
fn test_large_sparse_table() {
    // Sparse table with large indices
    let mut data = vec![0; 10001];
    data[0] = 1;
    data[1000] = 2;
    data[10000] = 3;
    let (info, _best) = pack_table(&data, Some(0), 1.0);
    assert!(!info.solutions.is_empty());
    // Should compress well due to sparsity
}

#[test]
fn test_u8_boundary() {
    // Test values at u8 boundary (255)
    let data = vec![0, 127, 255];
    let (info, best) = pack_table(&data, Some(0), 1.0);
    let code = generate(&info, best, "data", Language::C);
    assert!(code.contains("uint8_t"));
}

#[test]
fn test_u16_boundary() {
    // Test values requiring u16
    let data = vec![0, 255, 256, 65535];
    let (info, best) = pack_table(&data, Some(0), 1.0);
    let code = generate(&info, best, "data", Language::C);
    assert!(code.contains("uint16_t"));
}

#[test]
fn test_alternating_pattern() {
    // Alternating 0/1 pattern
    let data: Vec<i64> = (0..100).map(|i| i % 2).collect();
    let (info, best) = pack_table(&data, Some(0), 1.0);
    let solution = &info.solutions[best];
    // Should use sub-byte packing
    assert!(solution.cost() < 100); // Better than naive storage
}

#[test]
fn test_power_of_two_values() {
    // Values that are powers of two
    let data = vec![1, 2, 4, 8, 16, 32, 64, 128];
    let (info, best) = pack_table(&data, Some(0), 1.0);
    let code = generate(&info, best, "data", Language::C);
    assert!(!code.is_empty());
}

// ── Palette encoding tests ───────────────────────────────────────

#[test]
fn test_palette_generated_for_outlier() {
    // The outlier forces 32-bit storage; palette encoding should be generated.
    let data = vec![1i64, 2, 3, 2, 3, 2, 1, 0, 2, 1, 2, 2, 3, 3, 1, 11110124];
    let info = pack_table_all(&data, Some(0));

    let palette_solutions: Vec<_> = info.solutions.iter().filter(|s| s.is_palette()).collect();
    assert!(!palette_solutions.is_empty(), "should have at least one palette solution");

    // Palette should contain all unique values sorted.
    assert_eq!(info.palette, vec![0, 1, 2, 3, 11110124]);

    // A palette solution adds one lookup for the palette array.
    assert!(palette_solutions[0].n_lookups() >= 2);
}

#[test]
fn test_palette_skipped_all_unique() {
    let data: Vec<i64> = (0..100).collect();
    let info = pack_table_all(&data, Some(0));

    assert!(
        !info.solutions.iter().any(|s| s.is_palette()),
        "should not have palette solutions when all values unique"
    );
}

#[test]
fn test_palette_skipped_no_savings() {
    let data: Vec<i64> = (0..16).collect();
    let info = pack_table_all(&data, Some(0));

    assert!(
        !info.solutions.iter().any(|s| s.is_palette()),
        "should not have palette solutions when no bit savings"
    );
}

#[test]
fn test_palette_with_few_unique_values() {
    // 100 random values from {1,2,3,4,5} (Python random.seed(42)) plus one huge outlier.
    // Random data does not produce regular pairs, so binary splitting is less effective
    // than palette encoding for this range of values.
    let data: Vec<i64> = vec![
        1, 1, 3, 2, 2, 2, 1, 5, 1, 5, 4, 1, 1, 1, 2, 2, 5, 5, 1, 5, 2, 5, 4, 2, 4, 5, 3, 1, 2,
        4, 3, 3, 2, 2, 3, 1, 1, 4, 1, 3, 3, 5, 3, 1, 4, 5, 1, 4, 1, 5, 3, 5, 3, 5, 2, 1, 1, 2,
        3, 1, 2, 1, 4, 3, 4, 3, 2, 3, 3, 2, 3, 1, 5, 2, 5, 2, 2, 4, 4, 3, 5, 2, 3, 1, 2, 1, 3,
        4, 3, 1, 2, 5, 3, 2, 4, 4, 4, 2, 3, 2, 999999,
    ];
    let info = pack_table_all(&data, Some(0));

    assert!(
        info.solutions.iter().any(|s| s.is_palette()),
        "should have palette solution for few-unique + outlier data"
    );
    assert!(info.palette.len() <= 7, "palette should be small");
}

#[test]
fn test_palette_cost_calculation() {
    let data = vec![1i64, 2, 3, 2, 3, 2, 1, 0, 2, 1, 2, 2, 3, 3, 1, 11110124];
    let info = pack_table_all(&data, Some(0));

    let palette_sol = info
        .solutions
        .iter()
        .find(|s| s.is_palette())
        .expect("should have palette solution");

    // Palette: 5 values × 4 bytes = 20 bytes; indices: 16 × 4 bits / 8 = 8 bytes → ~28 bytes.
    assert!(palette_sol.cost() <= 30, "palette cost should be ≤30 bytes");
    assert!(palette_sol.cost() < 64, "palette cost should beat direct (64 bytes)");
}

#[test]
fn test_palette_in_pareto_frontier() {
    // All returned solutions must be mutually non-dominated.
    let data = vec![1i64, 2, 3, 2, 3, 2, 1, 0, 2, 1, 2, 2, 3, 3, 1, 11110124];
    let info = pack_table_all(&data, Some(0));

    for a in &info.solutions {
        for b in &info.solutions {
            if std::ptr::eq(a, b) {
                continue;
            }
            assert!(
                !(a.n_lookups() <= b.n_lookups() && a.full_cost() <= b.full_cost()),
                "Found dominated solution in Pareto frontier"
            );
        }
    }
}

#[test]
fn test_palette_selected_large_dataset() {
    // 1000 random values from {1,2,3,4,5} (Python random.seed(42)) plus one huge outlier.
    // Random data: binary splitting is less effective than palette encoding.
    let data: Vec<i64> = vec![
        1, 1, 3, 2, 2, 2, 1, 5, 1, 5, 4, 1, 1, 1, 2, 2, 5, 5, 1, 5,
        2, 5, 4, 2, 4, 5, 3, 1, 2, 4, 3, 3, 2, 2, 3, 1, 1, 4, 1, 3,
        3, 5, 3, 1, 4, 5, 1, 4, 1, 5, 3, 5, 3, 5, 2, 1, 1, 2, 3, 1,
        2, 1, 4, 3, 4, 3, 2, 3, 3, 2, 3, 1, 5, 2, 5, 2, 2, 4, 4, 3,
        5, 2, 3, 1, 2, 1, 3, 4, 3, 1, 2, 5, 3, 2, 4, 4, 4, 2, 3, 2,
        2, 5, 5, 3, 5, 4, 5, 4, 3, 2, 2, 5, 4, 1, 1, 1, 2, 2, 4, 5,
        1, 4, 4, 5, 4, 5, 3, 5, 1, 1, 5, 3, 3, 1, 3, 4, 2, 4, 1, 3,
        5, 2, 5, 1, 3, 5, 5, 2, 2, 3, 2, 5, 5, 1, 5, 3, 4, 1, 1, 3,
        3, 2, 1, 2, 5, 1, 1, 4, 1, 5, 2, 2, 4, 5, 2, 3, 5, 5, 4, 2,
        5, 2, 3, 4, 3, 4, 5, 4, 1, 2, 2, 1, 3, 1, 5, 5, 2, 5, 2, 1,
        1, 1, 2, 1, 1, 3, 1, 5, 2, 3, 4, 2, 5, 2, 5, 5, 4, 2, 4, 4,
        2, 1, 1, 4, 3, 4, 4, 4, 1, 1, 1, 4, 3, 1, 2, 2, 2, 5, 4, 2,
        4, 2, 3, 4, 2, 1, 4, 5, 1, 1, 5, 1, 1, 2, 2, 4, 4, 4, 2, 4,
        1, 2, 4, 1, 4, 3, 4, 3, 4, 5, 4, 2, 2, 3, 2, 1, 5, 5, 1, 3,
        1, 1, 5, 4, 5, 5, 2, 1, 5, 1, 2, 1, 5, 1, 2, 4, 1, 5, 2, 5,
        5, 1, 5, 1, 4, 5, 5, 5, 3, 3, 2, 3, 2, 3, 4, 2, 3, 4, 3, 1,
        1, 4, 5, 5, 1, 1, 5, 2, 5, 3, 2, 3, 1, 2, 3, 3, 2, 4, 5, 3,
        5, 5, 1, 5, 3, 1, 2, 3, 1, 1, 5, 2, 3, 3, 5, 2, 3, 2, 3, 5,
        4, 3, 1, 1, 4, 3, 1, 1, 3, 2, 3, 2, 4, 5, 4, 5, 1, 1, 1, 2,
        5, 1, 3, 5, 5, 2, 4, 2, 1, 3, 3, 1, 3, 2, 2, 1, 3, 5, 4, 5,
        2, 2, 2, 2, 4, 1, 2, 3, 4, 2, 3, 2, 1, 4, 1, 4, 2, 2, 4, 3,
        3, 2, 2, 1, 2, 4, 3, 3, 1, 3, 3, 5, 4, 5, 3, 1, 1, 3, 2, 5,
        3, 1, 1, 5, 4, 3, 3, 4, 5, 5, 1, 4, 5, 2, 3, 1, 4, 1, 5, 5,
        2, 3, 4, 1, 3, 5, 3, 1, 3, 5, 3, 4, 3, 4, 3, 5, 2, 2, 4, 4,
        2, 5, 5, 3, 4, 5, 1, 3, 3, 2, 4, 5, 5, 3, 4, 4, 4, 2, 5, 4,
        2, 1, 3, 5, 5, 3, 1, 2, 3, 2, 2, 2, 1, 1, 2, 4, 5, 1, 4, 4,
        5, 2, 4, 4, 4, 2, 2, 1, 1, 4, 2, 2, 5, 4, 1, 5, 2, 1, 4, 2,
        4, 5, 5, 5, 3, 4, 5, 5, 4, 5, 4, 2, 4, 4, 3, 2, 3, 5, 4, 2,
        3, 4, 1, 3, 2, 3, 3, 3, 5, 1, 2, 2, 2, 4, 2, 2, 1, 4, 4, 3,
        5, 4, 4, 1, 2, 4, 4, 5, 1, 5, 4, 4, 1, 3, 3, 4, 4, 5, 5, 5,
        2, 4, 2, 3, 4, 4, 1, 4, 3, 4, 2, 4, 2, 5, 5, 1, 4, 5, 5, 1,
        1, 4, 2, 4, 2, 1, 3, 4, 3, 2, 4, 3, 3, 4, 3, 4, 3, 1, 4, 1,
        5, 1, 3, 2, 1, 1, 1, 2, 2, 1, 5, 2, 2, 2, 4, 1, 5, 2, 4, 3,
        3, 2, 5, 5, 1, 2, 3, 1, 5, 1, 3, 5, 4, 4, 2, 1, 5, 2, 1, 3,
        5, 1, 5, 1, 3, 5, 4, 3, 1, 5, 3, 1, 4, 4, 1, 4, 3, 4, 2, 4,
        2, 5, 3, 5, 5, 4, 4, 4, 5, 3, 3, 2, 1, 3, 4, 2, 4, 5, 5, 4,
        3, 1, 4, 3, 2, 4, 2, 3, 3, 3, 3, 5, 3, 5, 1, 5, 2, 1, 2, 4,
        4, 5, 2, 4, 4, 4, 1, 1, 3, 2, 4, 2, 3, 5, 3, 4, 5, 5, 3, 4,
        5, 3, 3, 4, 3, 3, 3, 2, 1, 2, 3, 1, 5, 2, 2, 2, 4, 3, 5, 5,
        5, 3, 1, 2, 3, 2, 3, 2, 3, 1, 5, 2, 3, 1, 1, 5, 3, 2, 4, 1,
        1, 5, 3, 4, 4, 4, 3, 2, 1, 3, 4, 1, 1, 4, 4, 1, 5, 1, 2, 2,
        5, 3, 1, 2, 1, 5, 4, 5, 5, 5, 2, 5, 4, 4, 4, 3, 5, 4, 3, 5,
        5, 1, 5, 1, 2, 2, 3, 1, 2, 2, 2, 5, 1, 2, 1, 4, 4, 5, 4, 3,
        1, 2, 3, 3, 4, 1, 2, 3, 5, 2, 4, 1, 5, 2, 2, 3, 2, 1, 1, 2,
        3, 5, 5, 3, 4, 1, 4, 3, 4, 3, 5, 5, 4, 4, 1, 5, 1, 4, 3, 5,
        3, 1, 1, 2, 5, 5, 1, 3, 5, 1, 2, 4, 5, 4, 3, 2, 5, 4, 4, 1,
        4, 3, 4, 3, 3, 1, 2, 3, 4, 4, 3, 4, 5, 1, 4, 1, 3, 3, 3, 1,
        4, 5, 1, 5, 4, 4, 1, 2, 5, 3, 5, 4, 4, 1, 2, 3, 5, 2, 3, 4,
        4, 1, 1, 5, 2, 2, 3, 5, 1, 5, 4, 1, 2, 1, 4, 1, 2, 4, 3, 5,
        3, 4, 4, 4, 2, 4, 5, 2, 4, 2, 5, 5, 2, 1, 3, 4, 3, 5, 3, 1,
        999999,
    ];

    let (info, best) = pack_table(&data, Some(0), 1.0);
    let solution = &info.solutions[best];

    assert!(solution.is_palette(), "palette should be selected for large dataset with outlier");
    assert_eq!(info.palette.len(), 6, "palette should have 5 values + 1 outlier");
}

#[test]
fn test_palette_code_generation_c() {
    let data = vec![1i64, 2, 3, 2, 3, 2, 1, 0, 2, 1, 2, 2, 3, 3, 1, 11110124];
    let info = pack_table_all(&data, Some(0));
    let palette_idx = info
        .solutions
        .iter()
        .position(|s| s.is_palette())
        .expect("should have palette solution");

    let code = generate(&info, palette_idx, "data", Language::C);
    assert!(code.contains("palette"), "C output should contain 'palette'");
    assert!(code.contains("11110124"), "C output should contain the outlier value");
    assert!(code.contains("/* packtab: "), "C output should contain a packtab shape comment");
    assert!(code.contains("palette["), "C output should mention palette size in the shape comment");
}

#[test]
fn test_palette_code_generation_rust() {
    let data = vec![1i64, 2, 3, 2, 3, 2, 1, 0, 2, 1, 2, 2, 3, 3, 1, 11110124];
    let info = pack_table_all(&data, Some(0));
    let palette_idx = info
        .solutions
        .iter()
        .position(|s| s.is_palette())
        .expect("should have palette solution");

    let code = generate(&info, palette_idx, "data", Language::Rust { unsafe_access: false });
    assert!(code.contains("palette"), "Rust output should contain 'palette'");
    assert!(code.contains("fn ") || code.contains("#[inline]"), "should contain a function");
    assert!(code.contains("/* packtab: "), "Rust output should contain a packtab shape comment");
}

#[test]
fn test_shape_comment_in_generated_function() {
    let data = vec![0i64; 32]
        .into_iter()
        .chain([1, 2, 3, 4])
        .collect::<Vec<_>>();
    let (info, best) = pack_table(&data, Some(0), 10.0);
    let code = generate(&info, best, "data", Language::C);
    assert!(code.contains("/* packtab: "), "generated output should contain a packtab shape comment");
    assert!(code.contains("base=32"), "shape comment should include the rebased base offset");
    assert!(code.contains("[2^"), "shape comment should include the leaf-inclusive shape list");
}

#[test]
fn test_palette_end_to_end_c() {
    // 50 values cycling through {10,20,30} plus one outlier.
    let data: Vec<i64> = (0..50i64)
        .map(|i| (i % 3 + 1) * 10)
        .chain(std::iter::once(999999))
        .collect();
    let info = pack_table_all(&data, Some(0));
    let palette_idx = info
        .solutions
        .iter()
        .position(|s| s.is_palette())
        .expect("should have palette solution");

    let code = generate(&info, palette_idx, "data", Language::C);
    compile_and_run_c(&code, &data, 0);
}

#[test]
fn test_palette_end_to_end_rust() {
    let data: Vec<i64> = (0..50i64)
        .map(|i| (i % 3 + 1) * 10)
        .chain(std::iter::once(999999))
        .collect();
    let info = pack_table_all(&data, Some(0));
    let palette_idx = info
        .solutions
        .iter()
        .position(|s| s.is_palette())
        .expect("should have palette solution");

    let code = generate(&info, palette_idx, "data", Language::Rust { unsafe_access: false });
    compile_and_run_rust(&code, &data, 0);
}

#[test]
fn test_palette_end_to_end_rust_unsafe() {
    let data: Vec<i64> = (0..50i64)
        .map(|i| (i % 3 + 1) * 10)
        .chain(std::iter::once(999999))
        .collect();
    let info = pack_table_all(&data, Some(0));
    let palette_idx = info
        .solutions
        .iter()
        .position(|s| s.is_palette())
        .expect("should have palette solution");

    let code = generate(&info, palette_idx, "data", Language::Rust { unsafe_access: true });
    compile_and_run_rust(&code, &data, 0);
}

#[test]
fn test_palette_with_bias() {
    // Values with a common offset + outlier; palette should still generate valid code.
    let data = vec![100i64, 101, 102, 101, 102, 101, 100, 99, 999999];
    let info = pack_table_all(&data, Some(0));

    if let Some(palette_idx) = info.solutions.iter().position(|s| s.is_palette()) {
        let code = generate(&info, palette_idx, "data", Language::C);
        assert!(!code.is_empty());
    }
    // If no palette solution was generated that's also valid (the bias
    // optimisation may collapse the range enough to skip palette).
}

#[test]
fn test_palette_with_repeated_pattern() {
    // Repeated base pattern + outlier; any solution should produce valid code.
    let base = vec![1i64, 2, 3, 2, 3, 2, 1, 0];
    let mut data: Vec<i64> = base.iter().cycle().take(8 * 32).copied().collect();
    data.push(999999);

    let (info, best) = pack_table(&data, Some(0), 5.0);
    let code = generate(&info, best, "data", Language::C);
    assert!(!code.is_empty());
}

#[test]
fn test_palette_separate_from_other_arrays() {
    // The palette array must appear under its own namespaced name.
    let data = vec![1i64, 2, 3, 2, 3, 2, 1, 0, 2, 1, 2, 2, 3, 3, 1, 11110124];
    let info = pack_table_all(&data, Some(0));
    let palette_idx = info
        .solutions
        .iter()
        .position(|s| s.is_palette())
        .expect("should have palette solution");

    let code = generate(&info, palette_idx, "data", Language::C);
    assert!(code.contains("data_palette"), "output should contain 'data_palette'");
    assert!(code.contains("palette["), "output should contain a palette[] access");
}

#[test]
fn test_inferred_default_uses_boundary_candidates() {
    let data = vec![9, 1, 2, 3, 0, 0, 0];

    let inferred = pack_table_all(&data, None);
    let explicit = pack_table_all(&data, Some(0));

    assert!(inferred.solutions.iter().any(|a| {
        explicit.solutions.iter().any(|b| {
            (a.n_lookups(), a.n_extra_ops(), a.cost())
                == (b.n_lookups(), b.n_extra_ops(), b.cost())
        })
    }));
}

#[test]
fn test_inferred_default_considers_both_boundary_values() {
    let data = vec![7, 0, 0, 0, 1];

    let inferred = pack_table_all(&data, None);
    let left = pack_table_all(&data, Some(7));
    let right = pack_table_all(&data, Some(1));

    let inferred_costs: std::collections::BTreeSet<_> = inferred
        .solutions
        .iter()
        .map(|s| (s.n_lookups(), s.n_extra_ops(), s.cost()))
        .collect();
    let candidate_costs: std::collections::BTreeSet<_> = left
        .solutions
        .iter()
        .chain(right.solutions.iter())
        .map(|s| (s.n_lookups(), s.n_extra_ops(), s.cost()))
        .collect();

    assert!(inferred_costs.is_subset(&candidate_costs));
    assert!(!inferred_costs.is_empty());
}
