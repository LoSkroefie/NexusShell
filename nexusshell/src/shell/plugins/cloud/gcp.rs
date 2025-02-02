use async_trait::async_trait;
use super::super::super::{Command, Environment, Plugin};
use google_cloud_storage::client::{Client as StorageClient, ClientConfig};
use google_cloud_compute::client::{Client as ComputeClient};
use google_cloud_googleapis::cloud::compute::v1::{Instance, ListInstancesRequest};
use google_cloud_googleapis::cloud::storage::v1::{Bucket, Object};
use google_cloud_auth::credentials::CredentialsFile;
use anyhow::{Result, Context};
use tokio::fs;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use indicatif::{ProgressBar, ProgressStyle};
use futures::StreamExt;

#[derive(Debug, Serialize, Deserialize)]
struct GCPConfig {
    project_id: String,
    credentials_file: PathBuf,
    region: String,
    zone: String,
}

impl Default for GCPConfig {
    fn default() -> Self {
        GCPConfig {
            project_id: String::new(),
            credentials_file: PathBuf::new(),
            region: "us-west1".to_string(),
            zone: "us-west1-a".to_string(),
        }
    }
}

pub struct GCPPlugin {
    config: GCPConfig,
    storage_client: Option<StorageClient>,
    compute_client: Option<ComputeClient>,
}

impl GCPPlugin {
    pub async fn new() -> Self {
        let config = Self::load_config().unwrap_or_default();
        GCPPlugin {
            config,
            storage_client: None,
            compute_client: None,
        }
    }

    async fn load_config() -> Result<GCPConfig> {
        let mut config_path = dirs::home_dir().unwrap_or_default();
        config_path.push(".nexusshell");
        config_path.push("gcp_config.json");

        if !config_path.exists() {
            let config = GCPConfig::default();
            fs::create_dir_all(config_path.parent().unwrap()).await?;
            fs::write(&config_path, serde_json::to_string_pretty(&config)?).await?;
            Ok(config)
        } else {
            let content = fs::read_to_string(&config_path).await?;
            Ok(serde_json::from_str(&content)?)
        }
    }

    async fn init_clients(&mut self) -> Result<()> {
        let creds = CredentialsFile::new_from_file(&self.config.credentials_file).await?;

        // Initialize Storage Client
        let storage_config = ClientConfig::default()
            .with_credentials(creds.clone())
            .with_project_id(&self.config.project_id);
        self.storage_client = Some(StorageClient::new(storage_config).await?);

        // Initialize Compute Client
        let compute_config = google_cloud_compute::client::ClientConfig::default()
            .with_credentials(creds)
            .with_project_id(&self.config.project_id);
        self.compute_client = Some(ComputeClient::new(compute_config).await?);

        Ok(())
    }

    async fn list_instances(&self) -> Result<String> {
        let client = self.compute_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Compute client not initialized"))?;

        let request = ListInstancesRequest {
            project: self.config.project_id.clone(),
            zone: self.config.zone.clone(),
            ..Default::default()
        };

        let instances = client.list_instances(request).await?;
        
        let mut output = String::from("Compute Engine Instances:\n");
        for instance in instances.items.unwrap_or_default() {
            output.push_str(&format!("Name: {}\n", instance.name.unwrap_or_default()));
            output.push_str(&format!("  Machine Type: {}\n", instance.machine_type.unwrap_or_default()));
            output.push_str(&format!("  Status: {}\n", instance.status.unwrap_or_default()));
            
            if let Some(network_interfaces) = instance.network_interfaces {
                for interface in network_interfaces {
                    if let Some(ip) = interface.network_ip {
                        output.push_str(&format!("  Network IP: {}\n", ip));
                    }
                }
            }
        }

        Ok(output)
    }

    async fn list_buckets(&self) -> Result<String> {
        let client = self.storage_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Storage client not initialized"))?;

        let buckets = client.list_buckets().await?;
        
        let mut output = String::from("Storage Buckets:\n");
        for bucket in buckets {
            output.push_str(&format!("Name: {}\n", bucket.name()));
            output.push_str(&format!("  Location: {}\n", bucket.location().unwrap_or("Unknown")));
            output.push_str(&format!("  Storage Class: {}\n", bucket.storage_class().unwrap_or("Unknown")));
            
            if let Some(created) = bucket.time_created() {
                output.push_str(&format!("  Created: {}\n", created));
            }
        }

        Ok(output)
    }

