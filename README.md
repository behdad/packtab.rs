# packtab

Pack static integer tables into compact multi-level lookup tables.

Rust port of the Python [packTab](https://github.com/nicolo-ribaudo/packTab) library.

## Overview

Given a flat array of integer values (e.g. Unicode character properties
indexed by codepoint), this library compresses it into a set of smaller
arrays plus a lookup function that reconstructs any original value.

The core idea is a multi-level table decomposition. A single flat lookup
`table[index]` is replaced by a chain of smaller lookups:

```
level2[level1[level0[index >> K] + (index & mask)]]
```

The algorithm applies arithmetic reductions (bias subtraction, GCD
factoring, identity subtraction) before the multi-level split to shrink
the value range, enabling tighter sub-byte packing. A Pareto-optimal
set of solutions is computed, trading off table size vs. lookup speed.

Generated code can target **C** or **Rust**.

## Crates

| Crate | Description |
|-------|-------------|
| `packtab` | Core library: packing algorithm and code generation |
| `packtab-cli` | Command-line tool for generating C/Rust lookup tables |
| `packtab-macro` | Proc-macro for compile-time table generation in Rust |

## CLI Usage

### Installation

```sh
cargo install packtab-cli
```

### Examples

```sh
# Generate C code
packtab 1 2 3 4 5 6 7 8

# Generate Rust code
packtab --rust 1 2 3 4 5 6 7 8

# With options
packtab --language rust --unsafe --name my_table --default 0 --compression 2.0 \
    0 128 256 384
```

### Options

```
Arguments:
  <DATA>...              Integer data values to pack

Options:
      --language <LANGUAGE>    Output language [default: c] [possible values: c, rust]
      --rust                   Shorthand for --language=rust
      --unsafe                 Use unsafe array access (Rust only)
      --default <DEFAULT>      Default value for out-of-range indices [default: 0]
      --compression <COMPRESSION>  Size vs speed tradeoff; higher = smaller tables [default: 1]
      --name <NAME>            Namespace prefix for generated symbols [default: data]
```

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
packtab = "0.1.0"
```

Example:

```rust
use packtab::codegen::Language;

let data: Vec<i64> = (0..256).map(|i| (i * 7) % 256).collect();
let (info, best) = packtab::pack_table(&data, 0, 1.0);
let code = packtab::generate(&info, best, "my_table", Language::C);
println!("{}", code);
```

## Proc-Macro Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
packtab-macro = "0.1.0"
```

Example:

```rust
packtab_macro::pack_table! {
    pub fn lookup(u: usize) -> u8 {
        data: [10, 20, 30, 40, 50, 60, 70, 80],
        default: 0,
    }
}

// Now `lookup(3)` returns `40`, `lookup(100)` returns `0`.
```

The macro expands at compile time into packed static arrays and an
inline lookup function.

### Options

- **`default`** (required): Value returned for out-of-range indices.
- **`compression`** (optional): Size vs speed tradeoff; higher = smaller tables. Default `1.0`.
- **`unsafe`** (optional): When `true`, uses `get_unchecked` for array accesses. Default `false`.

```rust
packtab_macro::pack_table! {
    pub fn lookup(u: usize) -> u8 {
        data: [10, 20, 30, 40, 50, 60, 70, 80],
        default: 0,
        compression: 2.0,
        unsafe: true,
    }
}
```

## Algorithm

The algorithm has two layers:

**OuterLayer** applies arithmetic reductions before splitting:
- **Bias subtraction**: if all values >= B, subtract B and add it back
  in generated code.
- **GCD factoring**: if all values share a common factor M, divide them
  out and multiply back.
- **Identity subtraction**: for near-linear data (data[i] ~ i), store
  residuals data[i] - i.
- **Bake-in optimizations**: if the mult/bias can be folded into the
  stored values without widening the integer type, do so to save a
  runtime operation.

**InnerLayer** is the recursive binary-split engine:
- Considers every possible split depth: use data as-is (1 lookup), or
  split the index in half, grouping adjacent pairs into a mapping.
- Splitting recurses, producing 2-level, 3-level, etc. solutions.
- Sub-byte packing (1/2/4 bits per value) for small value ranges.
- Inlining: arrays <= 64 bits are replaced by integer constants.

Solutions are pruned to the Pareto frontier on (lookups, storage cost).

## License

Apache License, Version 2.0. See [LICENSE](LICENSE).
