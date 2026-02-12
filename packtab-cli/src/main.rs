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

    /// Show compression statistics instead of generating code.
    #[arg(long)]
    analyze: bool,

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

    // Handle --analyze flag
    if cli.analyze {
        print_analysis(&data, cli.default, compression_values[0]);
        return;
    }

    let language_str = if cli.rust { "rust" } else { &cli.language };
    let lang = match language_str {
        "rust" => Language::Rust {
            unsafe_access: cli.unsafe_access,
        },
        _ => Language::C,
    };

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

fn print_analysis(data: &[i64], default: i64, compression: f64) {
    use packtab::util::binary_bits_for;

    let info = packtab::pack_table_all(data, default);

    let original_size = data.len();
    let min_v = *data.iter().min().unwrap_or(&0);
    let max_v = *data.iter().max().unwrap_or(&0);
    let bits_needed = binary_bits_for(min_v, max_v);
    let original_bytes = original_size * std::cmp::max(1, (bits_needed as usize + 7) / 8);

    println!("Compression Analysis");
    println!("{}", "=".repeat(70));
    println!("Original data: {} values, range [{}..{}]", original_size, min_v, max_v);
    println!(
        "Original storage: {} bits/value, {} bytes total",
        bits_needed, original_bytes
    );
    println!("Default value: {}", default);
    println!();
    println!("Found {} Pareto-optimal solutions:", info.solutions.len());
    println!();
    println!(
        "{:<3} {:<8} {:<9} {:<6} {:<8} {:<7} {:<8}",
        "#", "Lookups", "ExtraOps", "Bytes", "FullCost", "Ratio", "Score"
    );
    println!("{}", "-".repeat(70));

    for (i, sol) in info.solutions.iter().enumerate() {
        let ratio = if sol.cost > 0 {
            original_bytes as f64 / sol.cost as f64
        } else {
            f64::INFINITY
        };
        let full_cost = sol.full_cost();
        let score = if full_cost > 0 {
            sol.n_lookups as f64 + compression * ((full_cost as u64).ilog2() as f64)
        } else {
            sol.n_lookups as f64 - compression
        };
        println!(
            "{:<3} {:<8} {:<9} {:<6} {:<8} {:>6.2}x {:>7.1}",
            i + 1,
            sol.n_lookups,
            sol.n_extra_ops,
            sol.cost,
            full_cost,
            ratio,
            score
        );
    }

    println!();
    let chosen_idx = packtab::pick_solution(&info.solutions, compression);
    let chosen = &info.solutions[chosen_idx];
    println!("Best solution for compression={}: #{}", compression, chosen_idx + 1);
    println!(
        "  {} lookups, {} extra ops, {} bytes",
        chosen.n_lookups, chosen.n_extra_ops, chosen.cost
    );
    if chosen.cost > 0 {
        println!(
            "  Compression ratio: {:.2}x",
            original_bytes as f64 / chosen.cost as f64
        );
    } else {
        println!("  Compression ratio: ∞ (computed inline, no storage)");
    }
}
