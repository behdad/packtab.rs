use crate::mapping::AutoMapping;
use crate::solution::{full_cost, InnerSolution, OuterSolution, SUB_BYTE_ACCESS_OPS};
use crate::util::{binary_bits_for, gcd};

/// One level in the inner binary-split chain.
#[derive(Debug)]
pub struct InnerLayer {
    pub data: Vec<i64>,
    pub max_v: i64,
    pub min_v: i64,
    pub unit_bits: u8,
    pub extra_ops: usize,
    pub bytes: usize,
    /// The mapping used to split this layer (pairs of values → IDs).
    /// Only present if this layer was split (i.e., has a child).
    pub mapping: Option<AutoMapping>,
}

/// The full chain of InnerLayers plus all candidate solutions.
#[derive(Debug)]
pub struct InnerLayerChain {
    pub layers: Vec<InnerLayer>,
    pub solutions: Vec<InnerSolution>,
}

impl InnerLayerChain {
    /// Build the complete chain from initial data.
    pub fn new(data: Vec<i64>) -> Self {
        let mut chain = InnerLayerChain {
            layers: Vec::new(),
            solutions: Vec::new(),
        };

        // Build the first layer from the original data.
        chain.build_layers(data);
        chain.build_solutions();
        chain.prune_solutions();

        chain
    }

    fn build_layers(&mut self, mut data: Vec<i64>) {
        loop {
            let max_v = *data.iter().max().unwrap();
            let min_v = *data.iter().min().unwrap();
            let unit_bits = binary_bits_for(min_v, max_v);
            let extra_ops = if unit_bits < 8 { SUB_BYTE_ACCESS_OPS } else { 0 };
            let bytes = ((unit_bits as usize) * data.len() + 7) / 8;

            let layer = InnerLayer {
                data,
                max_v,
                min_v,
                unit_bits,
                extra_ops,
                bytes,
                mapping: None,
            };
            self.layers.push(layer);

            if max_v == 0 {
                break;
            }

            // Split: pad to even length, pair adjacent elements
            // Smart padding: choose value that creates most common pair.
            // The padded position is never accessed, so this is safe.
            let cur = self.layers.last_mut().unwrap();
            if cur.data.len() & 1 != 0 {
                let last_val = cur.data[cur.data.len() - 1];
                let padding = Self::choose_optimal_padding(&cur.data, last_val);
                cur.data.push(padding);
            }

            // Collect pairs with frequencies and first occurrence positions
            use std::collections::HashMap;
            let pairs: Vec<(usize, usize)> = cur.data
                .chunks(2)
                .map(|pair| (pair[0] as usize, pair[1] as usize))
                .collect();

            let mut pair_freq: HashMap<(usize, usize), usize> = HashMap::new();
            let mut first_occurrence: HashMap<(usize, usize), usize> = HashMap::new();

            for (i, &pair) in pairs.iter().enumerate() {
                *pair_freq.entry(pair).or_insert(0) += 1;
                first_occurrence.entry(pair).or_insert(i);
            }

            // Sort unique pairs by frequency (descending), then position (ascending)
            let mut unique_pairs: Vec<(usize, usize)> = pair_freq.keys().copied().collect();
            unique_pairs.sort_by(|a, b| {
                let freq_cmp = pair_freq[b].cmp(&pair_freq[a]);
                if freq_cmp == std::cmp::Ordering::Equal {
                    first_occurrence[a].cmp(&first_occurrence[b])
                } else {
                    freq_cmp
                }
            });

            // Create mapping with IDs assigned in sorted order
            let mut mapping = AutoMapping::new();
            for pair in unique_pairs {
                mapping.get_or_insert(pair); // Assigns next sequential ID
            }

            // Apply mapping to create child layer data
            let mut data2 = Vec::with_capacity(pairs.len());
            for pair in pairs {
                let id = mapping.get_or_insert(pair);
                data2.push(id as i64);
            }
            cur.mapping = Some(mapping);
            data = data2;
        }
    }

