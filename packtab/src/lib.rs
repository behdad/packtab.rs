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

fn score_solution(solution: &AnyOuterSolution, compression: f64) -> f64 {
    solution.n_lookups() as f64 + compression * (solution.full_cost() as f64).log2()
}

fn better_solution(a: &AnyOuterSolution, b: &AnyOuterSolution, compression: f64) -> bool {
    if compression <= 0.0 {
        let a_flat = !a.is_palette() && a.bits() == Some(0);
        let b_flat = !b.is_palette() && b.bits() == Some(0);
        return match (a_flat, b_flat) {
            (true, true) => (a.n_extra_ops(), a.cost()) < (b.n_extra_ops(), b.cost()),
            (true, false) => true,
            (false, true) => false,
            (false, false) => (a.n_lookups(), a.n_extra_ops(), a.cost())
                < (b.n_lookups(), b.n_extra_ops(), b.cost()),
        };
    }

    if compression >= 10.0 {
        return (a.cost(), a.n_lookups(), a.n_extra_ops())
            < (b.cost(), b.n_lookups(), b.n_extra_ops());
    }

    score_solution(a, compression) < score_solution(b, compression)
}

fn pick_best_info(candidates: Vec<OuterLayerInfo>, compression: f64) -> (OuterLayerInfo, usize) {
    let mut best_choice: Option<(usize, usize)> = None;

    for (info_idx, info) in candidates.iter().enumerate() {
        let solution_idx = pick_solution(&info.solutions, compression);
        let solution = &info.solutions[solution_idx];
        let replace = match best_choice {
            None => true,
            Some((best_info_idx, best_solution_idx)) => {
                let best_solution = &candidates[best_info_idx].solutions[best_solution_idx];
                better_solution(solution, best_solution, compression)
            }
        };
        if replace {
            best_choice = Some((info_idx, solution_idx));
        }
    }

    let (info_idx, solution_idx) = best_choice.expect("at least one candidate layer");
    let info = candidates.into_iter().nth(info_idx).unwrap();
    (info, solution_idx)
}

/// Pack a table of integers into compact multi-level lookup tables.
///
/// Returns the OuterLayerInfo (containing all solutions) and the
/// picked best solution index.
pub fn pack_table(data: &[i64], default: i64, compression: f64) -> (OuterLayerInfo, usize) {
    assert!(!data.is_empty(), "data must not be empty");
    let mut candidates = vec![OuterLayerInfo::new(data, default)];
    if let Some(exact) = OuterLayerInfo::exact_inline_candidate(data, default) {
        candidates.push(exact);
    }
    pick_best_info(candidates, compression)
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
