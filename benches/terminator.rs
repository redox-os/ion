use criterion::*;
use ion_shell::parser::Terminator;
use std::time::Duration;

const TEXT: &str = include_str!("test.ion");
const EOF: &str = include_str!("herestring.ion");

fn criterion_benchmark(c: &mut Criterion) {
    c.bench(
        "terminator-throughput",
        ParameterizedBenchmark::new(
            "terminator",
            |b, script| {
                b.iter(|| {
                    let mut bytes = script.bytes().peekable();
                    while bytes.peek().is_some() {
                        let stmt = Terminator::new(&mut bytes).terminate();
                    }
                })
            },
            vec![TEXT, EOF],
        )
        .warm_up_time(Duration::from_secs(10))
        .measurement_time(Duration::from_secs(300))
        .throughput(|script| Throughput::Bytes(script.len() as u32)),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
