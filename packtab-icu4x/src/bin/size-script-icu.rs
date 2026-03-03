use icu_collections::codepointtrie::TrieValue;
use icu_properties::props::Script;
use icu_properties::CodePointMapData;
use std::hint::black_box;

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
    let script = CodePointMapData::<Script>::new();
    let acc = data.iter().fold(0u32, |acc, &cp| {
        let value = if cp > char::MAX as u32 {
            script.get32(u32::MAX).to_u32()
        } else {
            script.get32(cp).to_u32()
        };
        acc ^ value
    });
    println!("{}", black_box(acc));
}
