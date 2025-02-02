mod aws;
mod azure;
mod gcp;

pub use aws::AWSPlugin;
pub use azure::AzurePlugin;
pub use gcp::GCPPlugin;

use async_trait::async_trait;
use super::super::{Command, Environment};
use anyhow::Result;
use std::path::PathBuf;

#[async_trait]
pub trait CloudStorageProvider: Send + Sync {
    async fn upload_file(&self, source: &PathBuf, destination: &str) -> Result<String>;
    async fn download_file(&self, source: &str, destination: &PathBuf) -> Result<String>;
    async fn list_storage(&self) -> Result<String>;
}

#[async_trait]
pub trait CloudComputeProvider: Send + Sync {
    async fn list_instances(&self) -> Result<String>;
    async fn start_instance(&self, instance_id: &str) -> Result<String>;
    async fn stop_instance(&self, instance_id: &str) -> Result<String>;
}

pub struct CloudManager {
    aws: AWSPlugin,
    azure: AzurePlugin,
    gcp: GCPPlugin,
}

impl CloudManager {
    pub async fn new() -> Self {
        CloudManager {
            aws: AWSPlugin::new().await,
            azure: AzurePlugin::new().await,
            gcp: GCPPlugin::new().await,
        }
    }

    pub fn get_aws(&mut self) -> &mut AWSPlugin {
        &mut self.aws
    }

    pub fn get_azure(&mut self) -> &mut AzurePlugin {
        &mut self.azure
    }

    pub fn get_gcp(&mut self) -> &mut GCPPlugin {
        &mut self.gcp
    }

    pub async fn execute(&self, provider: &str, command: &Command, env: &Environment) -> Result<String> {
        match provider {
            "aws" => self.aws.execute(command, env).await,
            "azure" => self.azure.execute(command, env).await,
            "gcp" => self.gcp.execute(command, env).await,
            _ => Err(anyhow::anyhow!("Unknown cloud provider")),
        }
    }
}