    /// Choose padding value that maximizes compression.
    ///
    /// Returns a value V such that the pair (last_val, V) is the most
    /// common existing pair starting with last_val. If no such pair
    /// exists, returns the most frequent value overall.
    ///
    /// When padding is needed, the padded element is never accessed
    /// (guaranteed unreachable), so we choose a padding value that
    /// creates the most common pair, maximizing compression.
    fn choose_optimal_padding(data: &[i64], last_val: i64) -> i64 {
        use std::collections::HashMap;

        // Count existing pairs to find common patterns
        let mut pair_freq: HashMap<(i64, i64), usize> = HashMap::new();
        for i in (0..data.len() - 1).step_by(2) {
            let pair = (data[i], data[i + 1]);
            *pair_freq.entry(pair).or_insert(0) += 1;
        }

        // Find which value V makes (last_val, V) most frequent
        let mut candidates: HashMap<i64, usize> = HashMap::new();
        for ((a, b), freq) in pair_freq.iter() {
            if *a == last_val {
                candidates.insert(*b, *freq);
            }
        }

        if !candidates.is_empty() {
            // Return V that makes (last_val, V) most common
            return *candidates
                .iter()
                .max_by_key(|(_, &freq)| freq)
                .map(|(val, _)| val)
                .unwrap();
        }

        // No pairs with last_val - use most frequent value overall
        // to maximize future duplicate detection opportunities
        let mut value_freq: HashMap<i64, usize> = HashMap::new();
        for &val in data.iter() {
            *value_freq.entry(val).or_insert(0) += 1;
        }
        *value_freq
            .iter()
            .max_by_key(|(_, &freq)| freq)
            .map(|(val, _)| val)
            .unwrap()
    }

    fn build_solutions(&mut self) {
        let n_layers = self.layers.len();

        // Build solutions bottom-up, matching Python's InnerLayer.__init__ recursive approach.
        //
        // For each layer k (processed deepest-first), build all solutions rooted at k:
        //   - Constant layer (max_v == 0): one zero-cost, zero-lookup solution.
        //   - Non-constant layer: flat 1-lookup solution, plus for every descendant
        //     layer j > k, wrap each of j's solutions with an expansion table of
        //     `bits = j - k` at layer k.
        //
        // After building each intermediate layer k > 0, Pareto-prune its solutions
        // before parent layers wrap them.  This is safe because adding the same
        // extra_cost to two solutions at layer k preserves dominance: if B is
        // dominated by A at layer k, then (parent wrapping B) is dominated by
        // (parent wrapping A).  Matching Python's prune_solutions() per layer.

        // Per-layer solution index lists (indices into self.solutions).
        let mut layer_sol_indices: Vec<Vec<usize>> = vec![Vec::new(); n_layers];

        for k in (0..n_layers).rev() {
            if self.layers[k].max_v == 0 {
                // Constant: zero-cost, zero-lookup solution.
                let idx = self.solutions.len();
                self.solutions.push(InnerSolution {
                    layer_idx: k,
                    next: None,
                    n_lookups: 0,
                    n_extra_ops: 0,
                    cost: 0,
                    bits: 0,
                });
                layer_sol_indices[k].push(idx);
            } else {
                let unit_bits = self.layers[k].unit_bits as usize;
                let extra_ops_k = self.layers[k].extra_ops;
                let bytes_k = self.layers[k].bytes;

                // Flat solution: one lookup directly into this layer's data.
                let flat_idx = self.solutions.len();
                self.solutions.push(InnerSolution {
                    layer_idx: k,
                    next: None,
                    n_lookups: 1,
                    n_extra_ops: extra_ops_k,
                    cost: bytes_k,
                    bits: 0,
                });
                layer_sol_indices[k].push(flat_idx);

                // For each descendant layer j, wrap its solutions at layer k.
                // bits = j - k is the shift depth for the expansion table at k.
                for j in (k + 1)..n_layers {
                    let bits = (j - k) as u8;
                    let desc_max_v = self.layers[j].max_v as usize;
                    // Cost of the expansion table stored at layer k.
                    let extra_cost =
                        ((desc_max_v + 1) * (1usize << bits) * unit_bits + 7) / 8;

                    let desc_indices = layer_sol_indices[j].clone();
                    for child_sol_idx in desc_indices {
                        let (nl, nops, c) = {
                            let child = &self.solutions[child_sol_idx];
                            (child.n_lookups, child.n_extra_ops, child.cost)
                        };
                        let new_idx = self.solutions.len();
                        self.solutions.push(InnerSolution {
                            layer_idx: k,
                            next: Some(child_sol_idx),
                            n_lookups: nl + 1,
                            n_extra_ops: nops + extra_ops_k,
                            cost: c + extra_cost,
                            bits,
                        });
                        layer_sol_indices[k].push(new_idx);
                    }
                }
            }

            // Pareto-prune intermediate layers so parent layers only wrap
            // non-dominated solutions.  Skip the root (k == 0) since
            // prune_solutions() handles it separately.
            if k > 0 {
                layer_sol_indices[k] =
                    Self::pareto_prune_indices(&self.solutions, &layer_sol_indices[k]);
            }
        }
    }

