mod docker;
mod kubernetes;

pub use docker::DockerPlugin;
pub use kubernetes::KubernetesPlugin;

use async_trait::async_trait;
use super::super::{Command, Environment};
use anyhow::Result;
use std::path::PathBuf;

#[async_trait]
pub trait ContainerProvider: Send + Sync {
    async fn list_containers(&self) -> Result<String>;
    async fn start_container(&self, container_id: &str) -> Result<String>;
    async fn stop_container(&self, container_id: &str) -> Result<String>;
    async fn remove_container(&self, container_id: &str) -> Result<String>;
    async fn container_logs(&self, container_id: &str) -> Result<String>;
}

pub struct ContainerManager {
    docker: DockerPlugin,
    kubernetes: KubernetesPlugin,
}

impl ContainerManager {
    pub async fn new() -> Result<Self> {
        Ok(ContainerManager {
            docker: DockerPlugin::new().await?,
            kubernetes: KubernetesPlugin::new().await?,
        })
    }

    pub fn get_docker(&mut self) -> &mut DockerPlugin {
        &mut self.docker
    }

    pub fn get_kubernetes(&mut self) -> &mut KubernetesPlugin {
        &mut self.kubernetes
    }

    pub async fn execute(&self, provider: &str, command: &Command, env: &Environment) -> Result<String> {
        match provider {
            "docker" => self.docker.execute(command, env).await,
            "kubectl" => self.kubernetes.execute(command, env).await,
            _ => Err(anyhow::anyhow!("Unknown container provider")),
        }
    }
}
