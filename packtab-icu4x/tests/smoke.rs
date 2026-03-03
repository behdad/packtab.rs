use icu_codepointtrie_builder::CodePointTrieBuilder;
use icu_collections::codepointtrie::{CodePointTrie, TrieType};
use packtab_icu4x::{
    flatten_code_point_trie, generate_rust_code, generate_rust_code_from_trie, GenerateError,
    GenerateOptions, PackedCodePointTrieInput, UNICODE_LEN,
};

fn sample_trie(default_value: u8, error_value: u8) -> CodePointTrie<'static, u8> {
    let mut builder = CodePointTrieBuilder::new(default_value, error_value, TrieType::Small);
    builder.set_range_value('A' as u32..='Z' as u32, 1);
    builder.set_range_value('a' as u32..='z' as u32, 2);
    builder.set_range_value(0x1F600..=0x1F64F, 3);
    builder.build()
}

#[test]
fn flatten_preserves_scalar_values_and_error_value() {
    let trie = sample_trie(9, 250);
    let packed = flatten_code_point_trie(&trie);

    assert_eq!(packed.scalar_data.len(), UNICODE_LEN);
    assert_eq!(packed.error_value, 250u8);
    assert_eq!(packed.scalar_data[0], 9);
    assert_eq!(packed.scalar_data['A' as usize], 1);
    assert_eq!(packed.scalar_data['Z' as usize], 1);
    assert_eq!(packed.scalar_data['a' as usize], 2);
    assert_eq!(packed.scalar_data['z' as usize], 2);
    assert_eq!(packed.scalar_data[0x1F600], 3);
    assert_eq!(packed.scalar_data[0x1F650], 9);
}

#[test]
fn generation_adds_error_guard_when_needed() {
    let trie = sample_trie(9, 250);
    let packed = flatten_code_point_trie(&trie);
    let code = generate_rust_code(&packed, GenerateOptions::new("lookup")).unwrap();

    assert!(code.contains("fn lookup(cp: u32) -> u8"));
    assert!(code.contains("if cp > 0x10ffff"));
    assert!(code.contains("250 as u8"));
    assert!(code.contains("lookup_packtab_get(cp as usize) as u8"));
}

#[test]
fn generation_omits_error_guard_when_default_matches_error() {
    let trie = sample_trie(9, 9);
    let code = generate_rust_code_from_trie(&trie, GenerateOptions::new("lookup")).unwrap();

    assert!(code.contains("fn lookup(cp: u32) -> u8"));
    assert!(!code.contains("if cp > 0x10ffff"));
    assert!(code.contains("lookup_packtab_get(cp as usize)"));
}

#[test]
fn manual_u64_input_is_rejected() {
    let packed = PackedCodePointTrieInput {
        scalar_data: vec![0u64; UNICODE_LEN],
        error_value: 0u64,
    };

    assert_eq!(
        generate_rust_code(&packed, GenerateOptions::new("lookup")),
        Err(GenerateError::UnsupportedType("u64"))
    );
}
