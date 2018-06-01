use shell::Shell;
use std::io::{self, Write, StdoutLock};

trait AskColor {
    fn ask_color_for(&mut self, value: &str, new_prompt: &mut String);
}

impl<'a> AskColor for StdoutLock<'a> {
    fn ask_color_for(&mut self, value: &str, new_prompt: &mut String) {
        let _ = self.write(b"What color you would like to use for this variable?:\n");
        let _ = self.flush();
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
        new_prompt.push_str(&input);
        new_prompt.push_str(value);
    }
}

pub(crate) fn prompt(args: &[String], shell: &mut Shell) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let options_list = b"Select the variables on how you want to customize your prompt:\n\
                         a) USER\n\
                         b) PWD\n\
                         c) SWD\n\
                         For example: \'$a@$b\' becomes \'user@/path/to/pwd\' as the prompt";

    if args.is_empty() {
        let _ = writeln!(stdout, "This part is unimplemented, it will list the different set of prompts to choose from.\n\
                                  For now use `prompt config`");
    } else if args[0] == "config" {
        let _ = stdout.write(options_list);
        let _ = stdout.flush();
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
        if input.is_empty() {
            let _ = writeln!(stdout, "Nothing inputted. Going back to Ion shell");
        } else {
            let mut new_prompt = String::new();
            let mut input_iter = input.trim().chars();
            while let Some(character) = input_iter.next() {
                match character {
                    '$' => {
                        match input_iter.next() {
                            Some('a') => { stdout.ask_color_for("${USER}", &mut new_prompt); }
                            Some('b') => { stdout.ask_color_for("${PWD}", &mut new_prompt); }
                            Some('c') => { stdout.ask_color_for("${SWD}", &mut new_prompt); }
                            _ => continue,
                        }
                    }
                    character => {
                        new_prompt.push(character);
                    }
                }
            }
            shell.set_var("PROMPT", &new_prompt);
        }
    }
    Ok(())
}
