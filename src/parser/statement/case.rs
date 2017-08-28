use super::split_pattern;

pub fn parse_case<'a>(data: &'a str) -> (&'a str, Option<&'a str>, Option<&'a str>) {
    let (key, conditional) = split_pattern(data, " if ");
    let (key, binding) = split_pattern(key, " @ ");
    (key.trim(), binding.map(|x| x.trim()), conditional.map(|x| x.trim()))
}

#[cfg(test)]
mod tests {
    use super::parse_case;
    #[test]
    fn case_parsing() {
        assert_eq!(("test", Some("test"), Some("exists")), parse_case("test @ test if exists"));
        assert_eq!(("test", Some("test"), None), parse_case("test @ test"));
        assert_eq!(("test", None, None), parse_case("test"));
    }
}
