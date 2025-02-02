use async_trait::async_trait;
use super::super::{Command, Environment, Plugin};
use tokio::process::Command as TokioCommand;
use std::time::Duration;
use tokio::time::sleep;

pub struct NetworkPlugin;

impl NetworkPlugin {
    pub fn new() -> Self {
        NetworkPlugin
    }
}

#[async_trait]
impl Plugin for NetworkPlugin {
    fn name(&self) -> &str {
        "network"
    }

    fn description(&self) -> &str {
        "Network operations and diagnostics"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> anyhow::Result<String> {
        match command.name.as_str() {
            "ping" => self.ping(command).await,
            "curl" => self.curl(command).await,
            "wget" => self.wget(command).await,
            "netstat" => self.netstat(command).await,
            _ => Err(anyhow::anyhow!("Unknown network command")),
        }
    }
}

impl NetworkPlugin {
    async fn ping(&self, command: &Command) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: ping <host> [-c count]"));
        }

        let mut args = Vec::new();
        if cfg!(windows) {
            args.push("-n");
            args.push("4"); // Default count
            for arg in &command.args {
                if arg == "-c" {
                    args[0] = "-n";
                } else {
                    args.push(arg);
                }
            }
        } else {
            args.push("-c");
            args.push("4"); // Default count
            args.extend(command.args.iter().map(|s| s.as_str()));
        }

        let output = TokioCommand::new("ping")
            .args(&args)
            .output()
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn curl(&self, command: &Command) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: curl <url> [options]"));
        }

        let output = TokioCommand::new("curl")
            .args(&command.args)
            .output()
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn wget(&self, command: &Command) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: wget <url> [options]"));
        }

        let output = TokioCommand::new("wget")
            .args(&command.args)
            .output()
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn netstat(&self, _command: &Command) -> anyhow::Result<String> {
        let output = if cfg!(windows) {
            TokioCommand::new("netstat")
                .args(&["-ano"])
                .output()
                .await?
        } else {
            TokioCommand::new("netstat")
                .args(&["-tulpn"])
                .output()
                .await?
        };

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
