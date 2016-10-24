//! Types convertable to integers

/// Parse the string to a integer using a given radix
pub trait ToNum {
    fn to_num_radix(&self, radix: usize) -> usize;
    fn to_num_radix_signed(&self, radix: usize) -> isize;
    fn to_num(&self) -> usize;
    fn to_num_signed(&self) -> isize;
}

impl ToNum for str {
    fn to_num_radix(&self, radix: usize) -> usize {
        if radix == 0 {
            return 0;
        }

        let mut num = 0;
        for c in self.chars() {
            let digit = match c {
                '0'...'9' => c as usize - '0' as usize,
                'A'...'Z' => c as usize - 'A' as usize + 10,
                'a'...'z' => c as usize - 'a' as usize + 10,
                _ => break,
            };

            if digit >= radix {
                break;
            }

            num *= radix;
            num += digit;
        }

        num
    }

    /// Parse the string as a signed integer using a given radix
    fn to_num_radix_signed(&self, radix: usize) -> isize {
        if self.starts_with('-') {
            -(self[1..].to_num_radix(radix) as isize)
        } else {
            self.to_num_radix(radix) as isize
        }
    }

    /// Parse it as a unsigned integer in base 10
    fn to_num(&self) -> usize {
        self.to_num_radix(10)
    }

    /// Parse it as a signed integer in base 10
    fn to_num_signed(&self) -> isize {
        self.to_num_radix_signed(10)
    }
}
