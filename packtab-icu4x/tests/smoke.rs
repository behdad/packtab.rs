use icu_codepointtrie_builder::CodePointTrieBuilder;
use icu_collections::codepointtrie::{CodePointTrie, TrieType};
use icu_properties::CodePointMapData;
use packtab_icu4x::{
    flatten_code_point_map_data, flatten_code_point_map_data_borrowed, flatten_code_point_trie,
    generate_rust_code, generate_rust_code_from_code_point_map_data,
    generate_rust_code_from_code_point_map_data_borrowed, generate_rust_code_from_trie,
    GenerateError, GenerateOptions, PackedCodePointTrieInput, UNICODE_LEN,
};

fn sample_trie(default_value: u8, error_value: u8) -> CodePointTrie<'static, u8> {
    let mut builder = CodePointTrieBuilder::new(default_value, error_value, TrieType::Small);
    builder.set_range_value('A' as u32..='Z' as u32, 1);
    builder.set_range_value('a' as u32..='z' as u32, 2);
    builder.set_range_value(0x1F600..=0x1F64F, 3);
    builder.build()
}

fn edge_trie(default_value: u16, error_value: u16) -> CodePointTrie<'static, u16> {
    let mut builder = CodePointTrieBuilder::new(default_value, error_value, TrieType::Small);
    builder.set_value(0, 1);
    builder.set_value(char::MAX as u32, 2);
    builder.set_range_value(0x80..=0x8F, 3);
    builder.build()
}

fn sample_map(default_value: u8, error_value: u8) -> CodePointMapData<u8> {
    CodePointMapData::from_code_point_trie(sample_trie(default_value, error_value))
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
fn flatten_matches_trie_queries_at_range_boundaries() {
    let trie = edge_trie(99, 777);
    let packed = flatten_code_point_trie(&trie);
    let cps = [0, 1, 0x7F, 0x80, 0x8F, 0x90, char::MAX as u32];

    for cp in cps {
        assert_eq!(packed.scalar_data[cp as usize], trie.get32(cp));
    }
}

#[test]
fn flatten_covers_unicode_endpoints() {
    let trie = edge_trie(99, 777);
    let packed = flatten_code_point_trie(&trie);

    assert_eq!(packed.scalar_data[0], 1);
    assert_eq!(packed.scalar_data[char::MAX as usize], 2);
    assert_eq!(packed.scalar_data[1], 99);
}

#[test]
fn generation_respects_explicit_default_value() {
    let trie = sample_trie(9, 250);
    let packed = flatten_code_point_trie(&trie);
    let options = GenerateOptions {
        name: "lookup",
        compression: 1.0,
        default_value: Some(9),
        unsafe_access: false,
    };
    let code = generate_rust_code(&packed, options).unwrap();

    assert!(code.contains("fn lookup(cp: u32) -> u8"));
    assert!(code.contains("250 as u8"));
}

#[test]
fn owned_code_point_map_data_flattens_like_trie() {
    let trie = sample_trie(9, 250);
    let map = sample_map(9, 250);

    assert_eq!(flatten_code_point_map_data(&map), flatten_code_point_trie(&trie));
}

#[test]
fn borrowed_code_point_map_data_flattens_like_owned() {
    let map = sample_map(9, 250);

    assert_eq!(
        flatten_code_point_map_data_borrowed(map.as_borrowed()),
        flatten_code_point_map_data(&map)
    );
}

#[test]
fn generation_from_owned_code_point_map_data_works() {
    let map = sample_map(9, 250);
    let code = generate_rust_code_from_code_point_map_data(&map, GenerateOptions::new("lookup"))
        .unwrap();

    assert!(code.contains("fn lookup(cp: u32) -> u8"));
    assert!(code.contains("250 as u8"));
}

#[test]
fn generation_from_borrowed_code_point_map_data_works() {
    let map = sample_map(9, 250);
    let code = generate_rust_code_from_code_point_map_data_borrowed(
        map.as_borrowed(),
        GenerateOptions::new("lookup"),
    )
    .unwrap();

    assert!(code.contains("fn lookup(cp: u32) -> u8"));
    assert!(code.contains("250 as u8"));
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
