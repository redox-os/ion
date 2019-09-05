use criterion::*;
use ion_shell::parser::Terminator;

const TEXT: &str = include_str!("test.ion");
const EOF: &str = include_str!("herestring.ion");

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("terminator-Throughput");
    for script in &[TEXT, EOF] {
        group.throughput(Throughput::Bytes(script.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("terminator", script.len()),
            &script,
            |b, script| {
                b.iter(|| {
                    let mut bytes = script.bytes().peekable();
                    while bytes.peek().is_some() {
                        let _ = Terminator::new(&mut bytes).terminate();
                    }
                })
            },
        );
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
