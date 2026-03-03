//! ICU4X-oriented adapters and experiments built on top of `packtab`.

#[cfg(feature = "compiled_data")]
pub mod compiled_data;

use icu_collections::codepointtrie::{CodePointTrie, TrieValue};
use icu_properties::{CodePointMapData, CodePointMapDataBorrowed};
use packtab::codegen::Language;
use std::fmt;

/// Highest valid Unicode scalar value.
pub const UNICODE_MAX: u32 = 0x10_FFFF;

/// Number of valid Unicode scalar values.
pub const UNICODE_LEN: usize = (UNICODE_MAX as usize) + 1;

/// Dense scalar table plus the out-of-range value from an ICU4X code point trie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedCodePointTrieInput<T> {
    /// Dense values for all valid Unicode scalar values `0..=0x10_FFFF`.
    pub scalar_data: Vec<T>,
    /// Value returned for out-of-range code points.
    pub error_value: T,
}

/// Code generation options for a packed CodePointTrie lookup.
#[derive(Debug, Clone, Copy)]
pub struct GenerateOptions<'a> {
    /// Public wrapper function name.
    pub name: &'a str,
    /// Size/speed tradeoff passed through to `packtab`.
    pub compression: f64,
    /// Explicit default for `packtab`; `None` tries both boundary values.
    pub default_value: Option<i64>,
    /// Whether generated Rust should use unchecked array indexing internally.
    pub unsafe_access: bool,
}

impl<'a> GenerateOptions<'a> {
    /// Create generation options with the given wrapper name.
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            compression: 1.0,
            default_value: None,
            unsafe_access: false,
        }
    }
}

/// Error returned when `packtab-icu4x` cannot lower a value type into `packtab`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerateError {
    UnsupportedType(&'static str),
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedType(ty) => write!(f, "unsupported value type for packtab codegen: {ty}"),
        }
    }
}

impl std::error::Error for GenerateError {}

/// Value types that `packtab-icu4x` can lower into `packtab`.
pub trait PacktabValue: Copy + 'static {
    fn try_to_i64(self) -> Result<i64, GenerateError>;
    fn rust_type_name() -> Result<&'static str, GenerateError>;
    fn render_literal(value: i64) -> Result<String, GenerateError>;
    fn render_expr(expr: &str) -> Result<String, GenerateError>;
}

macro_rules! impl_packtab_value {
    ($ty:ty, $rust_name:literal) => {
        impl PacktabValue for $ty {
            fn try_to_i64(self) -> Result<i64, GenerateError> {
                Ok(self as i64)
            }

            fn rust_type_name() -> Result<&'static str, GenerateError> {
                Ok($rust_name)
            }

            fn render_literal(value: i64) -> Result<String, GenerateError> {
                Ok(format!("{value} as {}", $rust_name))
            }

            fn render_expr(expr: &str) -> Result<String, GenerateError> {
                Ok(format!("{expr} as {}", $rust_name))
            }
        }
    };
}

impl_packtab_value!(u8, "u8");
impl_packtab_value!(u16, "u16");
impl_packtab_value!(u32, "u32");
impl_packtab_value!(i8, "i8");
impl_packtab_value!(i16, "i16");
impl_packtab_value!(i32, "i32");

impl PacktabValue for char {
    fn try_to_i64(self) -> Result<i64, GenerateError> {
        Ok(self as u32 as i64)
    }

    fn rust_type_name() -> Result<&'static str, GenerateError> {
        Ok("char")
    }

    fn render_literal(value: i64) -> Result<String, GenerateError> {
        Ok(format!("char::from_u32({value}u32).unwrap()"))
    }

    fn render_expr(expr: &str) -> Result<String, GenerateError> {
        Ok(format!("char::from_u32(({expr}) as u32).unwrap()"))
    }
}

impl PacktabValue for u64 {
    fn try_to_i64(self) -> Result<i64, GenerateError> {
        Err(GenerateError::UnsupportedType("u64"))
    }

    fn rust_type_name() -> Result<&'static str, GenerateError> {
        Err(GenerateError::UnsupportedType("u64"))
    }

    fn render_literal(_value: i64) -> Result<String, GenerateError> {
        Err(GenerateError::UnsupportedType("u64"))
    }

    fn render_expr(_expr: &str) -> Result<String, GenerateError> {
        Err(GenerateError::UnsupportedType("u64"))
    }
}

#[cfg(feature = "compiled_data")]
macro_rules! impl_packtab_value_via_to_u32 {
    ($ty:ty, $rust_name:literal) => {
        impl PacktabValue for $ty {
            fn try_to_i64(self) -> Result<i64, GenerateError> {
                Ok(self.to_u32() as i64)
            }

            fn rust_type_name() -> Result<&'static str, GenerateError> {
                Ok($rust_name)
            }

            fn render_literal(value: i64) -> Result<String, GenerateError> {
                Ok(format!(
                    "<{} as icu_collections::codepointtrie::TrieValue>::try_from_u32({value} as u32).unwrap()",
                    $rust_name
                ))
            }

            fn render_expr(expr: &str) -> Result<String, GenerateError> {
                Ok(format!(
                    "<{} as icu_collections::codepointtrie::TrieValue>::try_from_u32(({expr}) as u32).unwrap()",
                    $rust_name
                ))
            }
        }
    };
}

