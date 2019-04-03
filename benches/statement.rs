use criterion::*;
use ion_shell::parser::{statement::StatementSplitter, Terminator};
use itertools::Itertools;

const TEXT: &[u8] = include_bytes!("test.ion");

fn criterion_benchmark(c: &mut Criterion) {
    let stmts = TEXT
        .iter()
        .cloned()
        .batching(|lines| Terminator::new(lines).terminate())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    c.bench(
        "statement-splitter-throughput",
        ParameterizedBenchmark::new(
            "statement",
            |b, script| {
                b.iter(|| {
                    script.iter().flat_map(|cmd| StatementSplitter::new(cmd)).collect::<Vec<_>>()
                })
            },
            vec![stmts],
        )
        .throughput(|script| Throughput::Bytes(script.len() as u32)),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
