use std::collections::VecDeque;

const MAX_HISTORY_SIZE: usize = 1000;

pub struct History {
    commands: VecDeque<String>,
}

impl History {
    pub fn new() -> Self {
        History {
            commands: VecDeque::with_capacity(MAX_HISTORY_SIZE),
        }
    }

    pub fn add(&mut self, command: String) {
        if self.commands.len() >= MAX_HISTORY_SIZE {
            self.commands.pop_front();
        }
        self.commands.push_back(command);
    }

    pub fn get_all(&self) -> Vec<String> {
        self.commands.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn get_last(&self, n: usize) -> Vec<String> {
        self.commands
            .iter()
            .rev()
            .take(n)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}
