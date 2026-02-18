use crate::ir::{ArrayDecl, CodeIR, FuncDecl, IntType, AccessorDecl};
use crate::layer::{AnyOuterSolution, InnerLayerChain, OuterLayerInfo, PaletteOuterSolution};
use crate::solution::OuterSolution;
use crate::util::{combine, expand};

/// Target language for code generation.
#[derive(Debug, Clone, Copy)]
pub enum Language {
    C,
    Rust { unsafe_access: bool },
}

impl Language {
    fn is_rust(self) -> bool {
        matches!(self, Language::Rust { .. })
    }

}

/// Accumulated code state during generation (mirrors Python's Code class).
struct CodeBuilder {
    namespace: String,
    arrays: Vec<(String, IntType, Vec<i64>)>,
    array_offsets: std::collections::HashMap<String, usize>,
    functions: Vec<(String, IntType, String, String, bool, bool)>,
    function_set: std::collections::HashSet<String>,
    accessors: std::collections::HashSet<u8>,
}

impl CodeBuilder {
    fn new(namespace: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            arrays: Vec::new(),
            array_offsets: std::collections::HashMap::new(),
            functions: Vec::new(),
            function_set: std::collections::HashSet::new(),
            accessors: std::collections::HashSet::new(),
        }
    }

    fn name_for(&self, name: &str) -> String {
        format!("{}_{}", self.namespace, name)
    }

    fn add_array(&mut self, typ: IntType, name: &str, values: &[i64]) -> (String, usize) {
        let full_name = self.name_for(name);
        if let Some(idx) = self.array_offsets.get(&full_name).copied() {
            let mut start = self.arrays[idx].2.len();

            // Overlap optimization: if last element of existing array equals
            // first element of new values, find maximum overlap of that value
            let mut overlap = 0;
            if !self.arrays[idx].2.is_empty()
                && !values.is_empty()
                && self.arrays[idx].2.last() == values.first()
            {
                let overlap_value = *self.arrays[idx].2.last().unwrap();

                // Count trailing run of overlap_value in existing array
                let trailing_count = self.arrays[idx]
                    .2
                    .iter()
                    .rev()
                    .take_while(|&&v| v == overlap_value)
                    .count();

                // Count leading run of overlap_value in new values
                let leading_count = values.iter().take_while(|&&v| v == overlap_value).count();

                // Overlap by the minimum of the two runs
                overlap = trailing_count.min(leading_count);
            }

            if overlap > 0 {
                start -= overlap;
                self.arrays[idx].2.extend_from_slice(&values[overlap..]);
            } else {
                self.arrays[idx].2.extend_from_slice(values);
            }

            return (full_name, start);
        }
        let idx = self.arrays.len();
        self.arrays.push((full_name.clone(), typ, values.to_vec()));
        self.array_offsets.insert(full_name.clone(), idx);
        (full_name, 0)
    }

    fn add_function(
        &mut self,
        ret_type: IntType,
        name: &str,
        arg_name: &str,
        body: &str,
        private: bool,
        inline_always: bool,
    ) -> String {
        let full_name = self.name_for(name);
        if !self.function_set.contains(&full_name) {
            self.function_set.insert(full_name.clone());
            self.functions.push((
                full_name.clone(),
                ret_type,
                arg_name.to_string(),
                body.to_string(),
                private,
                inline_always,
            ));
        }
        full_name
    }

    fn into_code_ir(self, private_arrays: bool) -> CodeIR {
        let mut ir = CodeIR::new();
        for (name, typ, values) in self.arrays {
            ir.arrays.push(ArrayDecl {
                name,
                typ,
                values,
                private: private_arrays,
            });
        }
        for bits in &self.accessors {
            ir.accessors.push(AccessorDecl {
                name: self.namespace.clone() + "_b" + &bits.to_string(),
                unit_bits: *bits,
                inline_always: true,
            });
        }
        for (name, ret_type, arg_name, body, private, inline_always) in self.functions {
            ir.functions.push(FuncDecl {
                name,
                ret_type,
                arg_name,
                body,
                private,
                inline_always,
            });
        }
        ir
    }
}

