use async_trait::async_trait;
use super::super::super::{Command, Environment, Plugin};
use aws_sdk_ec2::{Client as EC2Client, Region};
use aws_sdk_s3::{Client as S3Client};
use aws_sdk_iam::{Client as IAMClient};
use aws_config::meta::region::RegionProviderChain;
use aws_types::region::Region as AwsRegion;
use aws_config::BehaviorVersion;
use anyhow::{Result, Context};
use tokio::fs;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
struct AWSConfig {
    region: String,
    profile: Option<String>,
    output_format: String,
}

impl Default for AWSConfig {
    fn default() -> Self {
        AWSConfig {
            region: "us-west-2".to_string(),
            profile: None,
            output_format: "json".to_string(),
        }
    }
}

pub struct AWSPlugin {
    config: AWSConfig,
    ec2_client: Option<EC2Client>,
    s3_client: Option<S3Client>,
    iam_client: Option<IAMClient>,
}

impl AWSPlugin {
    pub async fn new() -> Self {
        let config = Self::load_config().unwrap_or_default();
        AWSPlugin {
            config,
            ec2_client: None,
            s3_client: None,
            iam_client: None,
        }
    }

    async fn load_config() -> Result<AWSConfig> {
        let mut config_path = dirs::home_dir().unwrap_or_default();
        config_path.push(".nexusshell");
        config_path.push("aws_config.json");

        if !config_path.exists() {
            let config = AWSConfig::default();
            fs::create_dir_all(config_path.parent().unwrap()).await?;
            fs::write(&config_path, serde_json::to_string_pretty(&config)?).await?;
            Ok(config)
        } else {
            let content = fs::read_to_string(&config_path).await?;
            Ok(serde_json::from_str(&content)?)
        }
    }

    async fn init_clients(&mut self) -> Result<()> {
        let region_provider = RegionProviderChain::first_try(AwsRegion::new(self.config.region.clone()))
            .or_default_provider()
            .or_else(Region::new("us-west-2"));

        let shared_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        self.ec2_client = Some(EC2Client::new(&shared_config));
        self.s3_client = Some(S3Client::new(&shared_config));
        self.iam_client = Some(IAMClient::new(&shared_config));

        Ok(())
    }

    async fn list_instances(&self) -> Result<String> {
        let client = self.ec2_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("EC2 client not initialized"))?;

        let resp = client.describe_instances()
            .send()
            .await?;

        let mut output = String::from("EC2 Instances:\n");
        for reservation in resp.reservations().unwrap_or_default() {
            for instance in reservation.instances().unwrap_or_default() {
                let instance_id = instance.instance_id().unwrap_or("Unknown");
                let state = instance.state().map(|s| s.name().as_str()).unwrap_or("Unknown");
                let instance_type = instance.instance_type().map(|t| t.as_str()).unwrap_or("Unknown");
                
                output.push_str(&format!("ID: {} | State: {} | Type: {}\n",
                    instance_id, state, instance_type));
                
                // Add tags if they exist
                if let Some(tags) = instance.tags() {
                    for tag in tags {
                        if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                            output.push_str(&format!("  {}: {}\n", key, value));
                        }
                    }
                }
            }
        }

        Ok(output)
    }

    async fn list_buckets(&self) -> Result<String> {
        let client = self.s3_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("S3 client not initialized"))?;

        let resp = client.list_buckets()
            .send()
            .await?;

        let mut output = String::from("S3 Buckets:\n");
        for bucket in resp.buckets().unwrap_or_default() {
            let name = bucket.name().unwrap_or("Unknown");
            let created = bucket.creation_date()
                .map(|d| d.fmt(aws_sdk_s3::types::DateTime::FORMAT))
                .unwrap_or_else(|| "Unknown".to_string());
            
            output.push_str(&format!("Name: {} | Created: {}\n", name, created));
        }

        Ok(output)
    }

    async fn upload_to_s3(&self, bucket: &str, key: &str, file_path: &PathBuf) -> Result<String> {
        let client = self.s3_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("S3 client not initialized"))?;

        let file_size = fs::metadata(file_path).await?.len();
        let pb = ProgressBar::new(file_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));

        let body = aws_sdk_s3::types::ByteStream::from_path(file_path).await?;
        
        client.put_object()
            .bucket(bucket)
            .key(key)
            .body(body)
            .send()
            .await?;

        pb.finish_with_message("Upload complete");
        Ok(format!("Successfully uploaded {} to s3://{}/{}", file_path.display(), bucket, key))
    }

    async fn download_from_s3(&self, bucket: &str, key: &str, file_path: &PathBuf) -> Result<String> {
        let client = self.s3_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("S3 client not initialized"))?;

        let resp = client.get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await?;

        let size = resp.content_length() as u64;
        let pb = ProgressBar::new(size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));

        let body = resp.body.collect().await?;
        fs::write(file_path, body.into_bytes()).await?;

        pb.finish_with_message("Download complete");
        Ok(format!("Successfully downloaded s3://{}/{} to {}", bucket, key, file_path.display()))
    }

    async fn list_users(&self) -> Result<String> {
        let client = self.iam_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("IAM client not initialized"))?;

        let resp = client.list_users()
            .send()
            .await?;

        let mut output = String::from("IAM Users:\n");
        for user in resp.users().unwrap_or_default() {
            let name = user.user_name().unwrap_or("Unknown");
            let created = user.create_date()
                .map(|d| d.fmt(aws_sdk_iam::types::DateTime::FORMAT))
                .unwrap_or_else(|| "Unknown".to_string());
            
            output.push_str(&format!("Username: {} | Created: {}\n", name, created));
        }

        Ok(output)
    }
}

