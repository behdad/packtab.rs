use crate::{generate_rust_code_from_code_point_map_data_borrowed, GenerateOptions, PacktabValue};
use icu_collections::codepointtrie::TrieValue;
use icu_properties::props::{
    BidiClass, CanonicalCombiningClass, EastAsianWidth, GeneralCategory, GraphemeClusterBreak,
    HangulSyllableType, IndicConjunctBreak, IndicSyllabicCategory, JoiningGroup, JoiningType,
    LineBreak, NumericType, Script, SentenceBreak, VerticalOrientation, WordBreak,
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
    &[
        "bc", "nt", "gc", "sc", "script", "hst", "ea", "lb", "gcb", "wb", "sb", "ccc", "incb",
        "insc", "jg", "jt", "vo",
    ]
}

fn normalize_property_name(property: &str) -> String {
    property
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
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
    match normalize_property_name(property).as_str() {
        "bc" => generate_property(CodePointMapData::<BidiClass>::new(), options),
        "bidiclass" => generate_property(CodePointMapData::<BidiClass>::new(), options),
        "nt" | "numerictype" => generate_property(CodePointMapData::<NumericType>::new(), options),
        "ccc" => generate_property(
            CodePointMapData::<CanonicalCombiningClass>::new(),
            options,
        ),
        "canonicalcombiningclass" => generate_property(
            CodePointMapData::<CanonicalCombiningClass>::new(),
            options,
        ),
        "ea" => generate_property(CodePointMapData::<EastAsianWidth>::new(), options),
        "eastasianwidth" => generate_property(CodePointMapData::<EastAsianWidth>::new(), options),
        "gc" => generate_property(CodePointMapData::<GeneralCategory>::new(), options),
        "generalcategory" => generate_property(CodePointMapData::<GeneralCategory>::new(), options),
        "lb" => generate_property(CodePointMapData::<LineBreak>::new(), options),
        "linebreak" => generate_property(CodePointMapData::<LineBreak>::new(), options),
        "sc" | "script" => generate_property(CodePointMapData::<Script>::new(), options),
        "hst" | "hangulsyllabletype" => {
            generate_property(CodePointMapData::<HangulSyllableType>::new(), options)
        }
        "gcb" | "graphemeclusterbreak" => {
            generate_property(CodePointMapData::<GraphemeClusterBreak>::new(), options)
        }
        "wb" | "wordbreak" => generate_property(CodePointMapData::<WordBreak>::new(), options),
        "sb" | "sentencebreak" => {
            generate_property(CodePointMapData::<SentenceBreak>::new(), options)
        }
        "incb" | "indicconjunctbreak" => {
            generate_property(CodePointMapData::<IndicConjunctBreak>::new(), options)
        }
        "insc" | "indicsyllabiccategory" => {
            generate_property(CodePointMapData::<IndicSyllabicCategory>::new(), options)
        }
        "jg" | "joininggroup" => generate_property(CodePointMapData::<JoiningGroup>::new(), options),
        "jt" | "joiningtype" => generate_property(CodePointMapData::<JoiningType>::new(), options),
        "vo" | "verticalorientation" => {
            generate_property(CodePointMapData::<VerticalOrientation>::new(), options)
        }
        _ => Err(PropertyError::UnknownProperty(property.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::{generate_rust_code_for_property, supported_properties, PropertyError};
    use crate::GenerateOptions;

    #[test]
    fn supported_property_list_is_stable() {
        assert_eq!(
            supported_properties(),
            &[
                "bc", "nt", "gc", "sc", "script", "hst", "ea", "lb", "gcb", "wb", "sb", "ccc",
                "incb", "insc", "jg", "jt", "vo",
            ]
        );
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
        assert!(code.contains(
            "<icu_properties::props::GeneralCategory as icu_collections::codepointtrie::TrieValue>::try_from_u32"
        ));
    }

    #[test]
    fn aliases_work_for_script_property() {
        let a = generate_rust_code_for_property("sc", GenerateOptions::new("lookup")).unwrap();
        let b =
            generate_rust_code_for_property("Script", GenerateOptions::new("lookup")).unwrap();
        let c = generate_rust_code_for_property("script", GenerateOptions::new("lookup")).unwrap();

        assert_eq!(a, b);
        assert_eq!(b, c);
    }
}