/// Generate code for an InnerSolution.
/// Returns (retType, expr).
fn gen_inner_code(
    sol_idx: usize,
    chain: &InnerLayerChain,
    code: &mut CodeBuilder,
    var: &str,
    lang: Language,
    data_multiplier: i64,
) -> (IntType, String) {
    let sol = &chain.solutions[sol_idx];
    let layer = &chain.layers[sol.layer_idx];

    let typ = IntType::for_range(layer.min_v, layer.max_v);
    let unit_bits = layer.unit_bits;

    if unit_bits == 0 {
        // All values identical; return constant.
        return (typ, format!("{}", layer.data[0]));
    }

    let shift = sol.bits;
    let mask = (1u64 << shift) - 1;

    // Check if we can bake shift into child's data.
    let mut bake_shift = false;
    if let Some(next_idx) = sol.next {
        if shift > 0 {
            let child_sol = &chain.solutions[next_idx];
            let child_layer = &chain.layers[child_sol.layer_idx];
            let child_max_v = child_layer.max_v;
            let child_unit_bits = child_layer.unit_bits;
            if child_max_v * (1i64 << shift) < (1i64 << child_unit_bits) {
                bake_shift = true;
            }
        }
    }

    // Recurse into child level.
    let mut expr = var.to_string();
    if let Some(next_idx) = sol.next {
        let child_multiplier = if bake_shift { 1i64 << shift } else { 1 };
        let child_var = format!("(({})>>{})", var, shift);
        let (_, child_expr) = gen_inner_code(
            next_idx,
            chain,
            code,
            &child_var,
            lang,
            child_multiplier,
        );
        expr = child_expr;
    }

    // Reconstruct flat data for this level by expanding mappings.
    let mut layers_with_mappings = Vec::new();
    let mut walk_idx = sol.layer_idx;
    let mut remaining_bits = sol.bits;
    while remaining_bits > 0 {
        layers_with_mappings.push(walk_idx);
        walk_idx += 1;
        remaining_bits -= 1;
    }

    let leaf_layer = &chain.layers[walk_idx];
    let mut data: Vec<i64> = if layers_with_mappings.is_empty() {
        leaf_layer.data.clone()
    } else {
        assert_eq!(leaf_layer.min_v, 0);
        let mappings: Vec<&crate::mapping::AutoMapping> = layers_with_mappings
            .iter()
            .map(|&idx| chain.layers[idx].mapping.as_ref().unwrap())
            .collect();
        let mut out = Vec::new();
        for d in 0..=leaf_layer.max_v as usize {
            expand(d, &mappings, mappings.len(), &mut out);
        }
        out
    };

    // Apply data_multiplier from parent.
    if data_multiplier > 1 {
        for v in &mut data {
            *v *= data_multiplier;
        }
    }

    // Pack sub-byte values.
    data = combine(data, layer.unit_bits);

    // Check if we can inline as a constant.
    let can_inline = data.len() * 8 <= 64 && data.iter().all(|&v| v >= 0);

    let arr_name: String;
    let start: usize;

    if !can_inline {
        let type_abbr = IntType::for_range(
            *data.iter().min().unwrap(),
            *data.iter().max().unwrap(),
        );
        let (name, s) = code.add_array(type_abbr, type_abbr.abbr(), &data);
        arr_name = name;
        start = s;
    } else {
        arr_name = String::new();
        start = 0;
    }

    // Build the index expression.
    let index0 = if expr == "0" {
        String::new()
    } else if shift == 0 || bake_shift {
        as_usize(&expr, lang)
    } else {
        format!("(({})<<{})", as_usize(&expr, lang), shift)
    };

    let index1 = if mask != 0 {
        format!("(({})&{})", var, mask)
    } else {
        String::new()
    };

    let index = if !index0.is_empty() && !index1.is_empty() {
        format!("{}+{}", as_usize(&index0, lang), as_usize(&index1, lang))
    } else if !index0.is_empty() {
        as_usize(&index0, lang)
    } else {
        as_usize(&index1, lang)
    };

    // Emit lookup expression.
    if can_inline {
        let mut packed: u64 = 0;
        for (i, &b) in data.iter().enumerate() {
            packed |= (b as u64) << (i * 8);
        }
        let total_bits = data.len() * 8;
        let const_typ = if total_bits >= 64 {
            IntType::U64
        } else {
            IntType::for_range(0, (1i64 << total_bits) - 1)
        };
        let lit = uint_literal(packed, const_typ, lang);
        let element_mask = (1u64 << unit_bits) - 1;
        let shift2 = if unit_bits > 1 {
            (unit_bits as f64).log2().round() as u8
        } else {
            0
        };
        let idx = as_usize(&index, lang);
        expr = if shift2 > 0 {
            format!("(({}>>(({})<< {}))&{})", lit, idx, shift2, element_mask)
        } else {
            format!("(({}>>( {}))&{})", lit, idx, element_mask)
        };
    } else if unit_bits >= 8 {
        // Direct array index.
        let start_str = if start > 0 {
            Some(usize_literal(start, lang))
        } else {
            None
        };
        let full_index = if let Some(s) = start_str {
            format!("{}+{}", s, as_usize(&index, lang))
        } else {
            index
        };
        expr = array_index(&arr_name, &full_index, lang);
    } else {
        // Sub-byte accessor.
        let shift1 = ((8 / unit_bits) as f64).log2().round() as u8;
        let mask1 = (8 / unit_bits) - 1;
        let shift2 = (unit_bits as f64).log2().round() as u8;
        let mask2 = (1u64 << unit_bits) - 1;

        let func_body = format!(
            "({}>>((i&{})<< {}))&{}",
            array_index("a", &format!("i>>{}", shift1), lang),
            mask1,
            shift2,
            mask2,
        );

        let func_name = code.add_function(IntType::U8, &format!("b{}", unit_bits), "i", &func_body, true, true);
        code.accessors.insert(unit_bits);

        let start_str = if start > 0 {
            Some(usize_literal(start, lang))
        } else {
            None
        };

        let sliced = if let Some(s) = &start_str {
            slice_array(&arr_name, s, lang)
        } else {
            borrow_array(&arr_name, lang)
        };

        expr = format!("{}({},{})", func_name, sliced, index);
    }

    (typ, expr)
}

