/// Returns the smallest power-of-two bit width that can store values in [min_v, max_v].
///
/// Returned widths: 0, 1, 2, 4, 8, 16, 32, 64.
/// Sub-byte widths (0/1/2/4) correspond to packing granularities;
/// 8/16/32/64 correspond to standard integer types.
pub fn binary_bits_for(min_v: i64, max_v: i64) -> u8 {
    assert!(min_v <= max_v, "min_v ({}) > max_v ({})", min_v, max_v);

    if 0 <= min_v && max_v <= 0 {
        return 0;
    }
    if 0 <= min_v && max_v <= 1 {
        return 1;
    }
    if 0 <= min_v && max_v <= 3 {
        return 2;
    }
    if 0 <= min_v && max_v <= 15 {
        return 4;
    }

    if 0 <= min_v && max_v <= 255 {
        return 8;
    }
    if -128 <= min_v && max_v <= 127 {
        return 8;
    }

    if 0 <= min_v && max_v <= 65535 {
        return 16;
    }
    if -32768 <= min_v && max_v <= 32767 {
        return 16;
    }

    if 0 <= min_v && max_v <= 4_294_967_295 {
        return 32;
    }
    if -2_147_483_648 <= min_v && max_v <= 2_147_483_647 {
        return 32;
    }

    // Everything else fits in 64 bits (we only support i64 range)
    64
}

/// Compute GCD of all values in an iterator.
pub fn gcd(values: impl IntoIterator<Item = i64>) -> i64 {
    let mut it = values.into_iter();
    let mut x = match it.next() {
        Some(v) => v.abs(),
        None => return 1,
    };
    for y_raw in it {
        let mut y = y_raw.abs();
        while y != 0 {
            let t = y;
            y = x % y;
            x = t;
        }
        if x == 1 {
            break;
        }
    }
    x
}

/// Pack sub-byte values into bytes.
///
/// If `bits` is 1, 2, or 4, multiple values are combined into each byte.
pub fn combine(mut data: Vec<i64>, bits: u8) -> Vec<i64> {
    if bits <= 1 {
        data = combine2(data, |a, b| (b << 1) | a);
    }
    if bits <= 2 {
        data = combine2(data, |a, b| (b << 2) | a);
    }
    if bits <= 4 {
        data = combine2(data, |a, b| (b << 4) | a);
    }
    data
}

/// Pairwise reduce: combine adjacent elements using function `f`.
pub fn combine2(data: Vec<i64>, f: fn(i64, i64) -> i64) -> Vec<i64> {
    let mut result = Vec::with_capacity((data.len() + 1) / 2);
    let mut it = data.iter().copied();
    while let Some(first) = it.next() {
        let second = it.next().unwrap_or(0);
        result.push(f(first, second));
    }
    result
}

/// Recursively expand a mapping index back into flat data values.
///
/// During splitting, pairs of values are mapped to single indices via
/// AutoMapping. This function reverses that: given an index `v` at
/// level `i` of the `stack`, it looks up the pair, and recursively
/// expands each half, appending leaf values to `out`.
pub fn expand(
    v: usize,
    mappings: &[&crate::mapping::AutoMapping],
    i: usize,
    out: &mut Vec<i64>,
) {
    if i == 0 {
        out.push(v as i64);
        return;
    }
    let pair = mappings[i - 1].get_pair(v);
    expand(pair.0, mappings, i - 1, out);
    expand(pair.1, mappings, i - 1, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_bits_for_zero() {
        assert_eq!(binary_bits_for(0, 0), 0);
    }

    #[test]
    fn test_binary_bits_for_1bit() {
        assert_eq!(binary_bits_for(0, 1), 1);
    }

    #[test]
    fn test_binary_bits_for_2bit() {
        assert_eq!(binary_bits_for(0, 2), 2);
        assert_eq!(binary_bits_for(0, 3), 2);
    }

    #[test]
    fn test_binary_bits_for_4bit() {
        assert_eq!(binary_bits_for(0, 4), 4);
        assert_eq!(binary_bits_for(0, 15), 4);
    }

    #[test]
    fn test_binary_bits_for_8bit_unsigned() {
        assert_eq!(binary_bits_for(0, 16), 8);
        assert_eq!(binary_bits_for(0, 255), 8);
    }

    #[test]
    fn test_binary_bits_for_8bit_signed() {
        assert_eq!(binary_bits_for(-128, 127), 8);
        assert_eq!(binary_bits_for(-1, 0), 8);
    }

    #[test]
    fn test_binary_bits_for_16bit() {
        assert_eq!(binary_bits_for(0, 256), 16);
        assert_eq!(binary_bits_for(0, 65535), 16);
        assert_eq!(binary_bits_for(-32768, 32767), 16);
    }

    #[test]
    fn test_binary_bits_for_32bit() {
        assert_eq!(binary_bits_for(0, 65536), 32);
        assert_eq!(binary_bits_for(0, (1i64 << 32) - 1), 32);
        assert_eq!(binary_bits_for(-(1i64 << 31), (1i64 << 31) - 1), 32);
    }

    #[test]
    fn test_binary_bits_for_64bit() {
        assert_eq!(binary_bits_for(0, 1i64 << 32), 64);
    }

    #[test]
    fn test_gcd_empty() {
        assert_eq!(gcd(std::iter::empty()), 1);
    }

    #[test]
    fn test_gcd_single() {
        assert_eq!(gcd([48]), 48);
    }

    #[test]
    fn test_gcd_negative() {
        assert_eq!(gcd([-48]), 48);
    }

    #[test]
    fn test_gcd_pair() {
        assert_eq!(gcd([48, 60]), 12);
    }

    #[test]
    fn test_gcd_multiple() {
        assert_eq!(gcd([48, 60, 6]), 6);
    }

    #[test]
    fn test_gcd_coprime() {
        assert_eq!(gcd([48, 61, 6]), 1);
    }

    #[test]
    fn test_gcd_all_same() {
        assert_eq!(gcd([7, 7, 7]), 7);
    }

    #[test]
    fn test_combine2_basic() {
        let data = vec![1, 2, 3, 4];
        let result = combine2(data, |a, b| (b << 4) | a);
        assert_eq!(result, vec![0x21, 0x43]);
    }

    #[test]
    fn test_combine2_odd_length() {
        let data = vec![1, 2, 3];
        let result = combine2(data, |a, b| (b << 4) | a);
        assert_eq!(result, vec![0x21, 0x03]);
    }

    #[test]
    fn test_combine_1bit() {
        let data = vec![0, 1, 1, 0, 1, 0, 0, 1];
        let result = combine(data, 1);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_combine_2bit() {
        let data = vec![0, 1, 2, 3];
        let result = combine(data, 2);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_combine_4bit() {
        let data = vec![5, 10, 3, 7];
        let result = combine(data, 4);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_combine_8bit_noop() {
        let data = vec![100, 200, 50];
        let result = combine(data.clone(), 8);
        assert_eq!(result, data);
    }
}
