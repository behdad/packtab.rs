use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_packtab"))
        .args(args)
        .output()
        .expect("failed to run packtab CLI")
}

#[test]
fn help_works() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Pack a list of integers"));
}

#[test]
fn c_output_contains_shape_comment() {
    let output = run(&["--language", "c", "1", "2", "3", "4"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#include <stdint.h>"));
    assert!(stdout.contains("/* packtab: "));
}

#[test]
fn rust_output_contains_shape_comment() {
    let output = run(&["--rust", "1", "2", "3", "4"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fn data_get"));
    assert!(stdout.contains("/* packtab: "));
}

#[test]
fn dual_compression_rejected_for_rust() {
    let output = run(&["--rust", "--compression", "1,9", "1", "2", "3"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("dual compression"));
}

#[test]
fn analyze_score_uses_exact_log2() {
    // The displayed Score must equal pick_solution's exact-log2 formula
    // (Lookups + compression*log2(FullCost)), not a floored ilog2, so the
    // highlighted "Best solution" matches the minimum-score row.
    let data: Vec<String> = (0..600).map(|i| (i % 50).to_string()).collect();
    let mut args: Vec<&str> = vec!["--analyze", "--default", "0", "--compression", "3"];
    let refs: Vec<&str> = data.iter().map(|s| s.as_str()).collect();
    args.extend_from_slice(&refs);
    let output = run(&args);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let compression = 3.0_f64;
    let mut checked = 0;
    for line in stdout.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Data rows are: idx Lookups ExtraOps Bytes FullCost Ratio(x) Score
        if cols.len() != 7 || cols[0].parse::<usize>().is_err() {
            continue;
        }
        let (Ok(lookups), Ok(full_cost), Ok(score)) = (
            cols[1].parse::<f64>(),
            cols[4].parse::<f64>(),
            cols[6].parse::<f64>(),
        ) else {
            continue;
        };
        let expected = lookups + compression * full_cost.log2();
        assert!(
            (score - expected).abs() < 0.02,
            "displayed score {} != exact {:.2} (lookups={}, fullcost={})\n{}",
            score,
            expected,
            lookups,
            full_cost,
            stdout
        );
        // Only a non-power-of-two FullCost distinguishes exact log2 from floor.
        if (full_cost.log2().fract()) > 0.01 {
            checked += 1;
        }
    }
    assert!(
        checked >= 1,
        "no non-power-of-two rows to distinguish exact vs floor log2:\n{}",
        stdout
    );
}

#[test]
fn sparse_input_works() {
    let output = run(&["--sparse", "--default", "0", "7:42", "50:99"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("/* packtab: "));
}