    /// Pareto-prune a list of solution indices.
    ///
    /// Returns the subset of `indices` that are non-dominated: sorted by
    /// (n_lookups, full_cost) ascending, keeping only solutions whose
    /// full_cost strictly improves on all previously kept solutions.
    fn pareto_prune_indices(solutions: &[InnerSolution], indices: &[usize]) -> Vec<usize> {
        let mut sorted = indices.to_vec();
        sorted.sort_by_key(|&i| (solutions[i].n_lookups, solutions[i].full_cost()));
        let mut kept = Vec::new();
        let mut best_cost = usize::MAX;
        for i in sorted {
            let fc = solutions[i].full_cost();
            if fc < best_cost {
                kept.push(i);
                best_cost = fc;
            }
        }
        kept
    }

    fn prune_solutions(&mut self) {
        // Only keep solutions at the root level (layer_idx == 0) that
        // have bits > 0 or are the flat/constant solutions.
        // Actually, we need to match the Python approach more carefully.
        // The Python keeps ALL solutions at the root InnerLayer level and
        // prunes based on Pareto dominance.

        // Filter to only root-level solutions (these are the ones
        // that represent complete lookup strategies from index → value).
        let root_solutions: Vec<usize> = self
            .solutions
            .iter()
            .enumerate()
            .filter(|(_, s)| s.layer_idx == 0)
            .map(|(i, _)| i)
            .collect();

        // Pareto pruning: sort by (nLookups, fullCost), keep non-dominated.
        let mut indexed: Vec<(usize, usize, usize)> = root_solutions
            .iter()
            .map(|&i| {
                let s = &self.solutions[i];
                (i, s.n_lookups, s.full_cost())
            })
            .collect();
        indexed.sort_by_key(|&(_, nl, fc)| (nl, fc));

        let mut kept_indices = Vec::new();
        let mut best_cost = usize::MAX;
        for (idx, _, fc) in indexed {
            if fc < best_cost {
                kept_indices.push(idx);
                best_cost = fc;
            }
        }

        // Mark which solutions are kept (need to keep all referenced children too).
        let mut keep = vec![false; self.solutions.len()];
        for &idx in &kept_indices {
            mark_keep(&self.solutions, idx, &mut keep);
        }

        // Remap indices
        let mut new_idx = vec![0usize; self.solutions.len()];
        let mut new_solutions = Vec::new();
        for (old_idx, sol) in self.solutions.iter().enumerate() {
            if keep[old_idx] {
                new_idx[old_idx] = new_solutions.len();
                new_solutions.push(sol.clone());
            }
        }
        // Fix up `next` pointers
        for sol in &mut new_solutions {
            if let Some(ref mut next) = sol.next {
                *next = new_idx[*next];
            }
        }

        self.solutions = new_solutions;
    }

