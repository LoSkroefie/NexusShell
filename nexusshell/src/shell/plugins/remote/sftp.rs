use async_trait::async_trait;
use super::super::super::{Command, Environment, Plugin};
use ssh2::{Session, Sftp};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use anyhow::{Result, Context};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use indicatif::{ProgressBar, ProgressStyle};

pub struct SFTPPlugin {
    sessions: std::collections::HashMap<String, (Session, Sftp)>,
}

impl SFTPPlugin {
    pub fn new() -> Self {
        SFTPPlugin {
            sessions: std::collections::HashMap::new(),
        }
    }

    async fn connect(&mut self, host: &str, username: &str, port: u16) -> Result<()> {
        let tcp = TcpStream::connect(format!("{}:{}", host, port))
            .with_context(|| format!("Failed to connect to {}:{}", host, port))?;

        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;

        // Try to authenticate with default key
        let mut ssh_dir = dirs::home_dir().unwrap_or_default();
        ssh_dir.push(".ssh");
        let key_path = ssh_dir.join("id_rsa");

        if key_path.exists() {
            session.userauth_pubkey_file(username, None, &key_path, None)?;
        } else {
            return Err(anyhow::anyhow!("No SSH key found and password auth not implemented"));
        }

        let sftp = session.sftp()?;
        self.sessions.insert(host.to_string(), (session, sftp));
        Ok(())
    }

    async fn upload_file(&self, host: &str, local_path: &Path, remote_path: &Path) -> Result<()> {
        let (_, sftp) = self.sessions.get(host)
            .ok_or_else(|| anyhow::anyhow!("Not connected to {}", host))?;

        let file_size = fs::metadata(local_path)?.len();
        let pb = ProgressBar::new(file_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));

        let mut local_file = File::open(local_path)?;
        let mut remote_file = sftp.create(remote_path)?;

        let mut buffer = [0; 8192];
        let mut uploaded = 0;
        loop {
            let n = local_file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            remote_file.write_all(&buffer[..n])?;
            uploaded += n;
            pb.set_position(uploaded as u64);
        }

        pb.finish_with_message("Upload complete");
        Ok(())
    }

    async fn download_file(&self, host: &str, remote_path: &Path, local_path: &Path) -> Result<()> {
        let (_, sftp) = self.sessions.get(host)
            .ok_or_else(|| anyhow::anyhow!("Not connected to {}", host))?;

        let file_size = sftp.stat(remote_path)?.size.unwrap_or(0);
        let pb = ProgressBar::new(file_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));

        let mut remote_file = sftp.open(remote_path)?;
        let mut local_file = File::create(local_path)?;

        let mut buffer = [0; 8192];
        let mut downloaded = 0;
        loop {
            let n = remote_file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            local_file.write_all(&buffer[..n])?;
            downloaded += n;
            pb.set_position(downloaded as u64);
        }

        pb.finish_with_message("Download complete");
        Ok(())
    }

    async fn list_directory(&self, host: &str, remote_path: &Path) -> Result<String> {
        let (_, sftp) = self.sessions.get(host)
            .ok_or_else(|| anyhow::anyhow!("Not connected to {}", host))?;

        let mut output = String::new();
        output.push_str(&format!("Contents of {}:\n", remote_path.display()));

        for entry in sftp.readdir(remote_path)? {
            let filename = entry.0;
            let attrs = entry.1;
            
            let file_type = if attrs.is_dir() { "DIR" } else { "FILE" };
            let size = attrs.size.unwrap_or(0);
            let permissions = attrs.permissions.unwrap_or(0);
            
            output.push_str(&format!("{:<4} {:>10} {:o} {}\n",
                file_type,
                size,
                permissions,
                filename.display()
            ));
        }

        Ok(output)
    }

    async fn create_directory(&self, host: &str, remote_path: &Path) -> Result<()> {
        let (_, sftp) = self.sessions.get(host)
            .ok_or_else(|| anyhow::anyhow!("Not connected to {}", host))?;

        sftp.mkdir(remote_path, 0o755)?;
        Ok(())
    }

    async fn remove_file(&self, host: &str, remote_path: &Path) -> Result<()> {
        let (_, sftp) = self.sessions.get(host)
            .ok_or_else(|| anyhow::anyhow!("Not connected to {}", host))?;

        sftp.unlink(remote_path)?;
        Ok(())
    }

    async fn remove_directory(&self, host: &str, remote_path: &Path) -> Result<()> {
        let (_, sftp) = self.sessions.get(host)
            .ok_or_else(|| anyhow::anyhow!("Not connected to {}", host))?;

        sftp.rmdir(remote_path)?;
        Ok(())
    }
}

