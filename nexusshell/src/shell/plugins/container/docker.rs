use async_trait::async_trait;
use super::super::super::{Command, Environment, Plugin};
use bollard::Docker;
use bollard::container::{CreateContainerOptions, Config, ListContainersOptions, StartContainerOptions, StopContainerOptions, RemoveContainerOptions};
use bollard::image::{CreateImageOptions, ListImagesOptions, RemoveImageOptions};
use bollard::service::{ContainerSummary, ImageSummary, ContainerInspectResponse};
use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::network::ListNetworksOptions;
use bollard::volume::ListVolumesOptions;
use futures::StreamExt;
use std::collections::HashMap;
use anyhow::{Result, Context};
use tokio::fs;
use serde::{Serialize, Deserialize};
use indicatif::{ProgressBar, ProgressStyle};
use chrono::{DateTime, Utc};
use std::time::Duration;
use colored::*;

#[derive(Debug, Serialize, Deserialize)]
struct DockerConfig {
    default_registry: String,
    pull_timeout: u64,
    push_timeout: u64,
}

impl Default for DockerConfig {
    fn default() -> Self {
        DockerConfig {
            default_registry: "docker.io".to_string(),
            pull_timeout: 300,
            push_timeout: 300,
        }
    }
}

pub struct DockerPlugin {
    config: DockerConfig,
    client: Docker,
}

impl DockerPlugin {
    pub async fn new() -> Result<Self> {
        let config = Self::load_config().await.unwrap_or_default();
        let client = Docker::connect_with_local_defaults()?;
        
        Ok(DockerPlugin {
            config,
            client,
        })
    }

    async fn load_config() -> Result<DockerConfig> {
        let mut config_path = dirs::home_dir().unwrap_or_default();
        config_path.push(".nexusshell");
        config_path.push("docker_config.json");

        if !config_path.exists() {
            let config = DockerConfig::default();
            fs::create_dir_all(config_path.parent().unwrap()).await?;
            fs::write(&config_path, serde_json::to_string_pretty(&config)?).await?;
            Ok(config)
        } else {
            let content = fs::read_to_string(&config_path).await?;
            Ok(serde_json::from_str(&content)?)
        }
    }

    async fn list_containers(&self, all: bool) -> Result<String> {
        let options = ListContainersOptions {
            all,
            ..Default::default()
        };

        let containers = self.client.list_containers(Some(options)).await?;
        let mut output = String::new();
        output.push_str(&format!("{}\n", "CONTAINERS".bright_green()));
        output.push_str(&format!("{:<20} {:<15} {:<20} {:<15} {:<30}\n",
            "CONTAINER ID", "STATUS", "PORTS", "NAME", "IMAGE"));

        for container in containers {
            let id = container.id.unwrap_or_default();
            let status = container.status.unwrap_or_default();
            let name = container.names.unwrap_or_default().join(", ");
            let image = container.image.unwrap_or_default();
            let ports = container.ports.unwrap_or_default()
                .iter()
                .map(|p| format!("{}:{}", p.private_port.unwrap_or(0), p.public_port.unwrap_or(0)))
                .collect::<Vec<_>>()
                .join(", ");

            output.push_str(&format!("{:<20} {:<15} {:<20} {:<15} {:<30}\n",
                &id[..12], status, ports, name, image));
        }

        Ok(output)
    }