    /// Get only the root-level solutions (for OuterLayer to wrap).
    pub fn root_solutions(&self) -> Vec<usize> {
        self.solutions
            .iter()
            .enumerate()
            .filter(|(_, s)| s.layer_idx == 0)
            .map(|(i, _)| i)
            .collect()
    }
}

fn mark_keep(solutions: &[InnerSolution], idx: usize, keep: &mut [bool]) {
    if keep[idx] {
        return;
    }
    keep[idx] = true;
    if let Some(next) = solutions[idx].next {
        mark_keep(solutions, next, keep);
    }
}

/// A palette-encoded solution: stores a compact array of unique values (the
/// palette) and a separate inner chain that looks up palette indices.
///
/// Analogous to indexed colour: instead of storing wide values directly, we
/// store narrow palette indices that index into a small palette table.
#[derive(Debug, Clone)]
pub struct PaletteOuterSolution {
    /// Index into `OuterLayerInfo::palette_inner.solutions`.
    pub inner_idx: usize,
    pub n_lookups: usize,
    pub n_extra_ops: usize,
    pub cost: usize,
}

impl PaletteOuterSolution {
    pub fn full_cost(&self) -> usize {
        full_cost(self.n_lookups, self.n_extra_ops, self.cost)
    }
}

/// Either a direct (arithmetic-encoded) or palette-encoded outer solution.
#[derive(Debug, Clone)]
pub enum AnyOuterSolution {
    Direct(OuterSolution),
    Palette(PaletteOuterSolution),
}

impl AnyOuterSolution {
    pub fn n_lookups(&self) -> usize {
        match self {
            AnyOuterSolution::Direct(s) => s.n_lookups,
            AnyOuterSolution::Palette(s) => s.n_lookups,
        }
    }
    pub fn n_extra_ops(&self) -> usize {
        match self {
            AnyOuterSolution::Direct(s) => s.n_extra_ops,
            AnyOuterSolution::Palette(s) => s.n_extra_ops,
        }
    }
    pub fn cost(&self) -> usize {
        match self {
            AnyOuterSolution::Direct(s) => s.cost,
            AnyOuterSolution::Palette(s) => s.cost,
        }
    }
    pub fn full_cost(&self) -> usize {
        match self {
            AnyOuterSolution::Direct(s) => s.full_cost(),
            AnyOuterSolution::Palette(s) => s.full_cost(),
        }
    }
    pub fn is_palette(&self) -> bool {
        matches!(self, AnyOuterSolution::Palette(_))
    }
    pub fn bits(&self) -> Option<u8> {
        match self {
            AnyOuterSolution::Direct(s) => Some(s.bits),
            AnyOuterSolution::Palette(_) => None,
        }
    }
}

/// Arithmetic preprocessing result.
#[derive(Debug)]
pub struct OuterLayerInfo {
    /// The original data (with trailing defaults stripped).
    pub data: Vec<i64>,
    /// The default value for out-of-range indices.
    pub default: i64,
    pub min_v: i64,
    pub max_v: i64,
    pub unit_bits: u8,
    pub identity: bool,
    pub bias: i64,
    pub mult: i64,
    pub extra_ops: usize,
    pub bytes: usize,
    /// The inner chain operating on reduced data.
    pub inner: InnerLayerChain,
    /// Palette values for palette encoding (empty if no palette encoding).
    pub palette: Vec<i64>,
    /// Inner chain for palette index lookups (None if no palette encoding).
    pub palette_inner: Option<InnerLayerChain>,
    /// All Pareto-frontier solutions (direct and palette interleaved).
    pub solutions: Vec<AnyOuterSolution>,
}

