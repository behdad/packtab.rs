use packtab_icu4x::compiled_data::{generate_rust_code_for_property, supported_properties};
use packtab_icu4x::GenerateOptions;
use std::env;
use std::process;

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} <property> <name> [compression]\nSupported properties: {}\n",
        supported_properties().join(", ")
    )
}

fn main() {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "packtab-icu4x-gen".to_string());
    let property = match args.next() {
        Some(value) => value,
        None => {
            eprint!("{}", usage(&program));
            process::exit(2);
        }
    };
    let name = match args.next() {
        Some(value) => value,
        None => {
            eprint!("{}", usage(&program));
            process::exit(2);
        }
    };
    let compression = match args.next() {
        Some(value) => match value.parse::<f64>() {
            Ok(parsed) => parsed,
            Err(_) => {
                eprintln!("invalid compression value: {value}");
                process::exit(2);
            }
        },
        None => 1.0,
    };

    let options = GenerateOptions {
        name: &name,
        compression,
        default_value: None,
        unsafe_access: false,
    };
    match generate_rust_code_for_property(&property, options) {
        Ok(code) => print!("{code}"),
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    }
}
