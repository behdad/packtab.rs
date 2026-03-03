use std::hint::black_box;

include!(concat!(env!("OUT_DIR"), "/generated_benches.rs"));

fn corpus() -> Vec<u32> {
    let mut cps = Vec::new();
    cps.extend(0..=0x7f);
    cps.extend((0x80..=0x7ff).step_by(7));
    cps.extend((0x800..=0xffff).step_by(113));
    cps.extend((0x1_0000..=0x10_ffff).step_by(997));
    cps.push(0x11_0000);
    cps
}

fn main() {
    let data = corpus();
    let acc = data.iter().fold(0u32, |acc, &cp| acc ^ script_lookup_unsafe(cp));
    println!("{}", black_box(acc));
}