/// Generate code for an OuterSolution.
fn gen_outer_code(
    outer: &OuterSolution,
    outer_info: &OuterLayerInfo,
    code_builder: &mut CodeBuilder,
    name: Option<&str>,
    var: &str,
    lang: Language,
    private: bool,
) -> (IntType, String) {
    let input_var = var;
    let var = if name.is_some() { "u" } else { var };

    let typ = IntType::for_range(outer_info.min_v, outer_info.max_v);
    let ret_type = typ;

    // Generate inner code.
    let (_, inner_expr) = gen_inner_code(
        outer.inner_idx,
        &outer_info.inner,
        code_builder,
        var,
        lang,
        1,
    );
    let mut expr = cast(&inner_expr, ret_type, lang);

    // Apply mult.
    if outer_info.mult != 1 {
        expr = format!("{}*{}", outer_info.mult, expr);
    }

    // Apply identity + bias.
    if outer_info.identity {
        expr = wrapping_add(&cast(var, ret_type, lang), &expr, lang);
        if outer_info.bias > 0 {
            expr = wrapping_add(
                &uint_literal(outer_info.bias as u64, ret_type, lang),
                &expr,
                lang,
            );
        } else if outer_info.bias < 0 {
            expr = wrapping_sub(
                &expr,
                &uint_literal((-outer_info.bias) as u64, ret_type, lang),
                lang,
            );
        }
    } else {
        if outer_info.bias > 0 {
            expr = format!("{}+{}", outer_info.bias, expr);
        } else if outer_info.bias < 0 {
            expr = format!("{}-{}", expr, -outer_info.bias);
        }
    }

    // Bounds check with default.
    let default_str = format!("{}", outer_info.default);
    expr = ternary(
        &format!("{}<{}", var, outer_info.data.len()),
        &expr,
        &default_str,
        lang,
    );

    // Wrap in a named function if requested.
    if let Some(name) = name {
        let func_name = code_builder.add_function(ret_type, name, "u", &expr, private, false);
        expr = format!("{}({})", func_name, input_var);
    }

    (ret_type, expr)
}

