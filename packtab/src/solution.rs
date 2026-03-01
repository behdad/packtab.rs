/// Cost model constants.
pub const BYTES_PER_OP: usize = 4;
pub const LOOKUP_OPS: usize = 4;
pub const SUB_BYTE_ACCESS_OPS: usize = 4;

/// Compute fullCost: cost + (nLookups * LOOKUP_OPS + nExtraOps) * BYTES_PER_OP
pub fn full_cost(n_lookups: usize, n_extra_ops: usize, cost: usize) -> usize {
    cost + (n_lookups * LOOKUP_OPS + n_extra_ops) * BYTES_PER_OP
}

/// A solution produced by InnerLayer — one specific split depth.
#[derive(Debug, Clone)]
pub struct InnerSolution {
    /// Index of the layer in the InnerLayerChain that produced this solution.
    pub layer_idx: usize,
    /// Index of the child InnerSolution (in the chain's solutions vec), or None.
    pub next: Option<usize>,
    pub n_lookups: usize,
    pub n_extra_ops: usize,
    pub cost: usize,
    /// Number of index bits consumed at this level.
    pub bits: u8,
}

impl InnerSolution {
    pub fn full_cost(&self) -> usize {
        full_cost(self.n_lookups, self.n_extra_ops, self.cost)
    }
}

/// A solution wrapping an InnerSolution with OuterLayer's arithmetic.
#[derive(Debug, Clone)]
pub struct OuterSolution {
    /// Index of the InnerSolution this wraps (in the chain's solutions vec).
    pub inner_idx: usize,
    /// Number of index bits consumed at the outermost inner level.
    pub bits: u8,
    pub n_lookups: usize,
    pub n_extra_ops: usize,
    pub cost: usize,
}

impl OuterSolution {
    pub fn full_cost(&self) -> usize {
        full_cost(self.n_lookups, self.n_extra_ops, self.cost)
    }
}