#[async_trait]
impl Plugin for SFTPPlugin {
    fn name(&self) -> &str {
        "sftp"
    }

    fn description(&self) -> &str {
        "SFTP file transfer operations"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("connect") => {
                if command.args.len() < 2 {
                    return Err(anyhow::anyhow!("Usage: sftp connect username@hostname[:port]"));
                }
                let parts: Vec<&str> = command.args[1].split('@').collect();
                if parts.len() != 2 {
                    return Err(anyhow::anyhow!("Invalid host string. Format: username@hostname[:port]"));
                }

                let username = parts[0];
                let host_parts: Vec<&str> = parts[1].split(':').collect();
                let hostname = host_parts[0];
                let port = host_parts.get(1).map_or(22, |p| p.parse().unwrap_or(22));

                self.connect(hostname, username, port).await?;
                Ok(format!("Connected to {}@{}", username, hostname))
            }

            Some("upload") => {
                if command.args.len() != 4 {
                    return Err(anyhow::anyhow!("Usage: sftp upload hostname local_path remote_path"));
                }
                let host = &command.args[1];
                let local_path = PathBuf::from(&command.args[2]);
                let remote_path = PathBuf::from(&command.args[3]);

                self.upload_file(host, &local_path, &remote_path).await?;
                Ok("Upload completed successfully".to_string())
            }

            Some("download") => {
                if command.args.len() != 4 {
                    return Err(anyhow::anyhow!("Usage: sftp download hostname remote_path local_path"));
                }
                let host = &command.args[1];
                let remote_path = PathBuf::from(&command.args[2]);
                let local_path = PathBuf::from(&command.args[3]);

                self.download_file(host, &remote_path, &local_path).await?;
                Ok("Download completed successfully".to_string())
            }

            Some("ls") => {
                if command.args.len() != 3 {
                    return Err(anyhow::anyhow!("Usage: sftp ls hostname remote_path"));
                }
                let host = &command.args[1];
                let remote_path = PathBuf::from(&command.args[2]);

                self.list_directory(host, &remote_path).await
            }

            Some("mkdir") => {
                if command.args.len() != 3 {
                    return Err(anyhow::anyhow!("Usage: sftp mkdir hostname remote_path"));
                }
                let host = &command.args[1];
                let remote_path = PathBuf::from(&command.args[2]);

                self.create_directory(host, &remote_path).await?;
                Ok("Directory created successfully".to_string())
            }

            Some("rm") => {
                if command.args.len() != 3 {
                    return Err(anyhow::anyhow!("Usage: sftp rm hostname remote_path"));
                }
                let host = &command.args[1];
                let remote_path = PathBuf::from(&command.args[2]);

                self.remove_file(host, &remote_path).await?;
                Ok("File removed successfully".to_string())
            }

            Some("rmdir") => {
                if command.args.len() != 3 {
                    return Err(anyhow::anyhow!("Usage: sftp rmdir hostname remote_path"));
                }
                let host = &command.args[1];
                let remote_path = PathBuf::from(&command.args[2]);

                self.remove_directory(host, &remote_path).await?;
                Ok("Directory removed successfully".to_string())
            }

            _ => Ok("Available commands: connect, upload, download, ls, mkdir, rm, rmdir".to_string()),
        }
    }
}
