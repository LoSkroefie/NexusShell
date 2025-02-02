use async_trait::async_trait;
use super::super::super::{Command, Environment, Plugin};
use azure_identity::DefaultAzureCredential;
use azure_storage::StorageCredentials;
use azure_storage_blobs::prelude::*;
use azure_mgmt_compute::{ComputeClient, VirtualMachine};
use azure_mgmt_storage::StorageAccountClient;
use azure_core::auth::TokenCredential;
use anyhow::{Result, Context};
use tokio::fs;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use indicatif::{ProgressBar, ProgressStyle};
use futures::StreamExt;

#[derive(Debug, Serialize, Deserialize)]
struct AzureConfig {
    subscription_id: String,
    tenant_id: Option<String>,
    resource_group: String,
    location: String,
}

impl Default for AzureConfig {
    fn default() -> Self {
        AzureConfig {
            subscription_id: String::new(),
            tenant_id: None,
            resource_group: "default-rg".to_string(),
            location: "westus2".to_string(),
        }
    }
}

pub struct AzurePlugin {
    config: AzureConfig,
    compute_client: Option<ComputeClient>,
    storage_client: Option<StorageAccountClient>,
    credential: Option<DefaultAzureCredential>,
}

impl AzurePlugin {
    pub async fn new() -> Self {
        let config = Self::load_config().unwrap_or_default();
        AzurePlugin {
            config,
            compute_client: None,
            storage_client: None,
            credential: None,
        }
    }

    async fn load_config() -> Result<AzureConfig> {
        let mut config_path = dirs::home_dir().unwrap_or_default();
        config_path.push(".nexusshell");
        config_path.push("azure_config.json");

        if !config_path.exists() {
            let config = AzureConfig::default();
            fs::create_dir_all(config_path.parent().unwrap()).await?;
            fs::write(&config_path, serde_json::to_string_pretty(&config)?).await?;
            Ok(config)
        } else {
            let content = fs::read_to_string(&config_path).await?;
            Ok(serde_json::from_str(&content)?)
        }
    }

    async fn init_clients(&mut self) -> Result<()> {
        self.credential = Some(DefaultAzureCredential::default());
        let cred = self.credential.as_ref().unwrap();

        self.compute_client = Some(ComputeClient::new(
            cred,
            &self.config.subscription_id
        ));

        self.storage_client = Some(StorageAccountClient::new(
            cred,
            &self.config.subscription_id
        ));

        Ok(())
    }

    async fn list_vms(&self) -> Result<String> {
        let client = self.compute_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Compute client not initialized"))?;

        let vms = client
            .virtual_machines
            .list(&self.config.resource_group)
            .into_stream()
            .collect::<Vec<_>>()
            .await;

        let mut output = String::from("Virtual Machines:\n");
        for vm in vms {
            let vm = vm?;
            output.push_str(&format!("Name: {}\n", vm.name()));
            output.push_str(&format!("  Size: {}\n", vm.hardware_profile.vm_size));
            output.push_str(&format!("  State: {}\n", vm.provisioning_state.unwrap_or_default()));
            
            if let Some(os_profile) = vm.os_profile {
                output.push_str(&format!("  OS: {}\n", os_profile.computer_name.unwrap_or_default()));
            }
        }

        Ok(output)
    }

    async fn list_storage_accounts(&self) -> Result<String> {
        let client = self.storage_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Storage client not initialized"))?;

        let accounts = client
            .list()
            .into_stream()
            .collect::<Vec<_>>()
            .await;

        let mut output = String::from("Storage Accounts:\n");
        for account in accounts {
            let account = account?;
            output.push_str(&format!("Name: {}\n", account.name));
            output.push_str(&format!("  Location: {}\n", account.location));
            output.push_str(&format!("  Kind: {}\n", account.kind));
            
            if let Some(sku) = account.sku {
                output.push_str(&format!("  SKU: {}\n", sku.name));
            }
        }

        Ok(output)
    }

    async fn upload_blob(&self, account: &str, container: &str, blob_name: &str, file_path: &PathBuf) -> Result<String> {
        let credential = self.credential.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Azure credential not initialized"))?;

        let blob_client = BlobClient::new(
            account,
            container,
            blob_name,
            credential.clone()
        );

        let file_size = fs::metadata(file_path).await?.len();
        let pb = ProgressBar::new(file_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));

        let mut file = fs::File::open(file_path).await?;
        blob_client
            .put_block_blob(&mut file)
            .content_length(file_size)
            .send()
            .await?;

        pb.finish_with_message("Upload complete");
        Ok(format!("Successfully uploaded {} to blob storage", file_path.display()))
    }

    async fn download_blob(&self, account: &str, container: &str, blob_name: &str, file_path: &PathBuf) -> Result<String> {
        let credential = self.credential.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Azure credential not initialized"))?;

        let blob_client = BlobClient::new(
            account,
            container,
            blob_name,
            credential.clone()
        );

        let properties = blob_client.get_properties().await?;
        let size = properties.content_length();
        let pb = ProgressBar::new(size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));

        let mut file = fs::File::create(file_path).await?;
        let mut stream = blob_client.get().await?.into_stream();

        while let Some(chunk) = stream.next().await {
            let data = chunk?;
            file.write_all(&data).await?;
            pb.inc(data.len() as u64);
        }

        pb.finish_with_message("Download complete");
        Ok(format!("Successfully downloaded blob to {}", file_path.display()))
    }
}

#[async_trait]
impl Plugin for AzurePlugin {
    fn name(&self) -> &str {
        "azure"
    }

    fn description(&self) -> &str {
        "Azure cloud operations and management"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("configure") => {
                if command.args.len() < 3 {
                    return Ok("Usage: azure configure [subscription|resource-group|location] <value>".to_string());
                }
                let setting = &command.args[1];
                let value = &command.args[2];
                
                match *setting {
                    "subscription" => {
                        self.config.subscription_id = value.to_string();
                        Ok("Subscription ID updated successfully".to_string())
                    }
                    "resource-group" => {
                        self.config.resource_group = value.to_string();
                        Ok("Resource group updated successfully".to_string())
                    }
                    "location" => {
                        self.config.location = value.to_string();
                        Ok("Location updated successfully".to_string())
                    }
                    _ => Err(anyhow::anyhow!("Invalid configuration setting"))
                }
            }

            Some("vm") => {
                match command.args.get(1).map(|s| s.as_str()) {
                    Some("list") => self.list_vms().await,
                    _ => Ok("Available VM commands: list".to_string()),
                }
            }

            Some("storage") => {
                match command.args.get(1).map(|s| s.as_str()) {
                    Some("list") => self.list_storage_accounts().await,
                    Some("upload") => {
                        if command.args.len() != 6 {
                            return Ok("Usage: azure storage upload <account> <container> <blob_name> <file_path>".to_string());
                        }
                        let account = &command.args[2];
                        let container = &command.args[3];
                        let blob_name = &command.args[4];
                        let file_path = PathBuf::from(&command.args[5]);
                        
                        self.upload_blob(account, container, blob_name, &file_path).await
                    }
                    Some("download") => {
                        if command.args.len() != 6 {
                            return Ok("Usage: azure storage download <account> <container> <blob_name> <file_path>".to_string());
                        }
                        let account = &command.args[2];
                        let container = &command.args[3];
                        let blob_name = &command.args[4];
                        let file_path = PathBuf::from(&command.args[5]);
                        
                        self.download_blob(account, container, blob_name, &file_path).await
                    }
                    _ => Ok("Available storage commands: list, upload, download".to_string()),
                }
            }

            _ => Ok("Available commands: configure, vm, storage".to_string()),
        }
    }
}
