use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

pub struct Environment {
    vars: HashMap<String, String>,
    current_dir: PathBuf,
}

impl Environment {
    pub fn new() -> Self {
        let mut vars = HashMap::new();
        for (key, value) in env::vars() {
            vars.insert(key, value);
        }

        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        Environment {
            vars,
            current_dir,
        }
    }

    pub fn get_var(&self, name: &str) -> Option<&String> {
        self.vars.get(name)
    }

    pub fn set_var(&mut self, name: String, value: String) {
        self.vars.insert(name, value);
    }

    pub fn get_current_dir(&self) -> PathBuf {
        self.current_dir.clone()
    }

    pub fn change_directory(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let new_path = if path.is_absolute() {
            path
        } else {
            self.current_dir.join(path)
        };

        if !new_path.exists() {
            return Err(anyhow::anyhow!("Directory does not exist"));
        }

        if !new_path.is_dir() {
            return Err(anyhow::anyhow!("Path is not a directory"));
        }

        env::set_current_dir(&new_path)?;
        self.current_dir = new_path;
        Ok(())
    }

    pub fn expand_path(&self, path: &str) -> PathBuf {
        let path = if path.starts_with('~') {
            if let Some(home) = self.get_var("HOME") {
                if path.len() == 1 {
                    PathBuf::from(home)
                } else {
                    PathBuf::from(home).join(&path[2..])
                }
            } else {
                PathBuf::from(path)
            }
        } else {
            PathBuf::from(path)
        };

        if path.is_absolute() {
            path
        } else {
            self.current_dir.join(path)
        }
    }
}
