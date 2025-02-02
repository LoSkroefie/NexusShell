use async_trait::async_trait;
use super::super::{Command, Environment, Plugin};
use std::collections::HashMap;
use sysinfo::{System, SystemExt, ProcessExt};
use tokio::process::Command as TokioCommand;
use std::process::Stdio;

pub struct ProcessPlugin {
    sys: System,
}

impl ProcessPlugin {
    pub fn new() -> Self {
        ProcessPlugin {
            sys: System::new_all(),
        }
    }

    fn format_size(size: u64) -> String {
        if size < 1024 {
            format!("{}B", size)
        } else if size < 1024 * 1024 {
            format!("{:.1}K", size as f64 / 1024.0)
        } else if size < 1024 * 1024 * 1024 {
            format!("{:.1}M", size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1}G", size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

#[async_trait]
impl Plugin for ProcessPlugin {
    fn name(&self) -> &str {
        "process"
    }

    fn description(&self) -> &str {
        "Process management and monitoring"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> anyhow::Result<String> {
        match command.name.as_str() {
            "ps" => self.list_processes(command).await,
            "kill" => self.kill_process(command).await,
            "bg" => self.background_process(command).await,
            "fg" => self.foreground_process(command).await,
            "top" => self.show_top_processes(command).await,
            _ => Err(anyhow::anyhow!("Unknown process command")),
        }
    }
}

impl ProcessPlugin {
    async fn list_processes(&self, _command: &Command) -> anyhow::Result<String> {
        self.sys.refresh_all();
        
        let mut processes = Vec::new();
        processes.push(format!("{:<8} {:<8} {:<8} {:<20}", "PID", "CPU%", "MEM", "NAME"));
        processes.push("-".repeat(50));

        for (pid, process) in self.sys.processes() {
            processes.push(format!("{:<8} {:<8.1} {:<8} {:<20}",
                pid,
                process.cpu_usage(),
                Self::format_size(process.memory()),
                process.name()
            ));
        }

        Ok(processes.join("\n"))
    }

    async fn kill_process(&self, command: &Command) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: kill <pid>"));
        }

        let pid = command.args[0].parse::<i32>()?;
        
        if cfg!(windows) {
            TokioCommand::new("taskkill")
                .args(&["/PID", &pid.to_string(), "/F"])
                .output()
                .await?;
        } else {
            TokioCommand::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .output()
                .await?;
        }

        Ok(format!("Process {} killed", pid))
    }

    async fn background_process(&self, command: &Command) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: bg <command>"));
        }

        let mut cmd = TokioCommand::new(&command.args[0]);
        if command.args.len() > 1 {
            cmd.args(&command.args[1..]);
        }

        cmd.stdin(Stdio::null())
           .stdout(Stdio::null())
           .stderr(Stdio::null());

        let child = cmd.spawn()?;
        Ok(format!("Process started in background with PID: {}", child.id().unwrap()))
    }

    async fn foreground_process(&self, command: &Command) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: fg <command>"));
        }

        let mut cmd = TokioCommand::new(&command.args[0]);
        if command.args.len() > 1 {
            cmd.args(&command.args[1..]);
        }

        let output = cmd.output().await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn show_top_processes(&self, _command: &Command) -> anyhow::Result<String> {
        self.sys.refresh_all();
        
        let mut processes: Vec<_> = self.sys.processes()
            .values()
            .collect();
        
        processes.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap());
        
        let mut output = Vec::new();
        output.push(format!("{:<8} {:<8} {:<8} {:<20}", "PID", "CPU%", "MEM", "NAME"));
        output.push("-".repeat(50));

        for process in processes.iter().take(10) {
            output.push(format!("{:<8} {:<8.1} {:<8} {:<20}",
                process.pid(),
                process.cpu_usage(),
                Self::format_size(process.memory()),
                process.name()
            ));
        }

        Ok(output.join("\n"))
    }
}
