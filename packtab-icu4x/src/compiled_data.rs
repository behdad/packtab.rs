use crate::{generate_rust_code_from_code_point_map_data_borrowed, GenerateOptions, PacktabValue};
use icu_collections::codepointtrie::TrieValue;
use icu_properties::props::{
    BidiClass, CanonicalCombiningClass, EastAsianWidth, GeneralCategory, LineBreak, Script,
};
use icu_properties::{CodePointMapData, CodePointMapDataBorrowed};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyError {
    UnknownProperty(String),
    Generate(crate::GenerateError),
}

impl fmt::Display for PropertyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProperty(name) => write!(f, "unknown property: {name}"),
            Self::Generate(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for PropertyError {}

impl From<crate::GenerateError> for PropertyError {
    fn from(err: crate::GenerateError) -> Self {
        Self::Generate(err)
    }
}

pub fn supported_properties() -> &'static [&'static str] {
    &["bc", "ccc", "ea", "gc", "lb", "script"]
}

fn generate_property<T>(
    map: CodePointMapDataBorrowed<'static, T>,
    options: GenerateOptions<'_>,
) -> Result<String, PropertyError>
where
    T: TrieValue + PacktabValue,
{
    generate_rust_code_from_code_point_map_data_borrowed(map, options).map_err(PropertyError::from)
}

pub fn generate_rust_code_for_property(
    property: &str,
    options: GenerateOptions<'_>,
) -> Result<String, PropertyError> {
    match property {
        "bc" => generate_property(CodePointMapData::<BidiClass>::new(), options),
        "ccc" => generate_property(
            CodePointMapData::<CanonicalCombiningClass>::new(),
            options,
        ),
        "ea" => generate_property(CodePointMapData::<EastAsianWidth>::new(), options),
        "gc" => generate_property(CodePointMapData::<GeneralCategory>::new(), options),
        "lb" => generate_property(CodePointMapData::<LineBreak>::new(), options),
        "script" => generate_property(CodePointMapData::<Script>::new(), options),
        _ => Err(PropertyError::UnknownProperty(property.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::{generate_rust_code_for_property, supported_properties, PropertyError};
    use crate::GenerateOptions;

    #[test]
    fn supported_property_list_is_stable() {
        assert_eq!(supported_properties(), &["bc", "ccc", "ea", "gc", "lb", "script"]);
    }

    #[test]
    fn unknown_property_is_rejected() {
        assert_eq!(
            generate_rust_code_for_property("nope", GenerateOptions::new("lookup")),
            Err(PropertyError::UnknownProperty("nope".to_string()))
        );
    }

    #[test]
    fn general_category_generation_preserves_wrapper_type() {
        let code = generate_rust_code_for_property("gc", GenerateOptions::new("lookup")).unwrap();

        assert!(code.contains("fn lookup(cp: u32) -> icu_properties::props::GeneralCategory"));
        assert!(code.contains("icu_properties::props::GeneralCategory::from_icu4c_value"));
    }
}
