use clap::Parser;
use packtab::codegen::Language;
use std::fs;
use std::io::{self, Read};

#[derive(Parser)]
#[command(name = "packtab", about = "Pack a list of integers into compact lookup tables.")]
struct Cli {
    /// Integer data values to pack (reads from stdin if not provided).
    #[arg(required = false)]
    data: Vec<i64>,

    /// Output language.
    #[arg(long, value_parser = ["c", "rust"], default_value = "c")]
    language: String,

    /// Shorthand for --language=rust.
    #[arg(long)]
    rust: bool,

    /// Use unsafe array access (Rust only).
    #[arg(long, name = "unsafe")]
    unsafe_access: bool,

    /// Default value for out-of-range indices.
    #[arg(long, default_value = "0")]
    default: i64,

    /// Size vs speed tradeoff; higher = smaller tables.
    #[arg(long, default_value = "1")]
    compression: f64,

    /// Namespace prefix for generated symbols.
    #[arg(long, default_value = "data")]
    name: String,

    /// Read data from FILE (default: positional args or stdin).
    #[arg(short = 'i', long = "input")]
    input: Option<String>,

    /// Write output to FILE instead of stdout.
    #[arg(short = 'o', long = "output")]
    output: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    // Read data from input file, positional args, or stdin
    let data = if let Some(input_file) = &cli.input {
        // Read from input file
        let content = fs::read_to_string(input_file)
            .unwrap_or_else(|e| {
                eprintln!("Error reading input file '{}': {}", input_file, e);
                std::process::exit(1);
            });
        parse_data(&content)
    } else if !cli.data.is_empty() {
        // Use positional args
        cli.data.clone()
    } else {
        // Read from stdin
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .unwrap_or_else(|e| {
                eprintln!("Error reading from stdin: {}", e);
                std::process::exit(1);
            });
        parse_data(&buffer)
    };

    if data.is_empty() {
        eprintln!("Error: no data provided (use positional args, -i, or stdin)");
        std::process::exit(1);
    }

    let language_str = if cli.rust { "rust" } else { &cli.language };
    let lang = match language_str {
        "rust" => Language::Rust {
            unsafe_access: cli.unsafe_access,
        },
        _ => Language::C,
    };

    let (info, best) = packtab::pack_table(&data, cli.default, cli.compression);
    let code = packtab::generate(&info, best, &cli.name, lang);

    // Write to output file or stdout
    if let Some(output_file) = &cli.output {
        fs::write(output_file, &code)
            .unwrap_or_else(|e| {
                eprintln!("Error writing to output file '{}': {}", output_file, e);
                std::process::exit(1);
            });
    } else {
        print!("{}", code);
    }
}

fn parse_data(content: &str) -> Vec<i64> {
    content
        .split_whitespace()
        .filter_map(|s| s.parse::<i64>().ok())
        .collect()
}
