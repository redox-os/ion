use shell::{colors::COLORS, Shell};
use std::io::{self, Write, StdoutLock};

trait AskColorFor {
    fn ask_color_for(&mut self, value: &str, new_prompt: &mut String);
}

impl<'a> AskColorFor for StdoutLock<'a> {
    fn ask_color_for(&mut self, value: &str, new_prompt: &mut String) {
        let _ = self.write(b"Choose a color for the variable:\n");
        for &color in COLORS.keys {
            let _ = write!(self, "{}, ", color);
        }
        let _ = writeln!(self, "or default");
        let _ = self.flush();
        let mut color_input = String::new();
        let _ = io::stdin().read_line(&mut color_input);
        new_prompt.push_str(&["${c::", color_input.trim(), "}"].concat());
        new_prompt.push_str(value);
        new_prompt.push_str("${c::default}");
    }
}

pub(crate) fn prompt(args: &[String], shell: &mut Shell) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let options_list = format!(
r#"Write any variables in the order you want to customize your prompt:
$a for user name
$b for host name
$c1 for working directory
$c2 for simplified working directory
$d for CPU usage, as in TotalCPU%
$e for Memory usage, as in: Used%/Total%
For example:
    '($a):$d>' becomes '({0}):{1}>' for the prompt
    '\[$a\]:$d>' becomes '[{0}]:{1}>' for the prompt"#,
shell.get_var("USER").unwrap(), shell.get_var("SWD").unwrap());

    if args.len() == 1 {
        let _ = writeln!(stdout, "This part is unimplemented, it will list the different set of prompts to choose from.\n\
                                  For now use `prompt config`");
    } else if args[1] == "config" {
        let _ = writeln!(&mut stdout, "{}", options_list);
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
        let input = input.trim();
        if input.is_empty() {
            return Err(String::from("Nothing inputted. Going back to Ion shell"));
        } else {
            let mut new_prompt = String::new();
            let mut input_iter = input.trim().chars();
            while let Some(character) = input_iter.next() {
                match character {
                    '$' => {
                        match input_iter.next() {
                            Some('a') => { stdout.ask_color_for("${USER}", &mut new_prompt); }
                            Some('b') => { stdout.ask_color_for("${HOST}", &mut new_prompt); }
                            Some('c') => { 
                                match input_iter.next() {
                                    Some('1') => stdout.ask_color_for("${PWD}", &mut new_prompt),
                                    Some('2') => stdout.ask_color_for("${SWD}", &mut new_prompt),
                                    _ => continue,
                                }
                            }
                            Some('d') => { 
                                stdout.ask_color_for(r"$(let cpu_usage = [@(top -bn1 | grep Cpu)]; echo @cpu_usage[1]%)", &mut new_prompt);
                            }
                            Some('e') => { 
                                stdout.ask_color_for(r"$(let free=[@(free -h)]; echo @free[8]/@free[7])", &mut new_prompt);
                            }
                            _ => continue,
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
    let _ = stdout.flush();
    Ok(())
}