#[async_trait]
impl Plugin for AWSPlugin {
    fn name(&self) -> &str {
        "aws"
    }

    fn description(&self) -> &str {
        "AWS cloud operations and management"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("configure") => {
                if command.args.len() < 3 {
                    return Ok("Usage: aws configure [region|profile] <value>".to_string());
                }
                let setting = &command.args[1];
                let value = &command.args[2];
                
                match *setting {
                    "region" => {
                        self.config.region = value.to_string();
                        Ok("Region updated successfully".to_string())
                    }
                    "profile" => {
                        self.config.profile = Some(value.to_string());
                        Ok("Profile updated successfully".to_string())
                    }
                    _ => Err(anyhow::anyhow!("Invalid configuration setting"))
                }
            }

            Some("ec2") => {
                match command.args.get(1).map(|s| s.as_str()) {
                    Some("list") => self.list_instances().await,
                    _ => Ok("Available EC2 commands: list".to_string()),
                }
            }

            Some("s3") => {
                match command.args.get(1).map(|s| s.as_str()) {
                    Some("ls") => self.list_buckets().await,
                    Some("cp") => {
                        if command.args.len() != 4 {
                            return Ok("Usage: aws s3 cp <source> <destination>".to_string());
                        }
                        let source = &command.args[2];
                        let dest = &command.args[3];

                        if source.starts_with("s3://") {
                            // Download from S3
                            let parts: Vec<&str> = source[5..].splitn(2, '/').collect();
                            if parts.len() != 2 {
                                return Err(anyhow::anyhow!("Invalid S3 URL"));
                            }
                            self.download_from_s3(parts[0], parts[1], &PathBuf::from(dest)).await
                        } else {
                            // Upload to S3
                            let parts: Vec<&str> = dest[5..].splitn(2, '/').collect();
                            if parts.len() != 2 {
                                return Err(anyhow::anyhow!("Invalid S3 URL"));
                            }
                            self.upload_to_s3(parts[0], parts[1], &PathBuf::from(source)).await
                        }
                    }
                    _ => Ok("Available S3 commands: ls, cp".to_string()),
                }
            }

            Some("iam") => {
                match command.args.get(1).map(|s| s.as_str()) {
                    Some("list-users") => self.list_users().await,
                    _ => Ok("Available IAM commands: list-users".to_string()),
                }
            }

            _ => Ok("Available commands: configure, ec2, s3, iam".to_string()),
        }
    }
}
