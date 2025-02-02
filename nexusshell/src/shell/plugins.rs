use super::{Command, Environment};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, command: &Command, env: &Environment) -> anyhow::Result<String>;
}

pub struct PluginManager {
    plugins: RwLock<HashMap<String, Box<dyn Plugin>>>,
}

impl PluginManager {
    pub fn new() -> Self {
        PluginManager {
            plugins: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_plugin(&self, plugin: Box<dyn Plugin>) -> anyhow::Result<()> {
        let name = plugin.name().to_string();
        let mut plugins = self.plugins.write().map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        plugins.insert(name, plugin);
        Ok(())
    }

    pub fn get_plugin(&self, name: &str) -> Option<Box<dyn Plugin>> {
        self.plugins
            .read()
            .ok()?
            .get(name)
            .map(|p| Box::new(p.as_ref()) as Box<dyn Plugin>)
    }

    pub fn list_plugins(&self) -> Vec<(String, String)> {
        self.plugins
            .read()
            .map(|plugins| {
                plugins
                    .iter()
                    .map(|(name, plugin)| (name.clone(), plugin.description().to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

// Example built-in plugin
pub struct GitPlugin;

#[async_trait]
impl Plugin for GitPlugin {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Git version control system integration"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> anyhow::Result<String> {
        let mut cmd = tokio::process::Command::new("git");
        cmd.args(&command.args);
        let output = cmd.output().await?;
        
        let mut result = String::new();
        if !output.stdout.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        
        Ok(result)
    }
}
