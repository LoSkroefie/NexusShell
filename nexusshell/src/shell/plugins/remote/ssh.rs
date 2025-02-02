use async_trait::async_trait;
use super::super::super::{Command, Environment, Plugin};
use ssh2::{Session, Channel};
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use tokio::fs;
use std::fs::File;
use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use dirs::home_dir;

#[derive(Debug, Serialize, Deserialize)]
struct SSHConfig {
    known_hosts: PathBuf,
    private_keys: Vec<PathBuf>,
    default_key: Option<PathBuf>,
}

impl Default for SSHConfig {
    fn default() -> Self {
        let mut ssh_dir = home_dir().unwrap_or_default();
        ssh_dir.push(".ssh");
        
        SSHConfig {
            known_hosts: ssh_dir.join("known_hosts"),
            private_keys: vec![ssh_dir.join("id_rsa")],
            default_key: Some(ssh_dir.join("id_rsa")),
        }
    }
}

pub struct SSHPlugin {
    config: SSHConfig,
    sessions: std::collections::HashMap<String, Session>,
}

impl SSHPlugin {
    pub fn new() -> Self {
        let config = Self::load_config().unwrap_or_default();
        SSHPlugin {
            config,
            sessions: std::collections::HashMap::new(),
        }
    }

    fn load_config() -> Result<SSHConfig> {
        let mut config_path = home_dir().unwrap_or_default();
        config_path.push(".nexusshell");
        config_path.push("ssh_config.json");

        if !config_path.exists() {
            let config = SSHConfig::default();
            fs::create_dir_all(config_path.parent().unwrap())?;
            let file = File::create(&config_path)?;
            serde_json::to_writer_pretty(file, &config)?;
            Ok(config)
        } else {
            let file = File::open(&config_path)?;
            Ok(serde_json::from_reader(file)?)
        }
    }

    async fn connect(&mut self, host: &str, username: &str, port: u16) -> Result<()> {
        let tcp = TcpStream::connect(format!("{}:{}", host, port))
            .with_context(|| format!("Failed to connect to {}:{}", host, port))?;

        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;

        // Try private key authentication first
        for key_path in &self.config.private_keys {
            if key_path.exists() {
                match session.userauth_pubkey_file(username, None, key_path, None) {
                    Ok(_) => {
                        self.sessions.insert(host.to_string(), session);
                        return Ok(());
                    }
                    Err(_) => continue,
                }
            }
        }

        // Fallback to password authentication
        Err(anyhow::anyhow!("Authentication failed. Please check your SSH keys or use password authentication"))
    }

    async fn execute_remote(&self, host: &str, command: &str) -> Result<String> {
        let session = self.sessions.get(host)
            .ok_or_else(|| anyhow::anyhow!("Not connected to {}", host))?;

        let mut channel = session.channel_session()?;
        channel.exec(command)?;

        let mut output = String::new();
        channel.read_to_string(&mut output)?;
        channel.wait_close()?;

        Ok(output)
    }

    async fn copy_file(&self, host: &str, src: &Path, dest: &Path, to_remote: bool) -> Result<()> {
        let session = self.sessions.get(host)
            .ok_or_else(|| anyhow::anyhow!("Not connected to {}", host))?;

        if to_remote {
            let mut remote_file = session.scp_send(dest, 0o644, src.metadata()?.len(), None)?;
            let mut local_file = File::open(src)?;
            std::io::copy(&mut local_file, &mut remote_file)?;
        } else {
            let (mut remote_file, _) = session.scp_recv(src)?;
            let mut local_file = File::create(dest)?;
            std::io::copy(&mut remote_file, &mut local_file)?;
        }

        Ok(())
    }

    fn parse_host_string(host_str: &str) -> Result<(String, String, u16)> {
        let parts: Vec<&str> = host_str.split('@').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid host string. Format: username@hostname[:port]"));
        }

        let username = parts[0].to_string();
        let host_parts: Vec<&str> = parts[1].split(':').collect();
        let hostname = host_parts[0].to_string();
        let port = host_parts.get(1).map_or(22, |p| p.parse().unwrap_or(22));

        Ok((username, hostname, port))
    }
}

#[async_trait]
impl Plugin for SSHPlugin {
    fn name(&self) -> &str {
        "ssh"
    }

    fn description(&self) -> &str {
        "SSH client integration for remote operations"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("connect") => {
                if command.args.len() < 2 {
                    return Err(anyhow::anyhow!("Usage: ssh connect username@hostname[:port]"));
                }
                let (username, hostname, port) = Self::parse_host_string(&command.args[1])?;
                self.connect(&hostname, &username, port).await?;
                Ok(format!("Connected to {}@{}", username, hostname))
            }

            Some("exec") => {
                if command.args.len() < 3 {
                    return Err(anyhow::anyhow!("Usage: ssh exec hostname command"));
                }
                let host = &command.args[1];
                let remote_command = command.args[2..].join(" ");
                self.execute_remote(host, &remote_command).await
            }

            Some("copy") => {
                if command.args.len() != 4 {
                    return Err(anyhow::anyhow!("Usage: ssh copy hostname src_path dest_path direction(to/from)"));
                }
                let host = &command.args[1];
                let src = Path::new(&command.args[2]);
                let dest = Path::new(&command.args[3]);
                let direction = &command.args[4];
                
                match direction.as_str() {
                    "to" => self.copy_file(host, src, dest, true).await?,
                    "from" => self.copy_file(host, src, dest, false).await?,
                    _ => return Err(anyhow::anyhow!("Direction must be 'to' or 'from'")),
                }
                Ok("File transfer completed successfully".to_string())
            }

            Some("list-keys") => {
                let mut output = String::from("Configured SSH keys:\n");
                for key in &self.config.private_keys {
                    output.push_str(&format!("- {}\n", key.display()));
                }
                if let Some(default) = &self.config.default_key {
                    output.push_str(&format!("\nDefault key: {}", default.display()));
                }
                Ok(output)
            }

            Some("add-key") => {
                if command.args.len() != 2 {
                    return Err(anyhow::anyhow!("Usage: ssh add-key path/to/key"));
                }
                let key_path = PathBuf::from(&command.args[1]);
                if !key_path.exists() {
                    return Err(anyhow::anyhow!("Key file does not exist"));
                }
                self.config.private_keys.push(key_path);
                Ok("SSH key added successfully".to_string())
            }

            _ => Ok("Available commands: connect, exec, copy, list-keys, add-key".to_string()),
        }
    }
}
