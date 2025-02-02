use std::path::{Path, PathBuf};
use std::fs;
use super::Environment;

pub struct Completer {
    environment: Environment,
}

impl Completer {
    pub fn new(environment: Environment) -> Self {
        Completer { environment }
    }

    pub fn complete(&self, line: &str) -> Vec<String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        
        if words.is_empty() {
            return self.get_executables();
        }

        if words.len() == 1 {
            return self.complete_command(words[0]);
        }

        // Path completion for arguments
        if let Some(last_word) = words.last() {
            if last_word.contains('/') || last_word.contains('\\') {
                return self.complete_path(last_word);
            }
        }

        Vec::new()
    }

    fn complete_command(&self, partial: &str) -> Vec<String> {
        let mut completions = Vec::new();

        // Built-in commands
        let builtins = vec![
            "cd", "pwd", "ls", "clear", "exit", "help", "history",
            "cat", "echo", "grep", "find", "ps", "kill", "mkdir",
            "rm", "cp", "mv", "touch", "chmod", "chown", "df",
            "du", "free", "top", "htop", "ping", "curl", "wget",
        ];

        completions.extend(
            builtins
                .into_iter()
                .filter(|cmd| cmd.starts_with(partial))
                .map(String::from),
        );

        // Executables from PATH
        completions.extend(self.get_executables_filtered(partial));

        completions.sort();
        completions.dedup();
        completions
    }

    fn complete_path(&self, partial: &str) -> Vec<String> {
        let path = PathBuf::from(partial);
        let (dir, prefix) = if partial.ends_with('/') || partial.ends_with('\\') {
            (path, String::new())
        } else {
            (path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf(),
             path.file_name()
                 .map(|f| f.to_string_lossy().to_string())
                 .unwrap_or_default())
        };

        let expanded_dir = self.environment.expand_path(&dir.to_string_lossy());

        match fs::read_dir(expanded_dir) {
            Ok(entries) => {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.file_name()
                            .map(|name| name.to_string_lossy().starts_with(&prefix))
                            .unwrap_or(false)
                    })
                    .map(|path| {
                        let mut completion = path.to_string_lossy().to_string();
                        if path.is_dir() {
                            completion.push('/');
                        }
                        completion
                    })
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }

    fn get_executables(&self) -> Vec<String> {
        let mut executables = Vec::new();
        if let Some(path_var) = self.environment.get_var("PATH") {
            for path in path_var.split(if cfg!(windows) { ';' } else { ':' }) {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.filter_map(Result::ok) {
                        if let Some(name) = entry.file_name().to_str() {
                            if cfg!(windows) && name.ends_with(".exe") {
                                executables.push(name[..name.len() - 4].to_string());
                            } else if !cfg!(windows) {
                                executables.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        executables.sort();
        executables.dedup();
        executables
    }

    fn get_executables_filtered(&self, prefix: &str) -> Vec<String> {
        self.get_executables()
            .into_iter()
            .filter(|exe| exe.starts_with(prefix))
            .collect()
    }
}
