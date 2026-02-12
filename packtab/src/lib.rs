pub mod codegen;
pub mod ir;
pub mod layer;
pub mod mapping;
pub mod solution;
pub mod util;

#[cfg(test)]
mod tests;

use codegen::Language;
use layer::OuterLayerInfo;
use solution::OuterSolution;

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
/// The `compression` parameter controls the tradeoff:
/// - Higher values prefer smaller tables (more compression).
/// - Lower values prefer fewer lookups (faster access).
pub fn pick_solution(solutions: &[OuterSolution], compression: f64) -> usize {
    solutions
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let score_a = a.n_lookups as f64 + compression * (a.full_cost() as f64).log2();
            let score_b = b.n_lookups as f64 + compression * (b.full_cost() as f64).log2();
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
