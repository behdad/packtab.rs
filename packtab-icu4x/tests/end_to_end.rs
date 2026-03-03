use icu_codepointtrie_builder::CodePointTrieBuilder;
use icu_collections::codepointtrie::{CodePointTrie, TrieType};
use packtab_icu4x::{generate_rust_code_from_trie, GenerateOptions};
use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn sample_trie(default_value: u8, error_value: u8) -> CodePointTrie<'static, u8> {
    let mut builder = CodePointTrieBuilder::new(default_value, error_value, TrieType::Small);
    builder.set_range_value('A' as u32..='Z' as u32, 1);
    builder.set_range_value('a' as u32..='z' as u32, 2);
    builder.set_range_value(0x1F600..=0x1F64F, 3);
    builder.build()
}

#[test]
fn generated_rust_matches_trie_runtime_results() {
    let trie = sample_trie(9, 250);
    let generated = generate_rust_code_from_trie(&trie, GenerateOptions::new("lookup")).unwrap();
    let dir = tempdir().unwrap();
    let source = dir.path().join("main.rs");
    let binary = dir.path().join("lookup-check");
    let cps = [0u32, 'A' as u32, 'Z' as u32, 'a' as u32, 'z' as u32, 0x1F600, 0x1F650, 0x11_0000];

    let expected = cps
        .iter()
        .map(|&cp| {
            if cp > char::MAX as u32 {
                trie.error_value().to_string()
            } else {
                trie.get32(cp).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let calls = cps
        .iter()
        .map(|cp| format!("    println!(\"{{}}\", lookup({cp}));"))
        .collect::<Vec<_>>()
        .join("\n");
    let source_text = format!(
        "{generated}\nfn main() {{\n{calls}\n}}\n"
    );
    fs::write(&source, source_text).unwrap();

    let output = Command::new("rustc")
        .arg("--edition=2021")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rustc failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(&binary).output().unwrap();
    assert!(
        output.status.success(),
        "generated binary failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
}
