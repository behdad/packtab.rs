use crate::mapping::AutoMapping;
use crate::solution::{InnerSolution, OuterSolution, SUB_BYTE_ACCESS_OPS};
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
            let cur = self.layers.last_mut().unwrap();
            if cur.data.len() & 1 != 0 {
                cur.data.push(0);
            }

            let mut mapping = AutoMapping::new();
            let mut data2 = Vec::with_capacity(cur.data.len() / 2);
            for pair in cur.data.chunks(2) {
                let a = pair[0] as usize;
                let b = pair[1] as usize;
                let id = mapping.get_or_insert((a, b));
                data2.push(id as i64);
            }
            cur.mapping = Some(mapping);
            data = data2;
        }
    }

    fn build_solutions(&mut self) {
        let n_layers = self.layers.len();

        // For each layer, if maxV == 0, it contributes a zero-cost solution.
        // The deepest layer with maxV == 0 gives a constant solution.
        if self.layers[n_layers - 1].max_v == 0 {
            self.solutions.push(InnerSolution {
                layer_idx: 0,
                next: None,
                n_lookups: 0,
                n_extra_ops: 0,
                cost: 0,
                bits: 0,
            });
        }

        // The root layer's flat solution (1 lookup, no split).
        if self.layers[0].max_v != 0 {
            let root = &self.layers[0];
            self.solutions.push(InnerSolution {
                layer_idx: 0,
                next: None,
                n_lookups: 1,
                n_extra_ops: root.extra_ops,
                cost: root.bytes,
                bits: 0,
            });
        }

        // For each child layer depth, combine child solutions with root's expansion cost.
        let root_unit_bits = self.layers[0].unit_bits;
        let root_extra_ops = self.layers[0].extra_ops;
        let mut bits: u8 = 1;

        for child_idx in 1..n_layers {
            let child = &self.layers[child_idx];
            let extra_cost = ((child.max_v as usize + 1) * (1usize << bits) * root_unit_bits as usize + 7) / 8;

            // Collect child layer's own solutions (flat + deeper splits).
            // First, the child's flat solution:
            let child_flat_base = self.solutions.len();

            if child.max_v == 0 {
                // Child is all-zero: 0 lookups, 0 cost
                self.solutions.push(InnerSolution {
                    layer_idx: 0,
                    next: Some(child_flat_base), // self-referential placeholder
                    n_lookups: 0 + 1,
                    n_extra_ops: 0 + root_extra_ops,
                    cost: 0 + extra_cost,
                    bits,
                });
                // Fix: the "next" should point to the constant solution
                // which is index 0 (the zero-cost constant solution).
                self.solutions.last_mut().unwrap().next = Some(0);
            } else {
                // Child's flat solution
                let child_sol_idx = self.solutions.len();
                self.solutions.push(InnerSolution {
                    layer_idx: child_idx,
                    next: None,
                    n_lookups: 1,
                    n_extra_ops: child.extra_ops,
                    cost: child.bytes,
                    bits: 0,
                });

                // Wrap child's flat solution at root level
                self.solutions.push(InnerSolution {
                    layer_idx: 0,
                    next: Some(child_sol_idx),
                    n_lookups: 1 + 1,
                    n_extra_ops: child.extra_ops + root_extra_ops,
                    cost: child.bytes + extra_cost,
                    bits,
                });
            }

            // Now for deeper children: if child_idx + 1 < n_layers,
            // the child itself has children that form deeper solutions.
            // We need to recursively consider all depths from child_idx onward.
            let mut sub_bits: u8 = 1;
            for grandchild_idx in (child_idx + 1)..n_layers {
                let grandchild = &self.layers[grandchild_idx];
                let child_unit_bits = self.layers[child_idx].unit_bits;
                let child_extra_ops = self.layers[child_idx].extra_ops;
                let sub_extra_cost = ((grandchild.max_v as usize + 1)
                    * (1usize << sub_bits)
                    * child_unit_bits as usize
                    + 7)
                    / 8;

                if grandchild.max_v == 0 {
                    // Grandchild constant → child solution with sub_bits split
                    let gc_const_idx = self.solutions.len();
                    self.solutions.push(InnerSolution {
                        layer_idx: child_idx,
                        next: None, // constant grandchild (inline zero)
                        n_lookups: 0 + 1,
                        n_extra_ops: 0 + child_extra_ops,
                        cost: 0 + sub_extra_cost,
                        bits: sub_bits,
                    });
                    // Actually, the constant solution for grandchild should
                    // have 0 lookups. The child wrapping it adds 1 lookup.
                    // Then root wrapping that adds 1 more.

                    // Wrap at root level
                    self.solutions.push(InnerSolution {
                        layer_idx: 0,
                        next: Some(gc_const_idx),
                        n_lookups: 1 + 1,
                        n_extra_ops: child_extra_ops + root_extra_ops,
                        cost: sub_extra_cost + extra_cost,
                        bits,
                    });
                } else {
                    // Grandchild flat solution
                    let gc_flat_idx = self.solutions.len();
                    self.solutions.push(InnerSolution {
                        layer_idx: grandchild_idx,
                        next: None,
                        n_lookups: 1,
                        n_extra_ops: grandchild.extra_ops,
                        cost: grandchild.bytes,
                        bits: 0,
                    });

                    // Wrap at child level
                    let child_wrapped_idx = self.solutions.len();
                    self.solutions.push(InnerSolution {
                        layer_idx: child_idx,
                        next: Some(gc_flat_idx),
                        n_lookups: 1 + 1,
                        n_extra_ops: grandchild.extra_ops + child_extra_ops,
                        cost: grandchild.bytes + sub_extra_cost,
                        bits: sub_bits,
                    });

                    // Wrap at root level
                    self.solutions.push(InnerSolution {
                        layer_idx: 0,
                        next: Some(child_wrapped_idx),
                        n_lookups: 1 + 1 + 1,
                        n_extra_ops: grandchild.extra_ops + child_extra_ops + root_extra_ops,
                        cost: grandchild.bytes + sub_extra_cost + extra_cost,
                        bits,
                    });
                }

                sub_bits += 1;
            }

            bits += 1;
        }
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
    /// OuterSolutions wrapping inner solutions.
    pub solutions: Vec<OuterSolution>,
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
        if bias != 0 && mult == 1 && *base.iter().min().unwrap() >= 0 {
            let base_min = *base.iter().min().unwrap();
            let base_max = *base.iter().max().unwrap();
            let current_min = *reduced.iter().min().unwrap();
            let current_max = *reduced.iter().max().unwrap();
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
        let mut solutions = Vec::new();
        for &sol_idx in &root_idxs {
            let s = &inner.solutions[sol_idx];
            solutions.push(OuterSolution {
                inner_idx: sol_idx,
                n_lookups: s.n_lookups,
                n_extra_ops: s.n_extra_ops + extra_ops,
                cost: s.cost,
            });
        }

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
            solutions,
        }
    }
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
        let layer = OuterLayerInfo::new(&[100, 101, 102, 103], 0);
        assert_eq!(layer.bias, 0);

        // bias kept when baking in would enlarge the type
        let layer = OuterLayerInfo::new(&[1000, 1001, 1002, 1003], 0);
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
}