/// Generate code for a PaletteOuterSolution.
///
/// Emits a palette array of unique values and a lookup function that:
///   1. uses the inner chain to get a palette index, then
///   2. returns `palette[index]` (with the same arithmetic as `gen_outer_code`).
fn gen_palette_outer_code(
    palette_sol: &PaletteOuterSolution,
    outer_info: &OuterLayerInfo,
    code_builder: &mut CodeBuilder,
    name: Option<&str>,
    var: &str,
    lang: Language,
    private: bool,
) -> (IntType, String) {
    let input_var = var;
    let var = if name.is_some() { "u" } else { var };

    let typ = IntType::for_range(outer_info.min_v, outer_info.max_v);
    let ret_type = typ;

    // Emit the palette array.
    let palette = &outer_info.palette;
    let pal_min = *palette.iter().min().unwrap();
    let pal_max = *palette.iter().max().unwrap();
    let palette_typ = IntType::for_range(pal_min, pal_max);
    let (palette_name, _) = code_builder.add_array(palette_typ, "palette", palette);

    // Generate the index lookup expression from the palette inner chain.
    let palette_inner = outer_info.palette_inner.as_ref().unwrap();
    let (_, index_expr) = gen_inner_code(
        palette_sol.inner_idx,
        palette_inner,
        code_builder,
        var,
        lang,
        1,
    );

    // Cast index to usize (required in Rust; no-op in C) and look up palette.
    let index_usize = as_usize(&index_expr, lang);
    let mut expr = array_index(&palette_name, &index_usize, lang);
    expr = cast(&expr, ret_type, lang);

    // Apply mult.
    if outer_info.mult != 1 {
        expr = format!("{}*{}", outer_info.mult, expr);
    }

    // Apply identity + bias.
    if outer_info.identity {
        expr = wrapping_add(&cast(var, ret_type, lang), &expr, lang);
        if outer_info.bias > 0 {
            expr = wrapping_add(
                &uint_literal(outer_info.bias as u64, ret_type, lang),
                &expr,
                lang,
            );
        } else if outer_info.bias < 0 {
            expr = wrapping_sub(
                &expr,
                &uint_literal((-outer_info.bias) as u64, ret_type, lang),
                lang,
            );
        }
    } else {
        if outer_info.bias > 0 {
            expr = format!("{}+{}", outer_info.bias, expr);
        } else if outer_info.bias < 0 {
            expr = format!("{}-{}", expr, -outer_info.bias);
        }
    }

    // Bounds check with default.
    let default_str = format!("{}", outer_info.default);
    expr = ternary(
        &format!("{}<{}", var, outer_info.data.len()),
        &expr,
        &default_str,
        lang,
    );

    // Wrap in a named function if requested.
    if let Some(name) = name {
        let func_name = code_builder.add_function(ret_type, name, "u", &expr, private, false);
        expr = format!("{}({})", func_name, input_var);
    }

    (ret_type, expr)
}

/// Build the full CodeIR for a solution.
pub fn generate(
    solution: &AnyOuterSolution,
    outer_info: &OuterLayerInfo,
    name: &str,
    lang: Language,
) -> CodeIR {
    let mut builder = CodeBuilder::new(name);
    match solution {
        AnyOuterSolution::Direct(outer) => {
            gen_outer_code(outer, outer_info, &mut builder, Some("get"), "u", lang, false);
        }
        AnyOuterSolution::Palette(palette_sol) => {
            gen_palette_outer_code(
                palette_sol,
                outer_info,
                &mut builder,
                Some("get"),
                "u",
                lang,
                false,
            );
        }
    }
    builder.into_code_ir(true)
}