#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::BidiClass,
    "icu_properties::props::BidiClass"
);
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::CanonicalCombiningClass,
    "icu_properties::props::CanonicalCombiningClass"
);
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::EastAsianWidth,
    "icu_properties::props::EastAsianWidth"
);
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::GeneralCategory,
    "icu_properties::props::GeneralCategory"
);
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::LineBreak,
    "icu_properties::props::LineBreak"
);
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(icu_properties::props::Script, "icu_properties::props::Script");
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(icu_properties::props::NumericType, "icu_properties::props::NumericType");
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::HangulSyllableType,
    "icu_properties::props::HangulSyllableType"
);
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::GraphemeClusterBreak,
    "icu_properties::props::GraphemeClusterBreak"
);
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(icu_properties::props::WordBreak, "icu_properties::props::WordBreak");
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(icu_properties::props::SentenceBreak, "icu_properties::props::SentenceBreak");
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::IndicConjunctBreak,
    "icu_properties::props::IndicConjunctBreak"
);
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::IndicSyllabicCategory,
    "icu_properties::props::IndicSyllabicCategory"
);
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(icu_properties::props::JoiningGroup, "icu_properties::props::JoiningGroup");
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(icu_properties::props::JoiningType, "icu_properties::props::JoiningType");
#[cfg(feature = "compiled_data")]
impl_packtab_value_via_to_u32!(
    icu_properties::props::VerticalOrientation,
    "icu_properties::props::VerticalOrientation"
);

