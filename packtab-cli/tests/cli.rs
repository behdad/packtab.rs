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
fn sparse_input_works() {
    let output = run(&["--sparse", "--default", "0", "7:42", "50:99"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("/* packtab: "));
}
