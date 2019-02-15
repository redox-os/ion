#[derive(Debug)]
struct StaticMap {
    keys:   &'static [&'static str],
    values: &'static [&'static str],
}

impl StaticMap {
    fn get(&self, key: &str) -> Option<&'static str> {
        self.keys.binary_search(&key).ok().map(|pos| unsafe { *self.values.get_unchecked(pos) })
    }
}

macro_rules! map {
    ($($name:expr => $value:expr),+) => {{
        StaticMap {
            keys: &[$($name),+],
            values: &[$($value),+],
        }
    }
}}

const ATTRIBUTES: StaticMap = map!(
    "blink" => "5",
    "bold" => "1",
    "dim" => "2",
    "hidden" => "8",
    "reverse" => "7",
    "underlined" => "4"
);

const COLORS: StaticMap = map!(
    "black" => "30",
    "blue" => "34",
    "cyan" => "36",
    "dark_gray" => "90",
    "default" => "39",
    "green" => "32",
    "light_blue" => "94",
    "light_cyan" => "96",
    "light_gray" => "37",
    "light_green" => "92",
    "light_magenta" => "95",
    "light_red" => "91",
    "light_yellow" => "93",
    "magenta" => "35",
    "red" => "31",
    "yellow" => "33"
);

const BG_COLORS: StaticMap = map!(
    "blackbg" => "40",
    "bluebg" => "44",
    "cyanbg" => "46",
    "dark_graybg" => "100",
    "defaultbg" => "49",
    "greenbg" => "42",
    "light_bluebg" => "104",
    "light_cyanbg" => "106",
    "light_graybg" => "47",
    "light_greenbg" => "102",
    "light_magentabg" => "105",
    "light_redbg" => "101",
    "light_yellowbg" =>  "103",
    "magentabg" => "45",
    "redbg" => "41",
    "whitebg" => "107",
    "yellowbg" => "43"
);

#[derive(Debug, PartialEq)]
/// Colors may be called by name, or by a hexadecimal-converted decimal value.
enum Mode {
    Name(&'static str),
    Range256(u8),
    TrueColor(u8, u8, u8),
}

#[derive(Debug, PartialEq)]
/// Stores a reprensetation of text formatting data which can be used to get an
/// ANSI color code.
pub(crate) struct Colors {
    foreground: Option<Mode>,
    background: Option<Mode>,
    attributes: Option<Vec<&'static str>>,
}

impl Colors {
    /// Attempts to transform the data in the structure into the corresponding ANSI code
    /// representation. It would very ugly to require shell scripters to have to interface
    /// with these codes directly.
    pub(crate) fn into_string(self) -> Option<String> {
        let mut output = String::from("\x1b[");

        let foreground = match self.foreground {
            Some(Mode::Name(string)) => Some(string.to_owned()),
            Some(Mode::Range256(value)) => Some(format!("38;5;{}", value)),
            Some(Mode::TrueColor(red, green, blue)) => {
                Some(format!("38;2;{};{};{}", red, green, blue))
            }
            None => None,
        };

        let background = match self.background {
            Some(Mode::Name(string)) => Some(string.to_owned()),
            Some(Mode::Range256(value)) => Some(format!("48;5;{}", value)),
            Some(Mode::TrueColor(red, green, blue)) => {
                Some(format!("48;2;{};{};{}", red, green, blue))
            }
            None => None,
        };

        if let Some(attr) = self.attributes {
            output.push_str(&attr.join(";"));
            match (foreground, background) {
                (Some(c), None) | (None, Some(c)) => Some([&output, ";", &c, "m"].concat()),
                (None, None) => Some([&output, "m"].concat()),
                (Some(fg), Some(bg)) => Some([&output, ";", &fg, ";", &bg, "m"].concat()),
            }
        } else {
            match (foreground, background) {
                (Some(c), None) | (None, Some(c)) => Some([&output, &c, "m"].concat()),
                (None, None) => None,
                (Some(fg), Some(bg)) => Some([&output, &fg, ";", &bg, "m"].concat()),
            }
        }
    }

    /// If no matches were made, then this will attempt to parse the variable as either a
    /// 24-bit true color color, or one of 256 colors. It supports both hexadecimal and
    /// decimals.
    fn parse_colors(&mut self, variable: &str) -> bool {
        // First, determine which field we will write to.
        let (field, variable) = if variable.ends_with("bg") {
            (&mut self.background, &variable[..variable.len() - 2])
        } else {
            (&mut self.foreground, variable)
        };

        // Then, check if the value is a hexadecimal value
        if variable.starts_with("0x") {
            let variable = &variable[2..];

            match variable.len() {
                // 256 colors: 0xF | 0xFF
                1 | 2 => {
                    if let Ok(value) = u8::from_str_radix(variable, 16) {
                        *field = Some(Mode::Range256(value));
                        return true;
                    }
                }
                // 24-bit Color 0xRGB
                3 => {
                    let mut chars = variable.chars();
                    if let Some(red) = hex_char_to_u8_range(chars.next().unwrap()) {
                        if let Some(green) = hex_char_to_u8_range(chars.next().unwrap()) {
                            if let Some(blue) = hex_char_to_u8_range(chars.next().unwrap()) {
                                *field = Some(Mode::TrueColor(red, green, blue));
                                return true;
                            }
                        }
                    }
                }
                // 24-bit Color 0xRRGGBB
                6 => {
                    if let Ok(red) = u8::from_str_radix(&variable[0..2], 16) {
                        if let Ok(green) = u8::from_str_radix(&variable[2..4], 16) {
                            if let Ok(blue) = u8::from_str_radix(&variable[4..6], 16) {
                                *field = Some(Mode::TrueColor(red, green, blue));
                                return true;
                            }
                        }
                    }
                }
                _ => (),
            }
        } else if let Ok(value) = variable.parse::<u8>() {
            *field = Some(Mode::Range256(value));
            return true;
        }

        false
    }

