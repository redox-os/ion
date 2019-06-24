use rand::{thread_rng, Rng};
use small;
use std::io::{self, Write};

const INVALID: &str = "Invalid argument for random";

#[allow(unused_must_use)]
fn rand_list(args: &[small::String]) -> Result<(), small::String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut output = Vec::new();
    let arg1 = args[0].parse::<usize>().map_err::<small::String, _>(|_| INVALID.into())?;
    while output.len() < arg1 {
        let rand_num = thread_rng().gen_range(1, args.len());
        output.push(&*args[rand_num]);
        output.dedup();
    }
    for out in output {
        write!(stdout, "{} ", out);
    }
    writeln!(stdout);
    Ok(())
}

#[allow(unused_must_use)]
pub fn random(args: &[small::String]) -> Result<(), small::String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    match args.len() {
        0 => {
            let rand_num = thread_rng().gen_range(0, 32767);
            writeln!(stdout, "{}", rand_num);
        }
        1 => {
            writeln!(stdout, "Ion Shell does not currently support changing the seed");
        }
        2 => {
            let arg1: u64 = args[0].parse().map_err::<small::String, _>(|_| INVALID.into())?;
            let arg2: u64 = args[1].parse().map_err::<small::String, _>(|_| INVALID.into())?;
            if arg2 <= arg1 {
                return Err("END must be greater than START".into());
            }
            let rand_num = thread_rng().gen_range(arg1, arg2);
            writeln!(stdout, "{}", rand_num);
        }
        3 => {
            let arg1: u64 = args[0].parse().map_err::<small::String, _>(|_| INVALID.into())?;
            let arg2 = match args[1].parse::<u64>() {
                Ok(v) => v,
                Err(_) => return rand_list(args),
            };
            match args[2].parse::<u64>() {
                Ok(v) => {
                    if arg2 <= arg1 {
                        return Err("END must be greater than START".into());
                    }
                    let mut end = v / arg2 + 1;
                    if arg1 / arg2 >= end {
                        end += 1;
                    }
                    let rand_num = thread_rng().gen_range(arg1 / arg2, end);
                    writeln!(stdout, "{}", rand_num * arg2);
                }
                Err(_) => return rand_list(args),
            };
        }
        _ => return rand_list(args),
    }

    Ok(())
}
