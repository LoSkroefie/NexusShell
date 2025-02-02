mod fileops;
mod process;
mod git;
mod network;

pub use fileops::FileOperationsPlugin;
pub use process::ProcessPlugin;
pub use git::GitPlugin;
pub use network::NetworkPlugin;

use async_trait::async_trait;
use super::{Command, Environment};
use std::collections::HashMap;
use std::sync::RwLock;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, command: &Command, env: &Environment) -> anyhow::Result<String>;
}

pub struct PluginManager {
    plugins: RwLock<HashMap<String, Box<dyn Plugin + Send + Sync>>>,
}

impl PluginManager {
    pub fn new() -> Self {
        let mut manager = PluginManager {
            plugins: RwLock::new(HashMap::new()),
        };

        // Register built-in plugins
        let _ = manager.register_plugin(Box::new(FileOperationsPlugin::new()));
        let _ = manager.register_plugin(Box::new(ProcessPlugin::new()));
        let _ = manager.register_plugin(Box::new(GitPlugin::new()));
        let _ = manager.register_plugin(Box::new(NetworkPlugin::new()));

        manager
    }

    pub fn register_plugin(&self, plugin: Box<dyn Plugin + Send + Sync>) -> anyhow::Result<()> {
        let name = plugin.name().to_string();
        let mut plugins = self.plugins.write().map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        plugins.insert(name, plugin);
        Ok(())
    }

    pub fn get_plugin(&self, name: &str) -> Option<Box<dyn Plugin + Send + Sync>> {
        self.plugins
            .read()
            .ok()?
            .get(name)
            .map(|p| Box::new(p.as_ref()) as Box<dyn Plugin + Send + Sync>)
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