/// Render a CodeIR to a source code string.
pub fn render(ir: &CodeIR, lang: Language) -> String {
    let mut out = String::new();

    // Preamble
    if !lang.is_rust() {
        out.push_str("#include <stdint.h>\n\n");
    }

    // Arrays
    for arr in &ir.arrays {
        render_array(&mut out, arr, lang);
    }

    if !ir.arrays.is_empty() && (!ir.accessors.is_empty() || !ir.functions.is_empty()) {
        out.push('\n');
    }

    // Accessor functions (sub-byte)
    for acc in &ir.accessors {
        render_accessor(&mut out, acc, lang);
    }

    // Functions
    for func in &ir.functions {
        // Skip accessor functions that we've already rendered
        if ir.accessors.iter().any(|a| func.name.ends_with(&format!("_b{}", a.unit_bits))) {
            continue;
        }
        render_function(&mut out, func, lang);
    }

    out
}

fn render_array(out: &mut String, arr: &ArrayDecl, lang: Language) {
    match lang {
        Language::C => {
            let linkage = if arr.private { "static const" } else { "extern const" };
            let typ = c_type_name(arr.typ);
            out.push_str(&format!("{} {} {}[{}]", linkage, typ, arr.name, arr.values.len()));
            out.push_str(" =\n{\n");
        }
        Language::Rust { .. } => {
            out.push_str("#[allow(dead_code, non_upper_case_globals)]\n");
            let linkage = if arr.private { "static" } else { "pub(crate) static" };
            let typ = rust_type_name(arr.typ);
            out.push_str(&format!("{} {}: [{}; {}]", linkage, arr.name, typ, arr.values.len()));
            out.push_str(" =\n[\n");
        }
    }

    // Format values
    let w = arr
        .values
        .iter()
        .map(|v| format!("{}", v).len())
        .max()
        .unwrap_or(1);
    let n = 1usize << ((78.0 / (w as f64 + 1.0)).log2().floor() as u32);
    let w = if (w + 2) * n <= 78 { w + 1 } else { w };

    for chunk in arr.values.chunks(n) {
        out.push_str("  ");
        for v in chunk {
            out.push_str(&format!("{:>width$},", v, width = w));
        }
        out.push('\n');
    }

    match lang {
        Language::C => out.push_str("};\n"),
        Language::Rust { .. } => out.push_str("];\n"),
    }
}

fn render_accessor(out: &mut String, acc: &AccessorDecl, lang: Language) {
    let shift1 = ((8 / acc.unit_bits) as f64).log2().round() as u8;
    let mask1 = (8 / acc.unit_bits) - 1;
    let shift2 = (acc.unit_bits as f64).log2().round() as u8;
    let mask2 = (1u64 << acc.unit_bits) - 1;

    match lang {
        Language::C => {
            out.push_str(&format!(
                "static inline uint8_t {} (const uint8_t* a, unsigned i)\n",
                acc.name
            ));
            out.push_str("{\n");
            out.push_str(&format!(
                "  return (a[i>>{}]>>((i&{})<<{}))&{};\n",
                shift1, mask1, shift2, mask2
            ));
            out.push_str("}\n");
        }
        Language::Rust { unsafe_access } => {
            out.push_str("#[allow(dead_code, unused_parens)]\n");
            if acc.inline_always {
                out.push_str("#[inline(always)]\n");
            } else {
                out.push_str("#[inline]\n");
            }
            out.push_str(&format!(
                "fn {} (a: &[u8], i: usize) -> u8\n",
                acc.name
            ));
            out.push_str("{\n");
            let idx_expr = if unsafe_access {
                format!("unsafe {{ *(a.get_unchecked((i>>{}) as usize)) }}", shift1)
            } else {
                format!("a[(i>>{}) as usize]", shift1)
            };
            out.push_str(&format!(
                "  ({}>>((i&{})<<{}))&{}\n",
                idx_expr, mask1, shift2, mask2
            ));
            out.push_str("}\n");
        }
    }
}

