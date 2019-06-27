use crate::expansion;
use itertools::Itertools;
use std::fmt;

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
pub struct Colors {
    foreground: Option<Mode>,
    background: Option<Mode>,
    attributes: Vec<&'static str>,
}

/// Transform the data in the structure into the corresponding ANSI code
/// representation. It would very ugly to require shell scripters to have to interface
/// with these codes directly.
impl fmt::Display for Colors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let foreground = self.foreground.as_ref().map(|fg| match fg {
            Mode::Name(string) => (*string).to_owned(),
            Mode::Range256(value) => format!("38;5;{}", value),
            Mode::TrueColor(red, green, blue) => format!("38;2;{};{};{}", red, green, blue),
        });

        let background = self.background.as_ref().map(|bkg| match bkg {
            Mode::Name(string) => (*string).to_owned(),
            Mode::Range256(value) => format!("48;5;{}", value),
            Mode::TrueColor(red, green, blue) => format!("48;2;{};{};{}", red, green, blue),
        });

        write!(
            f,
            "\x1b[{}m",
            self.attributes
                .iter()
                .cloned()
                .chain(foreground.as_ref().map(AsRef::as_ref))
                .chain(background.as_ref().map(AsRef::as_ref))
                .format(";")
        )
    }
}

impl Colors {
    /// If no matches were made, then this will attempt to parse the variable as either a
    /// 24-bit true color color, or one of 256 colors. It supports both hexadecimal and
    /// decimals.
    fn parse_colors(&mut self, variable: &str) -> Result<(), ()> {
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
                        return Ok(());
                    }
                }
                // 24-bit Color 0xRGB
                3 => {
                    let mut chars = variable.chars();
                    if let Some(red) = hex_char_to_u8_range(chars.next().unwrap()) {
                        if let Some(green) = hex_char_to_u8_range(chars.next().unwrap()) {
                            if let Some(blue) = hex_char_to_u8_range(chars.next().unwrap()) {
                                *field = Some(Mode::TrueColor(red, green, blue));
                                return Ok(());
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
                                return Ok(());
                            }
                        }
                    }
                }
                _ => (),
            }
        } else if let Ok(value) = variable.parse::<u8>() {
            *field = Some(Mode::Range256(value));
            return Ok(());
        }

        Err(())
    }

    /// Parses the given input and returns a structure obtaining the text data needed for proper
    /// transformation into ANSI code parameters, which may be obtained by calling the
    /// `into_string()` method on the newly-created `Colors` structure.
    pub fn collect<T: std::fmt::Display + std::fmt::Debug + std::error::Error>(
        input: &str,
    ) -> expansion::Result<Self, T> {
        let mut colors = Self { foreground: None, background: None, attributes: Vec::new() };
        for variable in input.split(',') {
            if variable == "reset" {
                return Ok(Self { foreground: None, background: None, attributes: vec!["0"] });
            } else if let Some(attribute) = ATTRIBUTES.get(variable) {
                colors.attributes.push(attribute);
            } else if let Some(color) = COLORS.get(variable) {
                colors.foreground = Some(Mode::Name(color));
            } else if let Some(color) = BG_COLORS.get(variable) {
                colors.background = Some(Mode::Name(color));
            } else {
                colors
                    .parse_colors(variable)
                    .map_err(|_| expansion::Error::ColorError(variable.into()))?;
            }
        }
        if colors.foreground.is_none()
            && colors.background.is_none()
            && colors.attributes.is_empty()
        {
            return Err(expansion::Error::EmptyColor);
        }
        Ok(colors)
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
    use crate::shell::IonError;

    #[test]
    fn convert_hex_digit() {
        assert_eq!(Some(255), hex_char_to_u8_range('F'));
        assert_eq!(Some(255), hex_char_to_u8_range('f'));
        assert_eq!(Some(0), hex_char_to_u8_range('0'));
    }

    #[test]
    fn set_multiple_color_attributes() {
        let expected =
            Colors { attributes: vec!["1", "4", "5"], background: None, foreground: None };
        let actual = Colors::collect::<IonError>("bold,underlined,blink").unwrap();
        assert_eq!(actual, expected);
        assert_eq!("\x1b[1;4;5m", &actual.to_string());
    }

    #[test]
    fn set_multiple_colors() {
        let expected = Colors {
            attributes: vec!["1"],
            background: Some(Mode::Name("107")),
            foreground: Some(Mode::Name("35")),
        };
        let actual = Colors::collect::<IonError>("whitebg,magenta,bold").unwrap();
        assert_eq!(actual, expected);
        assert_eq!("\x1b[1;35;107m", actual.to_string());
    }

    #[test]
    fn hexadecimal_256_colors() {
        let expected = Colors {
            attributes: Vec::default(),
            background: Some(Mode::Range256(77)),
            foreground: Some(Mode::Range256(75)),
        };
        let actual = Colors::collect::<IonError>("0x4b,0x4dbg").unwrap();
        assert_eq!(actual, expected);
        assert_eq!("\x1b[38;5;75;48;5;77m", &actual.to_string())
    }

    #[test]
    fn decimal_256_colors() {
        let expected = Colors {
            attributes: Vec::default(),
            background: Some(Mode::Range256(78)),
            foreground: Some(Mode::Range256(32)),
        };
        let actual = Colors::collect::<IonError>("78bg,32").unwrap();
        assert_eq!(actual, expected);
        assert_eq!("\x1b[38;5;32;48;5;78m", &actual.to_string())
    }

    #[test]
    fn three_digit_hex_24bit_colors() {
        let expected = Colors {
            attributes: Vec::default(),
            background: Some(Mode::TrueColor(255, 255, 255)),
            foreground: Some(Mode::TrueColor(0, 0, 0)),
        };
        let actual = Colors::collect::<IonError>("0x000,0xFFFbg").unwrap();
        assert_eq!(expected, actual);
        assert_eq!("\x1b[38;2;0;0;0;48;2;255;255;255m", &actual.to_string());
    }

    #[test]
    fn six_digit_hex_24bit_colors() {
        let expected = Colors {
            attributes: Vec::default(),
            background: Some(Mode::TrueColor(255, 0, 0)),
            foreground: Some(Mode::TrueColor(0, 255, 0)),
        };
        let actual = Colors::collect::<IonError>("0x00FF00,0xFF0000bg").unwrap();
        assert_eq!(expected, actual);
    }
}
