//! ICU4X-oriented adapters and experiments built on top of `packtab`.

use icu_collections::codepointtrie::{CodePointTrie, TrieValue};
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
}

impl PacktabValue for u64 {
    fn try_to_i64(self) -> Result<i64, GenerateError> {
        Err(GenerateError::UnsupportedType("u64"))
    }

    fn rust_type_name() -> Result<&'static str, GenerateError> {
        Err(GenerateError::UnsupportedType("u64"))
    }
}

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
            "#[allow(dead_code)]\n#[inline]\npub(crate) fn {}(cp: u32) -> {} {{\n  {}_get(cp as usize)\n}}\n",
            options.name, return_type, namespace
        )
    } else {
        format!(
            "#[allow(dead_code)]\n#[inline]\npub(crate) fn {}(cp: u32) -> {} {{\n  if cp > 0x10ffff {{\n    {} as {}\n  }} else {{\n    {}_get(cp as usize) as {}\n  }}\n}}\n",
            options.name,
            return_type,
            error_value,
            return_type,
            namespace,
            return_type,
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
