use std::collections::VecDeque;

pub struct History {
    history: VecDeque<String>,
    max_size: usize,
}

impl History {
    pub fn new() -> History {
        History {
            history: VecDeque::new(),
            max_size: 1000, // TODO don't hardcode this size, make it configurable
        }
    }

    pub fn add(&mut self, command: String) {
        self.history.truncate(self.max_size - 1); // Make room for new item
        self.history.push_front(command);
    }

    pub fn history<I: IntoIterator>(&self, _: I) where I::Item: AsRef<str> {
        for command in self.history.iter().rev() {
            println!("{}", command);
        }
    }
}
