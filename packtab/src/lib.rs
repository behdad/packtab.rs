pub mod codegen;
pub mod ir;
pub mod layer;
pub mod mapping;
pub mod solution;
pub mod util;

#[cfg(test)]
mod tests;

use codegen::Language;
use layer::{AnyOuterSolution, OuterLayerInfo};

/// Pack a table of integers into compact multi-level lookup tables.
///
/// Returns the OuterLayerInfo (containing all solutions) and the
/// picked best solution index.
pub fn pack_table(data: &[i64], default: i64, compression: f64) -> (OuterLayerInfo, usize) {
    assert!(!data.is_empty(), "data must not be empty");
    let info = OuterLayerInfo::new(data, default);
    let best_idx = pick_solution(&info.solutions, compression);
    (info, best_idx)
}

/// Pack and return all solutions (for advanced usage).
pub fn pack_table_all(data: &[i64], default: i64) -> OuterLayerInfo {
    assert!(!data.is_empty(), "data must not be empty");
    OuterLayerInfo::new(data, default)
}

/// Select the best solution from the Pareto frontier.
///
/// The `compression` parameter controls the tradeoff.
/// Values 1..9 use the historical heuristic.
/// Values <= 0 pick a flat / unsplit solution when available.
/// Values >= 10 minimize raw table bytes.
pub fn pick_solution(solutions: &[AnyOuterSolution], compression: f64) -> usize {
    if compression <= 0.0 {
        if let Some((i, _)) = solutions
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_palette() && s.bits() == Some(0))
            .min_by_key(|(_, s)| (s.n_extra_ops(), s.cost()))
        {
            return i;
        }
        return solutions
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| (s.n_lookups(), s.n_extra_ops(), s.cost()))
            .map(|(i, _)| i)
            .unwrap();
    }

    if compression >= 10.0 {
        return solutions
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| (s.cost(), s.n_lookups(), s.n_extra_ops()))
            .map(|(i, _)| i)
            .unwrap();
    }

    solutions
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let score_a = a.n_lookups() as f64 + compression * (a.full_cost() as f64).log2();
            let score_b = b.n_lookups() as f64 + compression * (b.full_cost() as f64).log2();
            score_a.partial_cmp(&score_b).unwrap()
        })
        .map(|(i, _)| i)
        .unwrap()
}

/// Generate source code for a packed table solution.
pub fn generate(
    info: &OuterLayerInfo,
    solution_idx: usize,
    name: &str,
    lang: Language,
) -> String {
    let solution = &info.solutions[solution_idx];
    let ir = codegen::generate(solution, info, name, lang);
    codegen::render(&ir, lang)
}
