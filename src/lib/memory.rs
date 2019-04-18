use object_pool::Pool;
use small::String;

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
    static STRINGS: Pool<String> = Pool::new(256, || String::with_capacity(MAX_SIZE));
}

pub struct IonPool;

impl IonPool {
    pub fn string<T, F: FnMut(&mut String) -> T>(mut callback: F) -> T {
        STRINGS.with(|pool| match pool.pull() {
            Some(ref mut string) => call_and_shrink!(string, callback),
            None => callback(&mut String::new()),
        })
    }
}