/// Find the (unitBits, bias, mult) triple that minimizes unitBits.
fn best_reduction(values: &[i64], min_v: i64, max_v: i64) -> (u8, i64, i64) {
    let mut bias: i64 = 0;
    let mut mult: i64 = 1;
    let mut unit_bits = binary_bits_for(min_v, max_v);

    // Strategy 1: bias only — subtract min.
    let b = min_v;
    let candidate_bits = binary_bits_for(0, max_v - b);
    if unit_bits > candidate_bits {
        unit_bits = candidate_bits;
        bias = b;
    }

    // Strategy 2: GCD only.
    let m = gcd(values.iter().copied());
    if m > 1 {
        let candidate_bits = binary_bits_for(min_v / m, max_v / m);
        if unit_bits > candidate_bits {
            unit_bits = candidate_bits;
            bias = 0;
            mult = m;
        }
    }

    // Strategy 3: bias + GCD.
    if b != 0 {
        let m = gcd(values.iter().map(|&d| d - b));
        let m = if m == 0 { 1 } else { m };
        let candidate_bits = binary_bits_for(0, (max_v - b) / m);
        if unit_bits > candidate_bits {
            unit_bits = candidate_bits;
            bias = b;
            mult = m;
        }
    }

    (unit_bits, bias, mult)
}

impl OuterLayerInfo {
    pub fn new(data: &[i64], default: i64) -> Self {
        let mut data = data.to_vec();
        // Strip trailing default values.
        while data.len() > 1 && *data.last().unwrap() == default {
            data.pop();
        }

        let min_v = *data.iter().min().unwrap();
        let max_v = *data.iter().max().unwrap();

        let mut identity = false;

        let (mut unit_bits, mut bias, mut mult) = best_reduction(&data, min_v, max_v);

        // Try identity subtraction: store data[i] - i.
        let deltas: Vec<i64> = data.iter().enumerate().map(|(i, &d)| d - i as i64).collect();
        let d_min = *deltas.iter().min().unwrap();
        let d_max = *deltas.iter().max().unwrap();
        let (id_ub, id_b, id_m) = best_reduction(&deltas, d_min, d_max);

        if id_ub < unit_bits {
            unit_bits = id_ub;
            bias = id_b;
            mult = id_m;
            identity = true;
        }

        // Compute reduced values for InnerLayer.
        let base: Vec<i64> = if identity {
            deltas
        } else {
            data.clone()
        };
        let mut reduced: Vec<i64> = base.iter().map(|&d| (d - bias) / mult).collect();

        // Bake in width multiplier if it doesn't enlarge the C integer type.
        if mult > 1 {
            let undivided: Vec<i64> = base.iter().map(|&d| d - bias).collect();
            let divided_min = *reduced.iter().min().unwrap();
            let divided_max = *reduced.iter().max().unwrap();
            let undivided_min = *undivided.iter().min().unwrap();
            let undivided_max = *undivided.iter().max().unwrap();
            let divided_type_width = binary_bits_for(divided_min, divided_max).max(8);
            let undivided_type_width = binary_bits_for(undivided_min, undivided_max).max(8);
            if undivided_type_width <= divided_type_width {
                reduced = undivided;
                unit_bits = binary_bits_for(
                    *reduced.iter().min().unwrap(),
                    *reduced.iter().max().unwrap(),
                );
                mult = 1;
            }
        }

        // Bake in bias if it doesn't enlarge the type and doesn't introduce negatives.
        // Guard on the *reduced* data being non-zero: if reduced is all-zeros, InnerLayer
        // optimizes it to cost=0, and baking in the bias would destroy that benefit.
        let current_max = *reduced.iter().max().unwrap_or(&0);
        if bias != 0 && mult == 1 && *base.iter().min().unwrap() >= 0 && current_max != 0 {
            let base_min = *base.iter().min().unwrap();
            let base_max = *base.iter().max().unwrap();
            let current_min = *reduced.iter().min().unwrap();
            let current_type_width = binary_bits_for(current_min, current_max).max(8);
            let base_type_width = binary_bits_for(base_min, base_max).max(8);
            if base_type_width <= current_type_width {
                reduced = base.clone();
                unit_bits = binary_bits_for(base_min, base_max);
                bias = 0;
            }
        }

        let extra_ops_base = if unit_bits < 8 { SUB_BYTE_ACCESS_OPS } else { 0 };
        let mut extra_ops = extra_ops_base;
        if identity {
            extra_ops += 1;
        }
        if bias != 0 {
            extra_ops += 1;
        }
        if mult != 1 {
            extra_ops += 1;
        }

        let bytes = (unit_bits as usize * data.len() + 7) / 8;

        // Build inner chain on reduced data.
        let inner = InnerLayerChain::new(reduced);

        // Wrap each root-level InnerSolution in an OuterSolution.
        let root_idxs = inner.root_solutions();
        let mut solutions: Vec<AnyOuterSolution> = Vec::new();
        for &sol_idx in &root_idxs {
            let s = &inner.solutions[sol_idx];
            solutions.push(AnyOuterSolution::Direct(OuterSolution {
                inner_idx: sol_idx,
                bits: s.bits,
                n_lookups: s.n_lookups,
                n_extra_ops: s.n_extra_ops + extra_ops,
                cost: s.cost,
            }));
        }

        // Try palette encoding on the reduced data.
        let reduced_data = inner.layers[0].data.clone();
        let (palette, palette_inner_opt, palette_sols) =
            try_palette_encoding(&reduced_data, extra_ops);
        solutions.extend(palette_sols);

        OuterLayerInfo {
            data,
            default,
            min_v,
            max_v,
            unit_bits,
            identity,
            bias,
            mult,
            extra_ops,
            bytes,
            inner,
            palette,
            palette_inner: palette_inner_opt,
            solutions,
        }
    }
}

