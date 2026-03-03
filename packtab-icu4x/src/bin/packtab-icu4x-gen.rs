use packtab_icu4x::compiled_data::{generate_rust_code_for_property, supported_properties};
use packtab_icu4x::GenerateOptions;
use std::env;
use std::process;

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} <property> <name> [compression] [--unsafe]\nSupported properties: {}\n",
        supported_properties().join(", ")
    )
}

fn main() {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "packtab-icu4x-gen".to_string());
    let mut args: Vec<String> = args.collect();
    if args.first().is_some_and(|arg| arg == "--") {
        args.remove(0);
    }
    let mut args = args.into_iter();

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
    let mut compression = 1.0;
    let mut unsafe_access = false;
    for arg in args {
        if arg == "--unsafe" {
            unsafe_access = true;
            continue;
        }
        if compression != 1.0 {
            eprintln!("unexpected argument: {arg}");
            process::exit(2);
        }
        compression = match arg.parse::<f64>() {
            Ok(parsed) => parsed,
            Err(_) => {
                eprintln!("invalid compression value: {arg}");
                process::exit(2);
            }
        };
    }

    let options = GenerateOptions {
        name: &name,
        compression,
        default_value: None,
        unsafe_access,
    };
    match generate_rust_code_for_property(&property, options) {
        Ok(code) => print!("{code}"),
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    }
}
