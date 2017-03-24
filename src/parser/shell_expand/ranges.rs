pub fn parse_range(input: &str) -> Option<Vec<String>> {
    let mut bytes_iterator = input.bytes().enumerate();
    while let Some((id, byte)) = bytes_iterator.next() {
        match byte {
            b'0'...b'9' | b'-' | b'a'...b'z' | b'A'...b'Z' => continue,
            b'.' => {
                let first = &input[..id];

                let mut dots = 1;
                while let Some((_, byte)) = bytes_iterator.next() {
                    if byte == b'.' { dots += 1 } else { break }
                }

                if dots != 2 { break }

                let end = &input[id+dots..];

                if let Ok(start) = first.parse::<isize>() {
                    if let Ok(end) = end.parse::<isize>() {
                        return if start < end {
                            Some((start..end+1).map(|x| x.to_string()).collect())
                        } else if start > end {
                            Some((end..start+1).rev().map(|x| x.to_string()).collect())
                        } else {
                            Some(vec![first.to_owned()])
                        }
                    }
                } else if first.len() == 1 && end.len() == 1 {
                    let start = first.bytes().next().unwrap();
                    let end = end.bytes().next().unwrap();

                    let is_valid = ((start >= b'a' && start <= b'z') && (end >= b'a' && end <= b'z'))
                     || ((start >= b'A' && start <= b'Z') && (end >= b'A' && end <= b'Z'));

                    if !is_valid { break }
                    return if start < end {
                        Some((start..end+1).map(|x| {
                            let mut output = String::with_capacity(1);
                            output.push(x as char);
                            output
                        }).collect())
                    } else if start > end {
                        Some((end..start+1).rev().map(|x| {
                            let mut output = String::with_capacity(1);
                            output.push(x as char);
                            output
                        }).collect())
                    } else {
                        Some(vec![first.to_owned()])
                    }
                } else {
                    break
                }
            },
            _ => break
        }
    }

    None
}
