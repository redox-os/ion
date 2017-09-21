use std::ascii::AsciiExt;
use std::mem::transmute;
use std::ops::DerefMut;

// TODO: These could be generalised to work on non-ASCII characters (and even
//       strings!) as long as the byte size of the needle and haystack match.

pub trait AsciiReplaceInPlace {
    fn ascii_replace_in_place(&mut self, needle: char, haystack: char);
}

pub trait AsciiReplace: Sized {
    fn ascii_replace(self, needle: char, haystack: char) -> Self;
}

impl<T: DerefMut<Target = str>> AsciiReplace for T {
    fn ascii_replace(mut self, needle: char, haystack: char) -> Self {
        self.ascii_replace_in_place(needle, haystack);
        self
    }
}

impl AsciiReplaceInPlace for str {
    // I tried replacing these `assert!` calls with `debug_assert!` but it looks
    // like they get const-folded away anyway since it doesn't affect the speed
    fn ascii_replace_in_place(&mut self, needle: char, haystack: char) {
        assert!(needle.is_ascii(), "AsciiReplace functions can only be used for ascii characters");
        assert!(
            haystack.is_ascii(),
            "AsciiReplace functions can only be used for ascii characters"
        );
        let (needle, haystack): (u32, u32) = (needle.into(), haystack.into());
        let (needle, haystack) = (needle as u8, haystack as u8);

        // This is safe because we verify that we don't modify non-ascii bytes
        let mut_bytes: &mut [u8] = unsafe { transmute(self) };
        // NOTE: When str_mut_extras is stable, use this line instead
        // let mut_bytes = unsafe { self.as_bytes_mut() };
        for chr in mut_bytes.iter_mut() {
            if *chr == needle {
                *chr = haystack;
            }
        }
    }
}
