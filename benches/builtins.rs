use criterion::*;
use std::collections::HashMap;

const BUILTINS: &str = include_str!("builtins.txt");
const CALLS: &str = include_str!("calls.txt");

fn dummy() {}

const FNS: &'static [fn(); 100] = &[dummy; 100];

fn criterion_benchmark(c: &mut Criterion) {
    let builtins = BUILTINS.lines().collect::<Vec<_>>();
    let mut hashmap = HashMap::<&str, &dyn Fn()>::new();
    let mut hashbrown = hashbrown::HashMap::<&str, &dyn Fn()>::new();
    for builtin in &builtins {
        hashmap.insert(builtin, &dummy);
        hashbrown.insert(builtin, &dummy);
    }

    c.bench(
        "builtins",
        ParameterizedBenchmark::new(
            "hashmap",
            move |b, calls| {
                b.iter(|| {
                    for call in calls {
                        hashmap.get(call).map(|builtin| builtin());
                    }
                })
            },
            vec![CALLS.lines().collect::<Vec<_>>()],
        )
        .with_function("hashbrown", move |b, calls| {
            b.iter(|| {
                for call in calls {
                    hashbrown.get(call).map(|builtin| builtin());
                }
            })
        })
        .with_function("slice", move |b, calls| {
            b.iter(|| {
                for call in calls {
                    builtins
                        .binary_search(&call)
                        .ok()
                        .map(|pos| unsafe { FNS.get_unchecked(pos)() });
                }
            })
        }),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
