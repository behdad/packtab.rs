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
    /// For C: use '1,9' to generate both variants with #ifdef __OPTIMIZE_SIZE__.
    #[arg(long, default_value = "1")]
    compression: String,

    /// Shortcut for --compression 9 (maximum size optimization).
    #[arg(long)]
    optimize_size: bool,

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

    // Handle --optimize-size shortcut
    let compression_str = if cli.optimize_size { "9" } else { &cli.compression };

    // Parse compression value(s)
    let compression_values: Vec<f64> = compression_str
        .split(',')
        .map(|s| s.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|e| {
            eprintln!("Error: invalid compression value: {}", e);
            std::process::exit(1);
        });

    // Validate dual-compression (C-only for now)
    if compression_values.len() > 1 {
        if compression_values.len() != 2 {
            eprintln!("Error: compression can have at most 2 values (e.g., '1,9')");
            std::process::exit(1);
        }
        if language_str != "c" {
            eprintln!("Error: dual compression (e.g., '1,9') is only supported for C output");
            std::process::exit(1);
        }
    }

    let code = if compression_values.len() == 1 {
        // Single compression - generate normally
        let (info, best) = packtab::pack_table(&data, cli.default, compression_values[0]);
        packtab::generate(&info, best, &cli.name, lang)
    } else {
        // Dual compression - generate both with #ifdef
        let (info_speed, best_speed) = packtab::pack_table(&data, cli.default, compression_values[0]);
        let code_speed = packtab::generate(&info_speed, best_speed, &cli.name, lang);

        let (info_size, best_size) = packtab::pack_table(&data, cli.default, compression_values[1]);
        let code_size = packtab::generate(&info_size, best_size, &cli.name, lang);

        format!(
            "#ifdef __OPTIMIZE_SIZE__\n\n{}\n\n#else  /* optimize for speed */\n\n{}\n\n#endif\n",
            code_size, code_speed
        )
    };

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
