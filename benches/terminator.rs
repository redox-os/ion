#[macro_use]
extern crate criterion;

use criterion::Criterion;
use ion_shell::parser::Terminator;

const TEXT: &str = include_str!("test.ion");

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("terminator", |b| b.iter(|| {
        let mut bytes = TEXT.bytes().peekable();
        while bytes.peek().is_some() {
            println!("{:?}", Terminator::new(&mut bytes).terminate());
        }
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