    /// Attributes can be stacked, so this function serves to enable that
    /// stacking.
    fn append_attribute(&mut self, attribute: &'static str) {
        let vec_exists = match self.attributes.as_mut() {
            Some(vec) => {
                vec.push(attribute);
                true
            }
            None => false,
        };

        if !vec_exists {
            self.attributes = Some(vec![attribute]);
        }
    }

    /// Parses the given input and returns a structure obtaining the text data needed for proper
    /// transformation into ANSI code parameters, which may be obtained by calling the
    /// `into_string()` method on the newly-created `Colors` structure.
    pub(crate) fn collect(input: &str) -> Colors {
        let mut colors = Colors { foreground: None, background: None, attributes: None };
        for variable in input.split(',') {
            if variable == "reset" {
                return Colors { foreground: None, background: None, attributes: Some(vec!["0"]) };
            } else if let Some(attribute) = ATTRIBUTES.get(&variable) {
                colors.append_attribute(attribute);
            } else if let Some(color) = COLORS.get(&variable) {
                colors.foreground = Some(Mode::Name(color));
            } else if let Some(color) = BG_COLORS.get(&variable) {
                colors.background = Some(Mode::Name(color));
            } else if !colors.parse_colors(variable) {
                eprintln!("ion: {} is not a valid color", variable)
            }
        }
        colors
    }
}

fn hex_char_to_u8_range(character: char) -> Option<u8> {
    if character >= '0' && character <= '9' {
        Some((character as u8 - b'0') * 16)
    } else {
        // Convert the character to uppercase, if it isn't already.
        let mut character = character as u8 & !0x20;
        if character >= b'A' {
            character -= 54;
            if character < 17 {
                return Some(character * 15 + 15);
            }
        }
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn convert_hex_digit() {
        assert_eq!(Some(255), hex_char_to_u8_range('F'));
        assert_eq!(Some(255), hex_char_to_u8_range('f'));
        assert_eq!(Some(0), hex_char_to_u8_range('0'));
    }

    #[test]
    fn set_multiple_color_attributes() {
        let expected =
            Colors { attributes: Some(vec!["1", "4", "5"]), background: None, foreground: None };
        let actual = Colors::collect("bold,underlined,blink");
        assert_eq!(actual, expected);
        assert_eq!(Some("\x1b[1;4;5m".to_owned()), actual.into_string());
    }

    #[test]
    fn set_multiple_colors() {
        let expected = Colors {
            attributes: Some(vec!["1"]),
            background: Some(Mode::Name("107")),
            foreground: Some(Mode::Name("35")),
        };
        let actual = Colors::collect("whitebg,magenta,bold");
        assert_eq!(actual, expected);
        assert_eq!(Some("\x1b[1;35;107m".to_owned()), actual.into_string());
    }

    #[test]
    fn hexadecimal_256_colors() {
        let expected = Colors {
            attributes: None,
            background: Some(Mode::Range256(77)),
            foreground: Some(Mode::Range256(75)),
        };
        let actual = Colors::collect("0x4b,0x4dbg");
        assert_eq!(actual, expected);
        assert_eq!(Some("\x1b[38;5;75;48;5;77m".to_owned()), actual.into_string())
    }

    #[test]
    fn decimal_256_colors() {
        let expected = Colors {
            attributes: None,
            background: Some(Mode::Range256(78)),
            foreground: Some(Mode::Range256(32)),
        };
        let actual = Colors::collect("78bg,32");
        assert_eq!(actual, expected);
        assert_eq!(Some("\x1b[38;5;32;48;5;78m".to_owned()), actual.into_string())
    }

    #[test]
    fn three_digit_hex_24bit_colors() {
        let expected = Colors {
            attributes: None,
            background: Some(Mode::TrueColor(255, 255, 255)),
            foreground: Some(Mode::TrueColor(0, 0, 0)),
        };
        let actual = Colors::collect("0x000,0xFFFbg");
        assert_eq!(expected, actual);
        assert_eq!(Some("\x1b[38;2;0;0;0;48;2;255;255;255m".to_owned()), actual.into_string());
    }

    #[test]
    fn six_digit_hex_24bit_colors() {
        let expected = Colors {
            attributes: None,
            background: Some(Mode::TrueColor(255, 0, 0)),
            foreground: Some(Mode::TrueColor(0, 255, 0)),
        };
        let actual = Colors::collect("0x00FF00,0xFF0000bg");
        assert_eq!(expected, actual);
    }
}
