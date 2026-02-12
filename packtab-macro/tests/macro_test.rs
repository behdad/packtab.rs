#[allow(unused_parens, non_upper_case_globals)]
mod test_macro {
    packtab_macro::pack_table! {
        fn lookup(u: usize) -> u8 {
            data: [10, 20, 30, 40, 50, 60, 70, 80],
            default: 0,
        }
    }

    #[test]
    fn test_lookup_values() {
        assert_eq!(lookup(0), 10);
        assert_eq!(lookup(1), 20);
        assert_eq!(lookup(2), 30);
        assert_eq!(lookup(3), 40);
        assert_eq!(lookup(4), 50);
        assert_eq!(lookup(5), 60);
        assert_eq!(lookup(6), 70);
        assert_eq!(lookup(7), 80);
    }

    #[test]
    fn test_lookup_default() {
        assert_eq!(lookup(8), 0);
        assert_eq!(lookup(100), 0);
    }
}

#[allow(unused_parens, non_upper_case_globals)]
mod test_macro_large {
    packtab_macro::pack_table! {
        pub fn big_lookup(u: usize) -> u16 {
            data: [0, 100, 200, 300, 400, 500, 600, 700, 800, 900, 1000,
                   1100, 1200, 1300, 1400, 1500],
            default: 0,
        }
    }

    #[test]
    fn test_large_values() {
        assert_eq!(big_lookup(0), 0);
        assert_eq!(big_lookup(5), 500);
        assert_eq!(big_lookup(15), 1500);
        assert_eq!(big_lookup(16), 0);
    }
}