/// Attempt palette encoding on `reduced_data`.
///
/// Builds a sorted palette of unique values and an index array, then wraps
/// every solution from the index `InnerLayerChain` as a `PaletteOuterSolution`.
/// Returns `(palette, Some(chain), solutions)` when palette encoding is
/// beneficial; `(empty, None, empty)` otherwise.
fn try_palette_encoding(
    reduced_data: &[i64],
    extra_ops: usize,
) -> (Vec<i64>, Option<InnerLayerChain>, Vec<AnyOuterSolution>) {
    // Collect unique values in sorted order.
    let palette: Vec<i64> = {
        let mut set = std::collections::BTreeSet::new();
        for &v in reduced_data {
            set.insert(v);
        }
        set.into_iter().collect()
    };

    // Degenerate cases: nothing to compress.
    if palette.len() <= 1 {
        return (vec![], None, vec![]);
    }

    let pal_min = palette[0];
    let pal_max = *palette.last().unwrap();

    // Only proceed if index bits are strictly fewer than value bits.
    let index_bits = binary_bits_for(0, (palette.len() - 1) as i64);
    let value_bits = binary_bits_for(pal_min, pal_max);
    if index_bits >= value_bits {
        return (vec![], None, vec![]);
    }

    // Map each value to its palette index.
    let value_to_index: std::collections::HashMap<i64, usize> =
        palette.iter().copied().enumerate().map(|(i, v)| (v, i)).collect();
    let indices: Vec<i64> = reduced_data
        .iter()
        .map(|&v| value_to_index[&v] as i64)
        .collect();

    // Build an inner chain for the indices (can be split further).
    let palette_chain = InnerLayerChain::new(indices);

    // Palette storage cost in bytes.
    let palette_cost = (value_bits as usize * palette.len() + 7) / 8;

    // Wrap each root-level index solution as a PaletteOuterSolution.
    let root_idxs = palette_chain.root_solutions();
    let mut solutions = Vec::new();
    for inner_idx in root_idxs {
        let s = &palette_chain.solutions[inner_idx];
        solutions.push(AnyOuterSolution::Palette(PaletteOuterSolution {
            inner_idx,
            n_lookups: s.n_lookups + 1, // +1 for the palette array lookup
            n_extra_ops: s.n_extra_ops + extra_ops,
            cost: s.cost + palette_cost,
        }));
    }

    (palette, Some(palette_chain), solutions)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inner_all_zeros() {
        let chain = InnerLayerChain::new(vec![0, 0, 0, 0]);
        assert_eq!(chain.layers[0].max_v, 0);
        assert_eq!(chain.layers[0].min_v, 0);
        assert_eq!(chain.layers[0].unit_bits, 0);
    }

    #[test]
    fn test_inner_simple_data() {
        let chain = InnerLayerChain::new(vec![0, 1, 2, 3]);
        assert_eq!(chain.layers[0].max_v, 3);
        assert_eq!(chain.layers[0].unit_bits, 2);
        assert!(!chain.solutions.is_empty());
    }

    #[test]
    fn test_inner_has_mapping_after_split() {
        let chain = InnerLayerChain::new(vec![0, 1, 2, 3]);
        assert!(chain.layers[0].mapping.is_some());
        assert!(chain.layers.len() > 1);
    }

    #[test]
    fn test_inner_solutions_sorted() {
        let chain = InnerLayerChain::new((0..16).collect());
        let root_idxs = chain.root_solutions();
        let lookups: Vec<usize> = root_idxs
            .iter()
            .map(|&i| chain.solutions[i].n_lookups)
            .collect();
        let mut sorted = lookups.clone();
        sorted.sort();
        assert_eq!(lookups, sorted);
    }

    #[test]
    fn test_inner_solutions_not_dominated() {
        let chain = InnerLayerChain::new((0..256).map(|x| x as i64).collect());
        let root_idxs = chain.root_solutions();
        for &a in &root_idxs {
            for &b in &root_idxs {
                if a == b {
                    continue;
                }
                let sa = &chain.solutions[a];
                let sb = &chain.solutions[b];
                assert!(
                    !(sa.n_lookups <= sb.n_lookups && sa.full_cost() <= sb.full_cost()),
                    "Found dominated solution"
                );
            }
        }
    }

    #[test]
    fn test_outer_strips_trailing_default() {
        let layer = OuterLayerInfo::new(&[1, 2, 3, 0, 0, 0], 0);
        assert_eq!(layer.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_outer_bias_optimization() {
        // bias gets baked in when original data fits in same type
        // Use non-linear data so identity optimization doesn't interfere
        let layer = OuterLayerInfo::new(&[100, 105, 110, 115], 0);
        assert_eq!(layer.bias, 0);

        // bias kept when baking in would enlarge the type
        let layer = OuterLayerInfo::new(&[1000, 1005, 1010, 1015], 0);
        assert_eq!(layer.bias, 1000);
    }

    #[test]
    fn test_outer_gcd_optimization() {
        // mult gets baked in when undivided data fits in same type
        let layer = OuterLayerInfo::new(&[0, 10, 20, 30], 0);
        assert_eq!(layer.mult, 1);

        // mult kept when baking in would enlarge the type
        let layer = OuterLayerInfo::new(&[0, 128, 256, 384], 0);
        assert_eq!(layer.mult, 128);
    }

    #[test]
    fn test_outer_has_solutions() {
        let layer = OuterLayerInfo::new(&[1, 2, 3], 0);
        assert!(!layer.solutions.is_empty());
    }

    #[test]
    fn test_identity_chosen_for_linear_data() {
        let data: Vec<i64> = (0..16).collect();
        let layer = OuterLayerInfo::new(&data, 0);
        assert!(layer.identity);
    }

    #[test]
    fn test_identity_not_chosen_for_nonlinear() {
        let layer = OuterLayerInfo::new(&[0, 5, 10, 15], 0);
        assert!(!layer.identity);
    }

    #[test]
    fn test_identity_with_offset() {
        let data: Vec<i64> = (0..8).map(|i| 100 + i).collect();
        let layer = OuterLayerInfo::new(&data, 0);
        assert!(layer.identity);
    }

    // ── Deep-chain tests ────────────────────────────────────────────────────
    //
    // For these tests we use a repeating 0..16 pattern over 16 000 elements.
    // With 16 unique pair patterns the chain builds 5 inner layers (layers 0-4,
    // where layer 4 is the all-zero constant).  The 1-lookup wrap-constant
    // solution is so cheap (8 bytes) that it dominates all multi-lookup
    // alternatives, so exactly one solution survives Pareto pruning.

    fn deep_chain_data() -> Vec<i64> {
        // (0..16) repeated 1000 times → 16 000 elements, 5 inner layers.
        (0i64..16).cycle().take(16_000).collect()
    }

    /// The chain for (0..16)*1000 data must build exactly 5 layers,
    /// with the deepest being the all-zero constant layer.
    #[test]
    fn test_inner_deep_chain_builds_five_layers() {
        let chain = InnerLayerChain::new(deep_chain_data());
        assert_eq!(
            chain.layers.len(), 5,
            "Expected 5-layer chain for (0..16) repeating data, got {}",
            chain.layers.len()
        );
        assert_eq!(
            chain.layers.last().unwrap().max_v, 0,
            "Deepest layer must be the all-zero constant"
        );
    }

    /// All root solutions for a deep chain must be Pareto-optimal.
    #[test]
    fn test_inner_deep_chain_pareto_optimal() {
        let chain = InnerLayerChain::new(deep_chain_data());
        let root_idxs = chain.root_solutions();
        for &a in &root_idxs {
            for &b in &root_idxs {
                if a == b {
                    continue;
                }
                let sa = &chain.solutions[a];
                let sb = &chain.solutions[b];
                assert!(
                    !(sa.n_lookups <= sb.n_lookups && sa.full_cost() <= sb.full_cost()),
                    "Solution (nl={}, fc={}) dominates (nl={}, fc={})",
                    sa.n_lookups,
                    sa.full_cost(),
                    sb.n_lookups,
                    sb.full_cost()
                );
            }
        }
    }

    /// Root solutions must be sorted by n_lookups ascending.
    #[test]
    fn test_inner_deep_chain_lookups_sorted() {
        let chain = InnerLayerChain::new(deep_chain_data());
        let root_idxs = chain.root_solutions();
        let lookups: Vec<usize> = root_idxs
            .iter()
            .map(|&i| chain.solutions[i].n_lookups)
            .collect();
        let mut sorted = lookups.clone();
        sorted.sort();
        assert_eq!(lookups, sorted, "Root solutions not sorted by n_lookups");
    }

    /// When sorted by n_lookups, full_cost must strictly decrease
    /// (otherwise dominated solutions would have survived pruning).
    #[test]
    fn test_inner_deep_chain_cost_strictly_decreases() {
        let chain = InnerLayerChain::new(deep_chain_data());
        let root_idxs = chain.root_solutions();
        let mut prev_cost = usize::MAX;
        for &i in &root_idxs {
            let fc = chain.solutions[i].full_cost();
            assert!(
                fc < prev_cost,
                "full_cost did not strictly decrease: {} >= {}",
                fc,
                prev_cost
            );
            prev_cost = fc;
        }
    }

    /// pick_solution must return a valid index and choose a compact solution.
    /// For (0..16)*1000 the 1-lookup wrap-constant (8 bytes) is optimal.
    #[test]
    fn test_inner_deep_chain_pick_solution_valid() {
        use crate::pick_solution;

        let data: Vec<i64> = (0i64..16).cycle().take(16_000).collect();
        let info = OuterLayerInfo::new(&data, 0);
        assert!(!info.solutions.is_empty(), "Must have at least one solution");
        let best = pick_solution(&info.solutions, 9.0);
        assert!(best < info.solutions.len(), "pick_solution returned out-of-bounds index");
        // The chosen solution should be far smaller than naive flat storage
        // (16000 values × 4 bits = 8000 bytes).
        assert!(
            info.solutions[best].cost() < 100,
            "High-compression should pick a compact solution, got {} bytes",
            info.solutions[best].cost()
        );
    }
}
