extern crate rand;
use self::rand::Rng;
use std::io::{self, Write,Error};

pub(crate) fn random() -> Result<(), Error> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let rand_num = rand::thread_rng().next_u64();
    writeln!(stdout, "{}", rand_num)?;
    Ok(())
}
