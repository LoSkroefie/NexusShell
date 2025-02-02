use async_trait::async_trait;
use super::super::{Command, Environment, Plugin};
use tokio::process::Command as TokioCommand;

pub struct GitPlugin;

impl GitPlugin {
    pub fn new() -> Self {
        GitPlugin
    }

    async fn execute_git_command(&self, args: &[String]) -> anyhow::Result<String> {
        let output = TokioCommand::new("git")
            .args(args)
            .output()
            .await?;

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

#[async_trait]
impl Plugin for GitPlugin {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Git version control system integration"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> anyhow::Result<String> {
        // Special handling for common git commands
        match command.args.first().map(|s| s.as_str()) {
            Some("status") => {
                let output = self.execute_git_command(&["status", "--porcelain", "-b"]).await?;
                if output.is_empty() {
                    Ok("No changes (working directory clean)".to_string())
                } else {
                    Ok(output)
                }
            }
            Some("log") => {
                let mut args = vec!["log", "--pretty=format:%C(yellow)%h%Creset %C(cyan)%ad%Creset %s %C(green)<%an>%Creset", "--date=short"];
                args.extend(command.args.iter().skip(1).map(|s| s.as_str()));
                self.execute_git_command(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).await
            }
            Some("diff") => {
                let mut args = vec!["diff", "--color"];
                args.extend(command.args.iter().skip(1).map(|s| s.as_str()));
                self.execute_git_command(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).await
            }
            Some("branch") => {
                let mut args = vec!["branch", "--color"];
                args.extend(command.args.iter().skip(1).map(|s| s.as_str()));
                self.execute_git_command(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).await
            }
            _ => {
                // For all other git commands, pass through as-is
                self.execute_git_command(&command.args).await
            }
        }
    }
}
