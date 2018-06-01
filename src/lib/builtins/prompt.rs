use shell::{colors::COLORS, Shell};
use std::io::{self, Write, StdoutLock};

trait AskColor {
    fn ask_color_for(&mut self, value: &str, new_prompt: &mut String);
}

impl<'a> AskColor for StdoutLock<'a> {
    fn ask_color_for(&mut self, value: &str, new_prompt: &mut String) {
        let _ = self.write(b"Choose a color for the variable:\n");
        for &color in COLORS.keys {
            let _ = write!(self, "{}, ", color);
        }
        let _ = writeln!(self, "or default");
        let _ = self.flush();
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
        new_prompt.push_str(&["${c::", input.trim(), "}"].concat());
        new_prompt.push_str(value);
    }
}

pub(crate) fn prompt(args: &[String], shell: &mut Shell) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let options_list = format!("Select the variables on how you want to customize your prompt:\n\
                                a) USER\n\
                                b) PWD\n\
                                c) SWD\n\
                                For example: \'$a:$b>\' becomes \'{}:{}>\' as the prompt\n", shell.get_var("USER").unwrap(), shell.get_var("PWD").unwrap());

    if args.len() == 1 {
        let _ = writeln!(stdout, "This part is unimplemented, it will list the different set of prompts to choose from.\n\
                                  For now use `prompt config`");
    } else if args[1] == "config" {
        let _ = writeln!(&mut stdout, "{}", options_list);
        let _ = stdout.flush();
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
        if input.is_empty() {
            let _ = writeln!(stdout, "Nothing inputted. Going back to Ion shell");
        } else {
            let mut new_prompt = String::new();
            let mut input_iter = input.trim().chars().peekable();
            while let Some(character) = input_iter.next() {
                match character {
                    '$' => {
                        match input_iter.next() {
                            Some('a') => { stdout.ask_color_for("${USER}", &mut new_prompt); }
                            Some('b') => { stdout.ask_color_for("${PWD}", &mut new_prompt); }
                            Some('c') => { stdout.ask_color_for("${SWD}", &mut new_prompt); }
                            _ => (),
                        }
                    }
                    '@' => {
                        new_prompt.push('@');
                        match input_iter.peek() {
                            Some('$') => {
                                let _ = writeln!(stdout, "Invalid use of '@' character, do not use them right before a variables, please try again");
                            }
                            _ => (),
                        }
                    }
                    character => {
                        new_prompt.push(character);
                    }
                }
            }
            if !new_prompt.ends_with(" ") {
                new_prompt.push(' ');
            }
            new_prompt.push_str("${c::default}");
            shell.set_var("PROMPT", &new_prompt);
        }
    }
    Ok(())
}
