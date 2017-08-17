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

enum Mode {
    Name(&'static str),
    bits256(u16),
}

pub struct Colors {
    foreground: Option<Mode>,
    background: Option<Mode>,
    attribute: Option<Vec<&'static str>>
}

impl Colors {
    pub fn collect(input: &str) -> Colors {
        let mut colors = Colors { foreground: None, background: None, attribute: None };
        for variable in input.split(",") {
            if variable == "reset" {
                return Colors { foreground: None, background: None, attribute: Some(vec!["0"]) };
            } else if let Some(color) = ATTRIBUTES.get(&variable) {
                let vec_exists = match colors.attribute.as_mut() {
                    Some(vec) => { vec.push(color); true },
                    None => false
                };

                if !vec_exists {
                    colors.attribute = Some(vec![color]);
                }
            } else if let Some(color) = COLORS.get(&variable) {
                colors.foreground = Some(Mode::Name(*color))
            } else {
                match BG_COLORS.get(&variable) {
                    Some(color) => colors.background = Some(Mode::Name(*color)),
                    None => if let Ok(value) = u16::from_str_radix(&variable[0..2], 16) {
                        if variable.ends_with("bg") {
                            colors.background = Some(Mode::bits256(value));
                        } else {
                            colors.foreground = Some(Mode::bits256(value));
                        }
                    } else {
                        eprintln!("ion: {} is not a valid color", variable)
                    }
                }
            }
        }
        colors
    }

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

        if let Some(attr) = self.attribute {
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
