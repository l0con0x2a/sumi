use std::time::Instant;
use sumi_skk::{Engine, Key, dictionary::Dictionary};
fn main() {
    let start = Instant::now();
    let d = std::env::args()
        .nth(1)
        .map_or_else(|| Ok(Dictionary::demo()), Dictionary::load)
        .expect("dictionary");
    eprintln!("dictionary load: {:?}", start.elapsed());
    let mut e = Engine::default();
    let keys = [
        Key::Char('K'),
        Key::Char('a'),
        Key::Char('n'),
        Key::Char('j'),
        Key::Char('i'),
        Key::Space,
        Key::Enter,
    ];
    let mut samples = Vec::with_capacity(70000);
    for _ in 0..10000 {
        for key in &keys {
            let start = Instant::now();
            std::hint::black_box(e.process_key(key.clone(), &d));
            samples.push(start.elapsed().as_nanos());
        }
    }
    samples.sort_unstable();
    println!(
        "{} keys: p50={}ns p95={}ns p99={}ns max={}ns",
        samples.len(),
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
        samples[samples.len() * 99 / 100],
        samples.last().unwrap()
    );
}