fn render_function(out: &mut String, func: &FuncDecl, lang: Language) {
    match lang {
        Language::C => {
            let linkage = if func.private {
                "static inline"
            } else {
                "extern inline"
            };
            let typ = c_type_name(func.ret_type);
            out.push_str(&format!(
                "{} {} {} (unsigned {})\n",
                linkage, typ, func.name, func.arg_name
            ));
            out.push_str("{\n");
            out.push_str(&format!("  return {};\n", func.body));
            out.push_str("}\n");
        }
        Language::Rust { .. } => {
            out.push_str("#[allow(dead_code, unused_parens)]\n");
            if func.inline_always {
                out.push_str("#[inline(always)]\n");
            } else {
                out.push_str("#[inline]\n");
            }
            let linkage = if func.private { "" } else { "pub(crate) " };
            let typ = rust_type_name(func.ret_type);
            out.push_str(&format!(
                "{}fn {} ({}: usize) -> {}\n",
                linkage, func.name, func.arg_name, typ
            ));
            out.push_str("{\n");
            out.push_str(&format!("  {}\n", func.body));
            out.push_str("}\n");
        }
    }
}

fn c_type_name(typ: IntType) -> &'static str {
    match typ {
        IntType::U8 => "uint8_t",
        IntType::U16 => "uint16_t",
        IntType::U32 => "uint32_t",
        IntType::U64 => "uint64_t",
        IntType::I8 => "int8_t",
        IntType::I16 => "int16_t",
        IntType::I32 => "int32_t",
        IntType::I64 => "int64_t",
    }
}

fn rust_type_name(typ: IntType) -> &'static str {
    typ.abbr()
}

// Helper functions matching Python's Language methods

fn as_usize(expr: &str, lang: Language) -> String {
    match lang {
        Language::C => expr.to_string(),
        Language::Rust { .. } => {
            if expr.is_empty() {
                return String::new();
            }
            // Check if it's a plain integer literal
            if expr.parse::<i64>().is_ok() {
                format!("{}usize", expr)
            } else if expr.starts_with('(') && expr.ends_with(')') {
                format!("{} as usize", expr)
            } else {
                format!("({}) as usize", expr)
            }
        }
    }
}

fn usize_literal(value: usize, lang: Language) -> String {
    match lang {
        Language::C => format!("{}u", value),
        Language::Rust { .. } => format!("{}usize", value),
    }
}

fn uint_literal(value: u64, typ: IntType, lang: Language) -> String {
    match lang {
        Language::C => {
            if typ.bits() == 64 {
                format!("{}ULL", value)
            } else {
                format!("{}u", value)
            }
        }
        Language::Rust { .. } => format!("{}{}", value, typ.abbr()),
    }
}

fn cast(expr: &str, typ: IntType, lang: Language) -> String {
    match lang {
        Language::C => format!("({})({})", c_type_name(typ), expr),
        Language::Rust { .. } => format!("({}) as {}", expr, rust_type_name(typ)),
    }
}

fn array_index(name: &str, index: &str, lang: Language) -> String {
    match lang {
        Language::Rust { unsafe_access: true } => {
            format!("unsafe {{ *({}.get_unchecked({})) }}", name, index)
        }
        _ => format!("{}[{}]", name, index),
    }
}

fn borrow_array(name: &str, lang: Language) -> String {
    match lang {
        Language::C => name.to_string(),
        Language::Rust { .. } => format!("&{}", name),
    }
}

fn slice_array(name: &str, start: &str, lang: Language) -> String {
    match lang {
        Language::C => format!("{}+{}", name, start),
        Language::Rust { .. } => format!("&{}[{}..]", name, start),
    }
}

fn ternary(cond: &str, true_expr: &str, false_expr: &str, lang: Language) -> String {
    match lang {
        Language::C => format!("{} ? {} : {}", cond, true_expr, false_expr),
        Language::Rust { .. } => format!("if {} {{ {} }} else {{ {} }}", cond, true_expr, false_expr),
    }
}

fn wrapping_add(a: &str, b: &str, lang: Language) -> String {
    match lang {
        Language::C => format!("{}+{}", a, b),
        Language::Rust { .. } => format!("({}).wrapping_add({})", a, b),
    }
}

fn wrapping_sub(a: &str, b: &str, lang: Language) -> String {
    match lang {
        Language::C => format!("{}-{}", a, b),
        Language::Rust { .. } => format!("({}).wrapping_sub({})", a, b),
    }
}
