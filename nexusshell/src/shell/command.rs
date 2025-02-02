use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub args: Vec<String>,
    pub flags: HashMap<String, Option<String>>,
    pub raw_input: String,
}

impl Command {
    pub fn new(name: String, args: Vec<String>, flags: HashMap<String, Option<String>>, raw_input: String) -> Self {
        Command {
            name,
            args,
            flags,
            raw_input,
        }
    }

    pub fn is_builtin(&self) -> bool {
        matches!(
            self.name.as_str(),
            "cd" | "exit" | "history" | "help" | "clear" | "pwd" | "echo"
        )
    }

    pub fn is_exit(&self) -> bool {
        self.name == "exit"
    }
}