/// Flatten an ICU4X `CodePointTrie` into dense scalar data for `packtab`.
pub fn flatten_code_point_trie<T>(trie: &CodePointTrie<'_, T>) -> PackedCodePointTrieInput<T>
where
    T: TrieValue + PacktabValue,
{
    let mut scalar_data = vec![trie.error_value(); UNICODE_LEN];

    for range in trie.iter_ranges() {
        let start = (*range.range.start()).min(UNICODE_MAX);
        let end = (*range.range.end()).min(UNICODE_MAX);
        if start > end {
            continue;
        }

        scalar_data[start as usize..=end as usize].fill(range.value);
    }

    PackedCodePointTrieInput {
        scalar_data,
        error_value: trie.error_value(),
    }
}

/// Flatten an ICU4X `CodePointMapData` into dense scalar data for `packtab`.
pub fn flatten_code_point_map_data<T>(map: &CodePointMapData<T>) -> PackedCodePointTrieInput<T>
where
    T: TrieValue + PacktabValue,
{
    if let Some(trie) = map.as_code_point_trie() {
        flatten_code_point_trie(trie)
    } else {
        let trie = map.to_code_point_trie();
        flatten_code_point_trie(&trie)
    }
}

/// Flatten a borrowed ICU4X `CodePointMapDataBorrowed` into dense scalar data for `packtab`.
pub fn flatten_code_point_map_data_borrowed<T>(
    map: CodePointMapDataBorrowed<'_, T>,
) -> PackedCodePointTrieInput<T>
where
    T: TrieValue + PacktabValue,
{
    let mut scalar_data = vec![map.get32(0); UNICODE_LEN];

    for range in map.iter_ranges() {
        let start = (*range.range.start()).min(UNICODE_MAX);
        let end = (*range.range.end()).min(UNICODE_MAX);
        if start > end {
            continue;
        }

        scalar_data[start as usize..=end as usize].fill(range.value);
    }

    PackedCodePointTrieInput {
        scalar_data,
        error_value: map.get32(u32::MAX),
    }
}

/// Generate Rust code for a dense packed lookup derived from an ICU4X trie.
pub fn generate_rust_code<T>(
    input: &PackedCodePointTrieInput<T>,
    options: GenerateOptions<'_>,
) -> Result<String, GenerateError>
where
    T: PacktabValue,
{
    let namespace = format!("{}_packtab", options.name);
    let scalar_data: Vec<i64> = input
        .scalar_data
        .iter()
        .copied()
        .map(PacktabValue::try_to_i64)
        .collect::<Result<_, _>>()?;
    let error_value = input.error_value.try_to_i64()?;
    let return_type = T::rust_type_name()?;
    let error_literal = T::render_literal(error_value)?;
    let wrapped_lookup_expr = T::render_expr(&format!("{}_get(cp as usize)", namespace))?;
    let (info, best) = packtab::pack_table(&scalar_data, options.default_value, options.compression);
    let mut code = packtab::generate(
        &info,
        best,
        &namespace,
        Language::Rust {
            unsafe_access: options.unsafe_access,
        },
    );

    let wrapper = if error_value == info.default {
        format!(
            "#[allow(dead_code)]\n#[inline]\npub(crate) fn {}(cp: u32) -> {} {{\n  {}\n}}\n",
            options.name, return_type, wrapped_lookup_expr
        )
    } else {
        format!(
            "#[allow(dead_code)]\n#[inline]\npub(crate) fn {}(cp: u32) -> {} {{\n  if cp > 0x10ffff {{\n    {}\n  }} else {{\n    {}\n  }}\n}}\n",
            options.name,
            return_type,
            error_literal,
            wrapped_lookup_expr,
        )
    };

    if !code.ends_with('\n') {
        code.push('\n');
    }
    code.push('\n');
    code.push_str(&wrapper);
    Ok(code)
}

/// Flatten an ICU4X trie and immediately generate Rust code for it.
pub fn generate_rust_code_from_trie<T>(
    trie: &CodePointTrie<'_, T>,
    options: GenerateOptions<'_>,
) -> Result<String, GenerateError>
where
    T: TrieValue + PacktabValue,
{
    let input = flatten_code_point_trie(trie);
    generate_rust_code(&input, options)
}

/// Flatten an ICU4X `CodePointMapData` and immediately generate Rust code for it.
pub fn generate_rust_code_from_code_point_map_data<T>(
    map: &CodePointMapData<T>,
    options: GenerateOptions<'_>,
) -> Result<String, GenerateError>
where
    T: TrieValue + PacktabValue,
{
    let input = flatten_code_point_map_data(map);
    generate_rust_code(&input, options)
}

/// Flatten borrowed ICU4X code point map data and immediately generate Rust code for it.
pub fn generate_rust_code_from_code_point_map_data_borrowed<T>(
    map: CodePointMapDataBorrowed<'_, T>,
    options: GenerateOptions<'_>,
) -> Result<String, GenerateError>
where
    T: TrieValue + PacktabValue,
{
    let input = flatten_code_point_map_data_borrowed(map);
    generate_rust_code(&input, options)
}

#[cfg(test)]
mod tests {
    use super::{
        flatten_code_point_map_data_borrowed, generate_rust_code,
        generate_rust_code_from_code_point_map_data_borrowed, GenerateError, GenerateOptions,
        PackedCodePointTrieInput, PacktabValue,
    };
    use icu_codepointtrie_builder::CodePointTrieBuilder;
    use icu_collections::codepointtrie::TrieType;
    use icu_properties::CodePointMapData;

    fn sample_map(default_value: u8, error_value: u8) -> CodePointMapData<u8> {
        let mut builder = CodePointTrieBuilder::new(default_value, error_value, TrieType::Small);
        builder.set_range_value('A' as u32..='Z' as u32, 1);
        builder.set_range_value('a' as u32..='z' as u32, 2);
        CodePointMapData::from_code_point_trie(builder.build())
    }

    #[test]
    fn generate_rust_code_uses_explicit_default_override() {
        let input = PackedCodePointTrieInput {
            scalar_data: vec![7u8, 1, 2, 7],
            error_value: 99u8,
        };
        let options = GenerateOptions {
            name: "lookup",
            compression: 1.0,
            default_value: Some(7),
            unsafe_access: false,
        };
        let code = generate_rust_code(&input, options).unwrap();

        assert!(code.contains("fn lookup(cp: u32) -> u8"));
        assert!(code.contains("99 as u8"));
    }

    #[test]
    fn generate_rust_code_uses_original_value_type_in_wrapper() {
        let input = PackedCodePointTrieInput {
            scalar_data: vec![0u16, 1, 2, 0],
            error_value: 500u16,
        };
        let code = generate_rust_code(&input, GenerateOptions::new("lookup")).unwrap();

        assert!(code.contains("fn lookup(cp: u32) -> u16"));
        assert!(code.contains("500 as u16"));
    }

    #[test]
    fn packtab_value_reports_char_type_name() {
        assert_eq!(<char as PacktabValue>::rust_type_name().unwrap(), "char");
    }

    #[test]
    fn packtab_value_rejects_u64() {
        assert_eq!(
            <u64 as PacktabValue>::rust_type_name(),
            Err(GenerateError::UnsupportedType("u64"))
        );
        assert_eq!(
            42u64.try_to_i64(),
            Err(GenerateError::UnsupportedType("u64"))
        );
    }

    #[test]
    fn flatten_borrowed_code_point_map_data_preserves_values() {
        let map = sample_map(9, 250);
        let packed = flatten_code_point_map_data_borrowed(map.as_borrowed());

        assert_eq!(packed.scalar_data['A' as usize], 1);
        assert_eq!(packed.scalar_data['a' as usize], 2);
        assert_eq!(packed.scalar_data[0], 9);
        assert_eq!(packed.error_value, 250);
    }

    #[test]
    fn generate_from_borrowed_code_point_map_data_works() {
        let map = sample_map(9, 250);
        let code = generate_rust_code_from_code_point_map_data_borrowed(
            map.as_borrowed(),
            GenerateOptions::new("lookup"),
        )
        .unwrap();

        assert!(code.contains("fn lookup(cp: u32) -> u8"));
        assert!(code.contains("250 as u8"));
    }
}
