# packtab-icu4x

Experimental ICU4X adapters, generators, and measurement harnesses built on top of `packtab`.

## Scope

This crate currently focuses on Unicode property maps backed by ICU4X `CodePointTrie` / `CodePointMapData`.

It supports:

- flattening ICU4X tries and property maps into `packtab` input
- generating Rust lookup code from those maps
- benchmarking ICU4X lookups against generated `packtab` lookups
- measuring per-property release binary section sizes for the generated code

## Generator

Generate Rust for a compiled ICU4X property:

```sh
cargo run -p packtab-icu4x --features compiled_data --bin packtab-icu4x-gen -- gc gc_lookup
```

Supported property names shown by the tool:

- `bc`
- `nt`
- `gc`
- `sc`
- `script`
- `hst`
- `ea`
- `lb`
- `gcb`
- `wb`
- `sb`
- `ccc`
- `incb`
- `insc`
- `jg`
- `jt`
- `vo`

The generator also accepts normalized long-form names by stripping non-alphanumeric
characters and lowercasing first. Current long-form aliases are:

- `Bidi_Class`
- `Numeric_Type`
- `Canonical_Combining_Class`
- `East_Asian_Width`
- `General_Category`
- `Line_Break`
- `Hangul_Syllable_Type`
- `Grapheme_Cluster_Break`
- `Word_Break`
- `Sentence_Break`
- `Indic_Conjunct_Break`
- `Indic_Syllabic_Category`
- `Joining_Group`
- `Joining_Type`
- `Vertical_Orientation`

Options:

- positional `compression` after the output name, default `1.0`
- `--unsafe` to request Rust `get_unchecked()` array access

Example:

```sh
cargo run -p packtab-icu4x --features compiled_data --bin packtab-icu4x-gen -- gc gc_lookup 9 --unsafe
```

## Speed Benchmarking

Criterion benchmark:

```sh
cargo bench -p packtab-icu4x --features compiled_data --bench properties
```

The benchmark currently compares:

- empty loop baselines
- ICU4X baseline
- `packtab` at compression `1`
- `packtab` at compression `5`
- `packtab` at compression `9`
- safe and unsafe generated Rust for each `packtab` configuration

Bench corpus:

- ASCII
- stepped BMP values
- stepped astral values
- one out-of-range code point

### Current timings

Release-mode Criterion run on this machine:

| Benchmark | Time |
|---|---:|
| `empty/xor-cp` | `70.676..70.904 ns` |
| `empty/xor-black-box-cp` | `649.21..653.46 ns` |
| `gc/icu4x` | `3.8399..3.8602 us` |
| `gc/packtab` | `2.0201..2.0222 us` |
| `gc/packtab-unsafe` | `1.7228..1.7426 us` |
| `gc/packtab-c5` | `3.2767..3.2971 us` |
| `gc/packtab-c5-unsafe` | `2.7991..2.8095 us` |
| `gc/packtab-c9` | `3.2678..3.2835 us` |
| `gc/packtab-c9-unsafe` | `2.8355..2.9170 us` |
| `script/icu4x` | `3.7026..3.7193 us` |
| `script/packtab` | `1.3136..1.3201 us` |
| `script/packtab-unsafe` | `1.2250..1.2307 us` |
| `script/packtab-c5` | `2.5525..2.5550 us` |
| `script/packtab-c5-unsafe` | `2.0138..2.0279 us` |
| `script/packtab-c9` | `3.0107..3.0266 us` |
| `script/packtab-c9-unsafe` | `2.5198..2.5368 us` |

Takeaways from this run:

- `packtab` compression `1` is substantially faster than ICU4X for both `gc` and `script`
- `unsafe` improves the `packtab` path further
- compression `5` and `9` trade speed away for size
- on these two properties, compression `5` and `9` are very close for `gc`; for `script`, `9` is smaller but slower than `5`

Using `empty/xor-black-box-cp` as the runtime baseline, approximate lookup-overhead deltas are:

- `gc`:
  - ICU4X: about `3.20 us`
  - packtab c1: about `1.38 us`
  - packtab c5: about `2.61 us`
  - packtab c9: about `2.61 us`
- `script`:
  - ICU4X: about `3.05 us`
  - packtab c1: about `0.66 us`
  - packtab c5: about `1.90 us`
  - packtab c9: about `2.37 us`

That matches the size story well:

- `c1` is the fast-but-larger point
- `c5` and `c9` are the size-oriented points
- `script c9` is the smallest measured size point, but `script c5` is the better middle ground on speed

## Binary Size Measurement

The size harness uses separate release binaries per property and configuration, plus an `empty`
baseline binary with the same general harness shape but no property lookup tables.

Build them:

```sh
cargo build -p packtab-icu4x --release --features compiled_data --bins
```

The main binaries are:

- `size-empty`
- `size-gc-icu`
- `size-gc-packtab`
- `size-gc-packtab-unsafe`
- `size-gc-packtab-c5`
- `size-gc-packtab-c5-unsafe`
- `size-gc-packtab-c9`
- `size-gc-packtab-c9-unsafe`
- `size-script-icu`
- `size-script-packtab`
- `size-script-packtab-unsafe`
- `size-script-packtab-c5`
- `size-script-packtab-c5-unsafe`
- `size-script-packtab-c9`
- `size-script-packtab-c9-unsafe`

On macOS, inspect section sizes with:

```sh
size -m target/release/size-gc-packtab-c9
```

The most useful comparison here is:

- `__TEXT,__text`
- `__TEXT,__const`
- `__DATA_CONST,__const`

and their sum as a rough `code + rodata` metric.

For property accounting, use deltas against `size-empty`, not the raw totals. That removes most
of the common harness/runtime noise and gets much closer to the actual property representation cost.

### Current section sizes

Measured on this machine from `size -m`:

| Binary | `__text` | `__TEXT.__const` | `__DATA_CONST.__const` | `code + rodata` |
|---|---:|---:|---:|---:|
| `size-gc-icu` | `215124` | `33600` | `9528` | `258252` |
| `size-gc-packtab` | `214944` | `36304` | `9576` | `260824` |
| `size-gc-packtab-unsafe` | `215536` | `36208` | `9528` | `261272` |
| `size-gc-packtab-c5` | `215008` | `28160` | `9600` | `252768` |
| `size-gc-packtab-c5-unsafe` | `215416` | `28064` | `9528` | `253008` |
| `size-gc-packtab-c9` | `215008` | `28160` | `9600` | `252768` |
| `size-gc-packtab-c9-unsafe` | `215416` | `28064` | `9528` | `253008` |
| `size-script-icu` | `215152` | `41792` | `9600` | `266544` |
| `size-script-packtab` | `214904` | `55760` | `9552` | `280216` |
| `size-script-packtab-unsafe` | `215396` | `55664` | `9528` | `280588` |
| `size-script-packtab-c5` | `214976` | `31312` | `9576` | `255864` |
| `size-script-packtab-c5-unsafe` | `215332` | `31216` | `9528` | `256076` |
| `size-script-packtab-c9` | `215016` | `29552` | `9600` | `254168` |
| `size-script-packtab-c9-unsafe` | `215412` | `29456` | `9528` | `254396` |

### Current deltas over `size-empty`

`size-empty` measured:

| Binary | `__text` | `__TEXT.__const` | `__DATA_CONST.__const` | `code + rodata` |
|---|---:|---:|---:|---:|
| `size-empty` | `215016` | `15952` | `9528` | `240496` |

Using `code + rodata - size-empty(code + rodata)`:

| Binary | Delta vs `size-empty` |
|---|---:|
| `size-gc-icu` | `17756` |
| `size-gc-packtab` | `20328` |
| `size-gc-packtab-c5` | `12272` |
| `size-gc-packtab-c9` | `12272` |
| `size-script-icu` | `26032` |
| `size-script-packtab` | `39720` |
| `size-script-packtab-c5` | `15368` |
| `size-script-packtab-c9` | `13672` |

Takeaways from the delta view:

- `gc`:
  - compression `1` is larger than ICU4X
  - compression `5` and `9` are both smaller than ICU4X
  - `5` and `9` produced the same measured footprint on this property in this run
- `script`:
  - compression `1` is substantially larger than ICU4X
  - compression `5` and `9` are both smaller than ICU4X
  - compression `9` is the smallest of the measured `script` variants
- unsafe code has negligible size impact relative to the corresponding safe variant, so the safe deltas are a good proxy for both

## Notes

- Whole binary size is intentionally not the primary metric here.
- Raw section totals are still somewhat noisy; deltas against `size-empty` are the better approximation for property cost in this harness.
- The current build script generates `u32` wrappers for the measurement harnesses to compare lookup cost and static footprint without enum reconstruction noise.
