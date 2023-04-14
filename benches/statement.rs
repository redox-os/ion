use criterion::*;
use ion_shell::parser::{StatementSplitter, Terminator};
use itertools::Itertools;

const TEXT: &[u8] = include_bytes!("test.ion");

fn criterion_benchmark(c: &mut Criterion) {
    let stmts = TEXT
        .iter()
        .copied()
        .batching(|lines| Terminator::new(lines).terminate())
        .collect::<Vec<_>>();

    let mut group = c.benchmark_group("statement_splitter_throughput");

    group.throughput(Throughput::Bytes(stmts.len() as u64));
    group.bench_function("statement_splitter", |b| {
        b.iter(|| stmts.iter().flat_map(|cmd| StatementSplitter::new(cmd)).collect::<Vec<_>>())
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
