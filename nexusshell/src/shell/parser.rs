use super::Command;
use std::collections::HashMap;

pub struct Parser;

impl Parser {
    pub fn new() -> Self {
        Parser
    }

    pub fn parse(&self, input: &str) -> anyhow::Result<Command> {
        let input = input.trim();
        if input.is_empty() {
            return Err(anyhow::anyhow!("Empty command"));
        }

        let mut parts = input.split_whitespace();
        let name = parts.next().unwrap_or("").to_string();
        
        let mut args = Vec::new();
        let mut flags = HashMap::new();
        let mut current_arg = None;

        for part in parts {
            if part.starts_with("--") {
                if let Some(flag_name) = current_arg {
                    flags.insert(flag_name, None);
                }
                current_arg = Some(part[2..].to_string());
            } else if part.starts_with('-') {
                if let Some(flag_name) = current_arg {
                    flags.insert(flag_name, None);
                }
                current_arg = Some(part[1..].to_string());
            } else if let Some(flag_name) = current_arg.take() {
                flags.insert(flag_name, Some(part.to_string()));
            } else {
                args.push(part.to_string());
            }
        }

        if let Some(flag_name) = current_arg {
            flags.insert(flag_name, None);
        }

        Ok(Command::new(name, args, flags, input.to_string()))
    }
}
