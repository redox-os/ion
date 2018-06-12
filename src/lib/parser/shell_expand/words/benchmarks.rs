use super::*;

use test::Bencher;

struct DummyExpander;

impl Expander for DummyExpander {}


#[bench]
fn simple_no_glob(b: &mut Bencher) {
    b.iter(|| {
        WordIterator::new("L*", &DummyExpander, false).for_each(drop);
    })
}

#[bench]
fn braces_no_glob(b: &mut Bencher) {
    b.iter(|| {
        WordIterator::new("{a,}b", &DummyExpander, false).for_each(drop);
    })
}
