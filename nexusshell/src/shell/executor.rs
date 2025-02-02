use super::{Command, Environment, PluginManager};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command as TokioCommand;

pub struct Executor {
    plugin_manager: Arc<PluginManager>,
}

impl Executor {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Executor { plugin_manager }
    }

    pub async fn execute(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        if command.is_builtin() {
            self.execute_builtin(command, env).await
        } else if let Some(plugin) = self.plugin_manager.get_plugin(&command.name) {
            plugin.execute(command, env).await
        } else {
            self.execute_system_command(command).await
        }
    }

    async fn execute_builtin(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        match command.name.as_str() {
            "cd" => {
                let path = if command.args.is_empty() {
                    env.get_var("HOME")
                        .ok_or_else(|| anyhow::anyhow!("HOME environment variable not set"))?
                        .clone()
                } else {
                    command.args[0].clone()
                };
                
                let path = env.expand_path(&path);
                std::env::set_current_dir(&path)?;
                Ok("".to_string())
            }
            "pwd" => Ok(env.get_current_dir().to_string_lossy().to_string()),
            "echo" => Ok(command.args.join(" ")),
            "clear" => {
                print!("\x1B[2J\x1B[1;1H");
                Ok("".to_string())
            }
            _ => Err(anyhow::anyhow!("Unknown builtin command")),
        }
    }

    async fn execute_system_command(&self, command: &Command) -> anyhow::Result<String> {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = TokioCommand::new("cmd");
            cmd.args(&["/C", &command.name]);
            cmd
        } else {
            TokioCommand::new(&command.name)
        };

        cmd.args(&command.args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
