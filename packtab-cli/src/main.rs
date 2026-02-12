use clap::Parser;
use packtab::codegen::Language;

#[derive(Parser)]
#[command(name = "packtab", about = "Pack a list of integers into compact lookup tables.")]
struct Cli {
    /// Integer data values to pack.
    #[arg(required = true)]
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
}

fn main() {
    let cli = Cli::parse();

    let language_str = if cli.rust { "rust" } else { &cli.language };
    let lang = match language_str {
        "rust" => Language::Rust {
            unsafe_access: cli.unsafe_access,
        },
        _ => Language::C,
    };

    let (info, best) = packtab::pack_table(&cli.data, cli.default, cli.compression);
    let code = packtab::generate(&info, best, &cli.name, lang);
    print!("{}", code);
}
