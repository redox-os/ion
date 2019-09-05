use criterion::*;
use ion_shell::parser::{StatementSplitter, Terminator};
use itertools::Itertools;

const TEXT: &[u8] = include_bytes!("test.ion");

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("statement-splitter-Throughput");

    for stmt in TEXT.iter().cloned().batching(|lines| Terminator::new(lines).terminate()) {
        group.throughput(Throughput::Bytes(stmt.len() as u64));

        group.bench_with_input(BenchmarkId::new("statement", stmt.len()), &stmt, |b, stmt| {
            b.iter(|| StatementSplitter::new(stmt).collect::<Vec<_>>())
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
