// TODO: Shrink values that grow too large, after use.

// use crate::types::Array;
use object_pool::Pool;
use small::String;

thread_local! {
    static STRINGS: Pool<String> = Pool::new(256, String::new);
    // static STRING_VECS: Pool<Array> = Pool::new(1024, Array::new?);
}

pub struct IonPool;

impl IonPool {
    pub fn string<T, F: FnMut(&mut String) -> T>(mut callback: F) -> T {
        STRINGS.with(|pool| match pool.pull() {
            Some(ref mut string) => {
                string.clear();
                callback(string)
            }
            None => callback(&mut String::new()),
        })
    }

    // pub fn vec_of_string<T, F: FnMut(&mut Array) -> T>(mut callback: F) -> T {
    //     STRING_VECS.with(|pool| {
    //         match pool.pull() {
    //             Some(ref mut vector) => {
    //                 vector.clear();
    //                 callback(vector)
    //             }
    //             None => callback(&mut Array::new())
    //         }
    //     })
    // }
}
