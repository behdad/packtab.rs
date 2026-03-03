use icu_collections::codepointtrie::TrieValue;
use icu_properties::props::{GeneralCategory, Script};
use icu_properties::CodePointMapData;
use packtab::codegen::Language;
use std::env;
use std::fs;
use std::path::PathBuf;

fn generate_for_property<T>(
    name: &str,
    map: icu_properties::CodePointMapDataBorrowed<'static, T>,
    compression: f64,
    unsafe_access: bool,
) -> String
where
    T: TrieValue,
{
    let mut scalar_data = vec![map.get32(0).to_u32() as i64; 0x110000];
    for range in map.iter_ranges() {
        let start = (*range.range.start()).min(0x10_FFFF);
        let end = (*range.range.end()).min(0x10_FFFF);
        if start > end {
            continue;
        }
        scalar_data[start as usize..=end as usize].fill(range.value.to_u32() as i64);
    }

    let error_value = map.get32(u32::MAX).to_u32() as i64;
    let (info, best) = packtab::pack_table(&scalar_data, None, compression);
    let inner = packtab::generate(
        &info,
        best,
        &format!("{name}_packtab"),
        Language::Rust { unsafe_access },
    );
    let wrapper = format!(
        "#[allow(dead_code)]\n#[inline]\npub(crate) fn {name}(cp: u32) -> u32 {{\n  if cp > 0x10ffff {{\n    {error_value}u32\n  }} else {{\n    {name}_packtab_get(cp as usize) as u32\n  }}\n}}\n"
    );
    format!("{inner}\n{wrapper}")
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let generated = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        generate_for_property("gc_lookup", CodePointMapData::<GeneralCategory>::new(), 1.0, false),
        generate_for_property(
            "gc_lookup_unsafe",
            CodePointMapData::<GeneralCategory>::new(),
            1.0,
            true,
        ),
        generate_for_property("gc_lookup_c5", CodePointMapData::<GeneralCategory>::new(), 5.0, false),
        generate_for_property(
            "gc_lookup_c5_unsafe",
            CodePointMapData::<GeneralCategory>::new(),
            5.0,
            true,
        ),
        generate_for_property("gc_lookup_c9", CodePointMapData::<GeneralCategory>::new(), 9.0, false),
        generate_for_property(
            "gc_lookup_c9_unsafe",
            CodePointMapData::<GeneralCategory>::new(),
            9.0,
            true,
        ),
        generate_for_property("script_lookup", CodePointMapData::<Script>::new(), 1.0, false),
        generate_for_property(
            "script_lookup_unsafe",
            CodePointMapData::<Script>::new(),
            1.0,
            true,
        ),
        generate_for_property("script_lookup_c5", CodePointMapData::<Script>::new(), 5.0, false),
        generate_for_property(
            "script_lookup_c5_unsafe",
            CodePointMapData::<Script>::new(),
            5.0,
            true,
        ),
        generate_for_property("script_lookup_c9", CodePointMapData::<Script>::new(), 9.0, false),
        generate_for_property(
            "script_lookup_c9_unsafe",
            CodePointMapData::<Script>::new(),
            9.0,
            true,
        ),
    );
    fs::write(out_dir.join("generated_benches.rs"), generated).unwrap();
}
