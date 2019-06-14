use crate::types::Str;
use object_pool::Pool;

const MAX_SIZE: usize = 64;

macro_rules! call_and_shrink {
    ($value:ident, $callback:ident) => {{
        let result = $callback($value);
        if $value.len() > MAX_SIZE {
            $value.truncate(MAX_SIZE);
            $value.shrink_to_fit();
        }

        $value.clear();
        result
    }};
}

thread_local! {
    static STRINGS: Pool<Str> = Pool::new(256, || Str::with_capacity(MAX_SIZE));
}

pub struct IonPool;

impl IonPool {
    pub fn string<T, F: FnMut(&mut Str) -> T>(mut callback: F) -> T {
        STRINGS.with(|pool| match pool.pull() {
            Some(ref mut string) => call_and_shrink!(string, callback),
            None => callback(&mut Str::new()),
        })
    }
}
