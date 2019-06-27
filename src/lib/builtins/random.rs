use crate::types;
use itertools::Itertools;
use rand::{thread_rng, Rng};

const INVALID: &str = "Invalid argument for random";

fn rand_list(args: &[types::Str]) -> Result<(), types::Str> {
    let num_random = args[0].parse::<usize>().map_err::<types::Str, _>(|_| INVALID.into())?;
    let mut output = Vec::with_capacity(num_random);
    while output.len() < num_random {
        for _ in 0..(num_random - output.len()) {
            let rand_num = thread_rng().gen_range(1, args.len());
            output.push(&*args[rand_num]);
        }
        output.dedup();
    }
    println!("{}", output.iter().format(" "));
    Ok(())
}

pub fn random(args: &[types::Str]) -> Result<(), types::Str> {
    match args.len() {
        0 => {
            let rand_num = thread_rng().gen_range(0, 32767);
            println!("{}", rand_num);
        }
        1 => {
            eprintln!("Ion Shell does not currently support changing the seed");
        }
        2 => {
            let start: u64 = args[0].parse().map_err::<types::Str, _>(|_| INVALID.into())?;
            let end: u64 = args[1].parse().map_err::<types::Str, _>(|_| INVALID.into())?;
            if end <= start {
                return Err("END must be greater than START".into());
            }
            let rand_num = thread_rng().gen_range(start, end);
            println!("{}", rand_num);
        }
        3 => {
            let start: u64 = args[0].parse().map_err::<types::Str, _>(|_| INVALID.into())?;
            let step = match args[1].parse::<u64>() {
                Ok(v) => v,
                Err(_) => return rand_list(args),
            };
            match args[2].parse::<u64>() {
                Ok(end) => {
                    if step <= start {
                        return Err("END must be greater than START".into());
                    }
                    let mut end = end / step + 1;
                    if start / step >= end {
                        end += 1;
                    }
                    let rand_num = thread_rng().gen_range(start / step, end);
                    println!("{}", rand_num * step);
                }
                Err(_) => return rand_list(args),
            };
        }
        _ => return rand_list(args),
    }

    Ok(())
}
