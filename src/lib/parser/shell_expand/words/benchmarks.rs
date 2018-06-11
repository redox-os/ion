extern crate test;

use super::*;

use self::test::Bencher;

struct DummyExpander;

impl Expander for DummyExpander {}


#[bench]
fn simple_no_glob(b: &mut Bencher) {
    b.iter(|| {
        WordIterator::new_no_glob("L*", &DummyExpander).for_each(drop);
    })
}

#[bench]
fn braces_no_glob(b: &mut Bencher) {
    b.iter(|| {
        WordIterator::new_no_glob("{a,}b", &DummyExpander).for_each(drop);
    })
}