    async fn pull_image(&self, image: &str) -> Result<String> {
        let options = CreateImageOptions {
            from_image: image,
            ..Default::default()
        };

        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"));
        pb.set_message(format!("Pulling image {}", image));

        let mut stream = self.client.create_image(Some(options), None, None);
        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = info.status {
                        pb.set_message(status);
                    }
                }
                Err(e) => {
                    pb.finish_with_message("Pull failed");
                    return Err(anyhow::anyhow!("Failed to pull image: {}", e));
                }
            }
        }

        pb.finish_with_message(format!("Successfully pulled {}", image));
        Ok(format!("Successfully pulled image {}", image))
    }

    async fn create_container(&self, name: &str, image: &str, command: Option<Vec<String>>, ports: Option<Vec<String>>, volumes: Option<Vec<String>>) -> Result<String> {
        let mut port_bindings = HashMap::new();
        if let Some(port_mappings) = ports {
            for port in port_mappings {
                let parts: Vec<&str> = port.split(':').collect();
                if parts.len() == 2 {
                    port_bindings.insert(
                        format!("{}/tcp", parts[1]),
                        Some(vec![bollard::models::PortBinding {
                            host_ip: Some("0.0.0.0".to_string()),
                            host_port: Some(parts[0].to_string()),
                        }]),
                    );
                }
            }
        }

        let mut volume_bindings = Vec::new();
        if let Some(volume_mappings) = volumes {
            for volume in volume_mappings {
                let parts: Vec<&str> = volume.split(':').collect();
                if parts.len() == 2 {
                    volume_bindings.push(format!("{}:{}", parts[0], parts[1]));
                }
            }
        }

        let options = CreateContainerOptions {
            name,
        };

        let config = Config {
            image: Some(image.to_string()),
            cmd: command,
            exposed_ports: Some(port_bindings.keys().map(|k| (k.clone(), HashMap::new())).collect()),
            host_config: Some(bollard::models::HostConfig {
                port_bindings: Some(port_bindings),
                binds: Some(volume_bindings),
                ..Default::default()
            }),
            ..Default::default()
        };

        let container = self.client.create_container(Some(options), config).await?;
        Ok(format!("Created container {} with ID {}", name, container.id))
    }

    async fn start_container(&self, container_id: &str) -> Result<String> {
        self.client.start_container(container_id, None::<StartContainerOptions<String>>).await?;
        Ok(format!("Started container {}", container_id))
    }

    async fn stop_container(&self, container_id: &str) -> Result<String> {
        self.client.stop_container(container_id, None::<StopContainerOptions>).await?;
        Ok(format!("Stopped container {}", container_id))
    }

    async fn remove_container(&self, container_id: &str, force: bool) -> Result<String> {
        let options = RemoveContainerOptions {
            force,
            ..Default::default()
        };

        self.client.remove_container(container_id, Some(options)).await?;
        Ok(format!("Removed container {}", container_id))
    }

    async fn container_logs(&self, container_id: &str) -> Result<String> {
        let options = bollard::container::LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: Some("100"),
            ..Default::default()
        };

        let mut logs = String::new();
        let mut stream = self.client.logs(container_id, Some(options));
        while let Some(result) = stream.next().await {
            match result {
                Ok(log) => {
                    logs.push_str(&format!("{}\n", log.to_string()));
                }
                Err(e) => return Err(anyhow::anyhow!("Failed to get logs: {}", e)),
            }
        }

        Ok(logs)
    }

    async fn container_stats(&self, container_id: &str) -> Result<String> {
        let stats = self.client.inspect_container(container_id, None).await?;
        let mut output = String::new();
        
        output.push_str(&format!("Container Stats for {}\n", container_id));
        if let Some(state) = stats.state {
            output.push_str(&format!("Status: {}\n", state.status.unwrap_or_default()));
            output.push_str(&format!("Running: {}\n", state.running.unwrap_or_default()));
            output.push_str(&format!("Pid: {}\n", state.pid.unwrap_or_default()));
            if let Some(started) = state.started_at {
                output.push_str(&format!("Started At: {}\n", started));
            }
        }

        if let Some(host_config) = stats.host_config {
            output.push_str(&format!("Memory Limit: {} MB\n", 
                host_config.memory.unwrap_or(0) / 1024 / 1024));
            output.push_str(&format!("CPU Shares: {}\n", 
                host_config.cpu_shares.unwrap_or(0)));
        }

        Ok(output)
    }

    async fn list_images(&self) -> Result<String> {
        let options = ListImagesOptions::<String> {
            all: true,
            ..Default::default()
        };

        let images = self.client.list_images(Some(options)).await?;
        let mut output = String::new();
        output.push_str(&format!("{}\n", "IMAGES".bright_green()));
        output.push_str(&format!("{:<20} {:<20} {:<20} {:<20}\n",
            "IMAGE ID", "REPOSITORY", "TAG", "SIZE"));

        for image in images {
            let id = image.id.unwrap_or_default();
            let repo_tags = image.repo_tags.unwrap_or_default();
            let size = image.size.unwrap_or(0) / 1024 / 1024; // Convert to MB

            for tag in repo_tags {
                let parts: Vec<&str> = tag.split(':').collect();
                let (repo, tag) = if parts.len() == 2 {
                    (parts[0], parts[1])
                } else {
                    (&tag[..], "latest")
                };

                output.push_str(&format!("{:<20} {:<20} {:<20} {:<20}MB\n",
                    &id[7..19], repo, tag, size));
            }
        }

        Ok(output)
    }

    async fn remove_image(&self, image: &str, force: bool) -> Result<String> {
        let options = RemoveImageOptions {
            force,
            ..Default::default()
        };

        self.client.remove_image(image, Some(options), None).await?;
        Ok(format!("Removed image {}", image))
    }

    async fn exec_in_container(&self, container_id: &str, command: Vec<String>) -> Result<String> {
        let exec = self.client.create_exec(container_id, CreateExecOptions {
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            cmd: Some(command),
            ..Default::default()
        }).await?;

        let mut output = String::new();
        if let bollard::exec::StartExecResults::Attached { mut output: stream, .. } = 
            self.client.start_exec(&exec.id, None::<StartExecOptions>).await? {
            while let Some(Ok(msg)) = stream.next().await {
                output.push_str(&msg.to_string());
            }
        }

        Ok(output)
    }
}