    async fn upload_object(&self, bucket_name: &str, object_name: &str, file_path: &PathBuf) -> Result<String> {
        let client = self.storage_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Storage client not initialized"))?;

        let file_size = fs::metadata(file_path).await?.len();
        let pb = ProgressBar::new(file_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));

        let mut file = fs::File::open(file_path).await?;
        client.upload_object(
            &bucket_name,
            &object_name,
            "application/octet-stream",
            &mut file,
        ).await?;

        pb.finish_with_message("Upload complete");
        Ok(format!("Successfully uploaded {} to gs://{}/{}", file_path.display(), bucket_name, object_name))
    }

    async fn download_object(&self, bucket_name: &str, object_name: &str, file_path: &PathBuf) -> Result<String> {
        let client = self.storage_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Storage client not initialized"))?;

        let object = client.get_object(bucket_name, object_name).await?;
        let size = object.size.unwrap_or(0) as u64;
        
        let pb = ProgressBar::new(size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));

        let mut file = fs::File::create(file_path).await?;
        client.download_object(bucket_name, object_name, &mut file).await?;

        pb.finish_with_message("Download complete");
        Ok(format!("Successfully downloaded gs://{}/{} to {}", bucket_name, object_name, file_path.display()))
    }
}

#[async_trait]
impl Plugin for GCPPlugin {
    fn name(&self) -> &str {
        "gcp"
    }

    fn description(&self) -> &str {
        "Google Cloud Platform operations and management"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("configure") => {
                if command.args.len() < 3 {
                    return Ok("Usage: gcp configure [project|credentials|region|zone] <value>".to_string());
                }
                let setting = &command.args[1];
                let value = &command.args[2];
                
                match *setting {
                    "project" => {
                        self.config.project_id = value.to_string();
                        Ok("Project ID updated successfully".to_string())
                    }
                    "credentials" => {
                        self.config.credentials_file = PathBuf::from(value);
                        Ok("Credentials file updated successfully".to_string())
                    }
                    "region" => {
                        self.config.region = value.to_string();
                        Ok("Region updated successfully".to_string())
                    }
                    "zone" => {
                        self.config.zone = value.to_string();
                        Ok("Zone updated successfully".to_string())
                    }
                    _ => Err(anyhow::anyhow!("Invalid configuration setting"))
                }
            }

            Some("compute") => {
                match command.args.get(1).map(|s| s.as_str()) {
                    Some("list") => self.list_instances().await,
                    _ => Ok("Available compute commands: list".to_string()),
                }
            }

            Some("storage") => {
                match command.args.get(1).map(|s| s.as_str()) {
                    Some("ls") => self.list_buckets().await,
                    Some("cp") => {
                        if command.args.len() != 4 {
                            return Ok("Usage: gcp storage cp <source> <destination>".to_string());
                        }
                        let source = &command.args[2];
                        let dest = &command.args[3];

                        if source.starts_with("gs://") {
                            // Download from GCS
                            let parts: Vec<&str> = source[5..].splitn(2, '/').collect();
                            if parts.len() != 2 {
                                return Err(anyhow::anyhow!("Invalid GCS URL"));
                            }
                            self.download_object(parts[0], parts[1], &PathBuf::from(dest)).await
                        } else {
                            // Upload to GCS
                            let parts: Vec<&str> = dest[5..].splitn(2, '/').collect();
                            if parts.len() != 2 {
                                return Err(anyhow::anyhow!("Invalid GCS URL"));
                            }
                            self.upload_object(parts[0], parts[1], &PathBuf::from(source)).await
                        }
                    }
                    _ => Ok("Available storage commands: ls, cp".to_string()),
                }
            }

            _ => Ok("Available commands: configure, compute, storage".to_string()),
        }
    }
}
