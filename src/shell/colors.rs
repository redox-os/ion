use fnv::FnvHashMap;
use std::u16;

lazy_static! {
    static ref ATTRIBUTES: FnvHashMap<&'static str, &'static str> = {
        let mut map = FnvHashMap::default();
        map.insert("bold", "1");
        map.insert("dim", "2");
        map.insert("underlined", "4");
        map.insert("blink", "5");
        map.insert("reverse", "7");
        map.insert("hidden", "8");
        map
    };
}

lazy_static! {
    static ref COLORS: FnvHashMap<&'static str, &'static str> = {
        let mut map = FnvHashMap::default();
        map.insert("black", "30");
        map.insert("red", "31");
        map.insert("green", "32");
        map.insert("yellow", "33");
        map.insert("blue", "34");
        map.insert("magenta", "35");
        map.insert("cyan", "36");
        map.insert("light_gray", "37");
        map.insert("default", "39");
        map.insert("dark_gray", "90");
        map.insert("light_red", "91");
        map.insert("light_green", "92");
        map.insert("light_yellow", "93");
        map.insert("light_blue", "94");
        map.insert("light_magenta", "95");
        map.insert("light_cyan", "96");
        map
    };
}

lazy_static! {
    static ref BG_COLORS: FnvHashMap<&'static str, &'static str> = {
        let mut map = FnvHashMap::default();
        map.insert("blackbg", "40");
        map.insert("redbg", "41");
        map.insert("greenbg", "42");
        map.insert("yellowbg", "43");
        map.insert("bluebg", "44");
        map.insert("magentabg", "45");
        map.insert("cyanbg", "46");
        map.insert("light_graybg", "47");
        map.insert("defaultbg", "49");
        map.insert("dark_graybg", "100");
        map.insert("light_redbg", "101");
        map.insert("light_greenbg", "102");
        map.insert("light_yellowbg",  "103");
        map.insert("light_bluebg", "104");
        map.insert("light_magentabg", "105");
        map.insert("light_cyanbg", "106");
        map.insert("whitebg", "107");
        map
    };
}

#[derive(Debug, PartialEq)]
/// Colors may be called by name, or by a hexadecimal-converted decimal value.
enum Mode {
    Name(&'static str),
    bits256(u16),
}

#[derive(Debug, PartialEq)]
/// Stores a reprensetation of text formatting data which can be used to get an ANSI color code.
pub struct Colors {
    foreground: Option<Mode>,
    background: Option<Mode>,
    attributes: Option<Vec<&'static str>>
}

impl Colors {
    /// Parses the given input and returns a structure obtaining the text data needed for proper
    /// transformation into ANSI code parameters, which may be obtained by calling the
    /// `into_string()` method on the newly-created `Colors` structure.
    pub fn collect(input: &str) -> Colors {
        let mut colors = Colors { foreground: None, background: None, attributes: None };
        for variable in input.split(",") {
            if variable == "reset" {
                return Colors { foreground: None, background: None, attributes: Some(vec!["0"]) };
            } else if let Some(attribute) = ATTRIBUTES.get(&variable) {
                colors.append_attribute(attribute);
            } else if let Some(color) = COLORS.get(&variable) {
                colors.foreground = Some(Mode::Name(*color));
            } else if let Some(color) = BG_COLORS.get(&variable) {
                colors.background = Some(Mode::Name(*color));
            } else if !colors.set_256bit_color(variable) {
                eprintln!("ion: {} is not a valid color", variable)
            }
        }
        colors
    }

    /// Attributes can be stacked, so this function serves to enable that stacking.
    fn append_attribute(&mut self, attribute: &'static str) {
        let vec_exists = match self.attributes.as_mut() {
            Some(vec) => { vec.push(attribute); true },
            None => false
        };

        if !vec_exists {
            self.attributes = Some(vec![attribute]);
        }
    }