#[async_trait]
impl Plugin for DockerPlugin {
    fn name(&self) -> &str {
        "docker"
    }

    fn description(&self) -> &str {
        "Docker container management and operations"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("ps") => {
                let all = command.args.get(1).map(|s| s == "-a").unwrap_or(false);
                self.list_containers(all).await
            }

            Some("pull") => {
                if command.args.len() < 2 {
                    return Ok("Usage: docker pull <image>".to_string());
                }
                self.pull_image(&command.args[1]).await
            }

            Some("run") => {
                if command.args.len() < 3 {
                    return Ok("Usage: docker run <name> <image> [command] [-p port:port] [-v volume:volume]".to_string());
                }
                let name = &command.args[1];
                let image = &command.args[2];
                
                let mut command_vec = Vec::new();
                let mut ports = Vec::new();
                let mut volumes = Vec::new();
                
                let mut i = 3;
                while i < command.args.len() {
                    match command.args[i].as_str() {
                        "-p" => {
                            if i + 1 < command.args.len() {
                                ports.push(command.args[i + 1].clone());
                                i += 2;
                            }
                        }
                        "-v" => {
                            if i + 1 < command.args.len() {
                                volumes.push(command.args[i + 1].clone());
                                i += 2;
                            }
                        }
                        _ => {
                            command_vec.push(command.args[i].clone());
                            i += 1;
                        }
                    }
                }

                let command_opt = if command_vec.is_empty() {
                    None
                } else {
                    Some(command_vec)
                };

                let ports_opt = if ports.is_empty() {
                    None
                } else {
                    Some(ports)
                };

                let volumes_opt = if volumes.is_empty() {
                    None
                } else {
                    Some(volumes)
                };

                self.create_container(name, image, command_opt, ports_opt, volumes_opt).await
            }

            Some("start") => {
                if command.args.len() < 2 {
                    return Ok("Usage: docker start <container_id>".to_string());
                }
                self.start_container(&command.args[1]).await
            }

            Some("stop") => {
                if command.args.len() < 2 {
                    return Ok("Usage: docker stop <container_id>".to_string());
                }
                self.stop_container(&command.args[1]).await
            }

            Some("rm") => {
                if command.args.len() < 2 {
                    return Ok("Usage: docker rm [-f] <container_id>".to_string());
                }
                let force = command.args.contains(&"-f".to_string());
                let container_id = if force {
                    &command.args[2]
                } else {
                    &command.args[1]
                };
                self.remove_container(container_id, force).await
            }

            Some("logs") => {
                if command.args.len() < 2 {
                    return Ok("Usage: docker logs <container_id>".to_string());
                }
                self.container_logs(&command.args[1]).await
            }

            Some("stats") => {
                if command.args.len() < 2 {
                    return Ok("Usage: docker stats <container_id>".to_string());
                }
                self.container_stats(&command.args[1]).await
            }

            Some("images") => {
                self.list_images().await
            }

            Some("rmi") => {
                if command.args.len() < 2 {
                    return Ok("Usage: docker rmi [-f] <image>".to_string());
                }
                let force = command.args.contains(&"-f".to_string());
                let image = if force {
                    &command.args[2]
                } else {
                    &command.args[1]
                };
                self.remove_image(image, force).await
            }

            Some("exec") => {
                if command.args.len() < 3 {
                    return Ok("Usage: docker exec <container_id> <command>".to_string());
                }
                let container_id = &command.args[1];
                let command = command.args[2..].to_vec();
                self.exec_in_container(container_id, command).await
            }

            _ => Ok("Available commands: ps, pull, run, start, stop, rm, logs, stats, images, rmi, exec".to_string()),
        }
    }
}