    /// If no matches were made, then this will attempt to parse the variable as either a
    /// two-digit hexadecimal value, or a decimal value, which corresponds to a 256-bit color.
    fn set_256bit_color(&mut self, variable: &str) -> bool {
        if variable.len() > 3 && variable.starts_with("0x") {
            if let Ok(value) = u16::from_str_radix(&variable[2..4], 16) {
                if variable.ends_with("bg") && variable.len() == 6 {
                    self.background = Some(Mode::bits256(value));
                    return true;
                } else if variable.len() == 4 {
                    self.foreground = Some(Mode::bits256(value));
                    return true;
                }
            }
        } else if variable.ends_with("bg") && variable.len() > 2 {
            let (number, _) = variable.split_at(variable.len()-2);
            if let Ok(value) = number.parse::<u16>() {
                self.background = Some(Mode::bits256(value));
                return true
            }
        } else if let Ok(value) = variable.parse::<u16>() {
            self.foreground = Some(Mode::bits256(value));
            return true
        }

        false
    }

    /// Attempts to transform the data in the structure into the corresponding ANSI code
    /// representation. It would very ugly to require shell scripters to have to interface
    /// with these codes directly.
    pub fn into_string(self) -> Option<String> {
        let mut output = String::from("\x1b[");

        let foreground = match self.foreground {
            Some(Mode::Name(string)) => Some(string.to_owned()),
            Some(Mode::bits256(value)) => Some(format!("38;5;{}", value)),
            None => None
        };

        let background = match self.background {
            Some(Mode::Name(string)) => Some(string.to_owned()),
            Some(Mode::bits256(value)) => Some(format!("48;5;{}", value)),
            None => None
        };

        if let Some(attr) = self.attributes {
            output.push_str(&attr.join(";"));
            match (foreground, background) {
                (Some(c), None) | (None, Some(c)) => Some([&output, ";", &c, "m"].concat()),
                (None, None) => Some([&output, "m"].concat()),
                (Some(fg), Some(bg)) => Some([&output, ";", &fg, ";", &bg, "m"].concat())
            }
        } else {
            match (foreground, background) {
                (Some(c), None) | (None, Some(c)) => Some([&output, &c, "m"].concat()),
                (None, None) => None,
                (Some(fg), Some(bg)) => Some([&output, &fg, ";", &bg, "m"].concat())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn set_multiple_color_attributes() {
        let expected = Colors {
            attributes: Some(vec!["1", "4", "5"]),
            background: None,
            foreground: None,
        };
        let actual = Colors::collect("bold,underlined,blink");
        assert_eq!(actual, expected);
        assert_eq!(Some("\x1b[1;4;5m".to_owned()), actual.into_string());
    }

    #[test]
    fn set_multiple_colors() {
        let expected = Colors {
            attributes: Some(vec!["1"]),
            background: Some(Mode::Name("107")),
            foreground: Some(Mode::Name("35"))
        };
        let actual = Colors::collect("whitebg,magenta,bold");
        assert_eq!(actual, expected);
        assert_eq!(Some("\x1b[1;35;107m".to_owned()), actual.into_string());
    }

    #[test]
    fn hexadecimal_256bit_colors() {
        let expected = Colors {
            attributes: None,
            background: Some(Mode::bits256(77)),
            foreground: Some(Mode::bits256(75))
        };
        let actual = Colors::collect("0x4b,0x4dbg");
        assert_eq!(actual, expected);
        assert_eq!(Some("\x1b[38;5;75;48;5;77m".to_owned()), actual.into_string())
    }

    #[test]
    fn decimal_256bit_colors() {
        let expected = Colors {
            attributes: None,
            background: Some(Mode::bits256(78)),
            foreground: Some(Mode::bits256(32))
        };
        let actual = Colors::collect("78bg,32");
        assert_eq!(actual, expected);
        assert_eq!(Some("\x1b[38;5;32;48;5;78m".to_owned()), actual.into_string())
    }
}
