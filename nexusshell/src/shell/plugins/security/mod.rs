use async_trait::async_trait;
use super::super::{Command, Environment, Plugin};
use anyhow::Result;
use serde::{Serialize, Deserialize};
use tokio::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use ring::{aead, digest, pbkdf2};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::num::NonZeroU32;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use colored::*;

const CREDENTIAL_STORE_PATH: &str = ".nexusshell/credentials";
const KEY_STORE_PATH: &str = ".nexusshell/keys";
const AUDIT_LOG_PATH: &str = ".nexusshell/audit.log";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    id: String,
    name: String,
    username: String,
    encrypted_password: String,
    salt: String,
    created_at: DateTime<Utc>,
    last_used: Option<DateTime<Utc>>,
    metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    id: String,
    name: String,
    public_key: String,
    encrypted_private_key: String,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    timestamp: DateTime<Utc>,
    action: String,
    user: String,
    resource: String,
    status: String,
    details: Option<String>,
}

pub struct SecurityPlugin {
    master_key: Vec<u8>,
    credentials: HashMap<String, Credential>,
    keys: HashMap<String, KeyPair>,
}

impl SecurityPlugin {
    pub async fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

        // Create security directories if they don't exist
        for dir in &[CREDENTIAL_STORE_PATH, KEY_STORE_PATH] {
            fs::create_dir_all(home_dir.join(dir)).await?;
        }

        // Initialize master key
        let master_key = Self::get_or_create_master_key().await?;

        Ok(SecurityPlugin {
            master_key,
            credentials: HashMap::new(),
            keys: HashMap::new(),
        })
    }

    async fn get_or_create_master_key() -> Result<Vec<u8>> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let master_key_path = home_dir.join(".nexusshell/master.key");

        if master_key_path.exists() {
            Ok(fs::read(&master_key_path).await?)
        } else {
            let key = ring::rand::SystemRandom::new()
                .generate_vec(32)?;
            fs::write(&master_key_path, &key).await?;
            Ok(key)
        }
    }

    fn encrypt(&self, data: &[u8]) -> Result<(String, String)> {
        let salt = ring::rand::SystemRandom::new()
            .generate_vec(16)?;

        let mut key = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(100_000).unwrap(),
            &salt,
            &self.master_key,
            &mut key,
        );

        let nonce = ring::rand::SystemRandom::new()
            .generate_vec(12)?;

        let sealing_key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &key)
            .map_err(|_| anyhow::anyhow!("Failed to create sealing key"))?;
        let sealed_key = aead::SealingKey::new(sealing_key, &nonce);

        let mut in_out = data.to_vec();
        let tag = sealed_key
            .seal_in_place_append_tag(&[], &mut in_out)
            .map_err(|_| anyhow::anyhow!("Failed to encrypt data"))?;

        Ok((
            BASE64.encode(&in_out),
            BASE64.encode(&salt),
        ))
    }

    fn decrypt(&self, encrypted_data: &str, salt: &str) -> Result<Vec<u8>> {
        let encrypted_bytes = BASE64.decode(encrypted_data)?;
        let salt = BASE64.decode(salt)?;

        let mut key = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(100_000).unwrap(),
            &salt,
            &self.master_key,
            &mut key,
        );

        let opening_key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &key)
            .map_err(|_| anyhow::anyhow!("Failed to create opening key"))?;

        let nonce = &encrypted_bytes[..12];
        let mut in_out = encrypted_bytes[12..].to_vec();

        let opening_key = aead::OpeningKey::new(opening_key, nonce);
        opening_key
            .open_in_place(&[], &mut in_out)
            .map_err(|_| anyhow::anyhow!("Failed to decrypt data"))?;

        Ok(in_out)
    }

    async fn log_audit(&self, entry: AuditLogEntry) -> Result<()> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let log_path = home_dir.join(AUDIT_LOG_PATH);

        let entry_json = serde_json::to_string(&entry)?;
        fs::write(&log_path, format!("{}\n", entry_json)).await?;

        Ok(())
    }

    async fn handle_credential(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: security credential [add|get|list|delete] [args...]".to_string());
        }

        match args[1].as_str() {
            "add" => {
                if args.len() < 5 {
                    return Ok("Usage: security credential add <name> <username> <password>".to_string());
                }

                let (encrypted_password, salt) = self.encrypt(args[4].as_bytes())?;

                let credential = Credential {
                    id: Uuid::new_v4().to_string(),
                    name: args[2].clone(),
                    username: args[3].clone(),
                    encrypted_password,
                    salt,
                    created_at: Utc::now(),
                    last_used: None,
                    metadata: HashMap::new(),
                };

                self.credentials.insert(credential.id.clone(), credential.clone());

                self.log_audit(AuditLogEntry {
                    timestamp: Utc::now(),
                    action: "credential_add".to_string(),
                    user: credential.username.clone(),
                    resource: credential.name,
                    status: "success".to_string(),
                    details: None,
                }).await?;

                Ok("Credential added successfully".to_string())
            }

            "get" => {
                if args.len() < 3 {
                    return Ok("Usage: security credential get <name>".to_string());
                }

                if let Some(credential) = self.credentials.values()
                    .find(|c| c.name == args[2])
                {
                    let password = self.decrypt(&credential.encrypted_password, &credential.salt)?;
                    let password = String::from_utf8(password)?;

                    self.log_audit(AuditLogEntry {
                        timestamp: Utc::now(),
                        action: "credential_get".to_string(),
                        user: credential.username.clone(),
                        resource: credential.name.clone(),
                        status: "success".to_string(),
                        details: None,
                    }).await?;

                    Ok(format!("Username: {}\nPassword: {}", credential.username, password))
                } else {
                    Ok(format!("Credential '{}' not found", args[2]))
                }
            }

            "list" => {
                let mut output = String::new();
                output.push_str(&format!("{:<36} {:<20} {:<20} {:<30}\n",
                    "ID", "NAME", "USERNAME", "CREATED AT"));

                for credential in self.credentials.values() {
                    output.push_str(&format!("{:<36} {:<20} {:<20} {:<30}\n",
                        credential.id,
                        credential.name,
                        credential.username,
                        credential.created_at.to_rfc3339()));
                }

                Ok(output)
            }

            "delete" => {
                if args.len() < 3 {
                    return Ok("Usage: security credential delete <name>".to_string());
                }

                if let Some(credential) = self.credentials.values()
                    .find(|c| c.name == args[2])
                {
                    self.credentials.remove(&credential.id);

                    self.log_audit(AuditLogEntry {
                        timestamp: Utc::now(),
                        action: "credential_delete".to_string(),
                        user: credential.username.clone(),
                        resource: credential.name.clone(),
                        status: "success".to_string(),
                        details: None,
                    }).await?;

                    Ok(format!("Credential '{}' deleted", args[2]))
                } else {
                    Ok(format!("Credential '{}' not found", args[2]))
                }
            }

            _ => Ok("Available commands: add, get, list, delete".to_string()),
        }
    }

    async fn handle_key(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: security key [generate|import|export|list|delete] [args...]".to_string());
        }

        match args[1].as_str() {
            "generate" => {
                if args.len() < 3 {
                    return Ok("Usage: security key generate <name>".to_string());
                }

                let key_pair = ring::signature::Ed25519KeyPair::generate(
                    &ring::rand::SystemRandom::new())?;

                let (encrypted_private_key, salt) = self.encrypt(key_pair.as_ref())?;

                let key = KeyPair {
                    id: Uuid::new_v4().to_string(),
                    name: args[2].clone(),
                    public_key: BASE64.encode(key_pair.public_key().as_ref()),
                    encrypted_private_key,
                    created_at: Utc::now(),
                    expires_at: None,
                    metadata: HashMap::new(),
                };

                self.keys.insert(key.id.clone(), key.clone());

                self.log_audit(AuditLogEntry {
                    timestamp: Utc::now(),
                    action: "key_generate".to_string(),
                    user: "system".to_string(),
                    resource: key.name,
                    status: "success".to_string(),
                    details: None,
                }).await?;

                Ok("Key pair generated successfully".to_string())
            }

            "import" => {
                if args.len() < 4 {
                    return Ok("Usage: security key import <name> <private_key_path>".to_string());
                }

                let private_key = fs::read(&args[3]).await?;
                let (encrypted_private_key, salt) = self.encrypt(&private_key)?;

                let key = KeyPair {
                    id: Uuid::new_v4().to_string(),
                    name: args[2].clone(),
                    public_key: "".to_string(), // Would need to derive public key from private key
                    encrypted_private_key,
                    created_at: Utc::now(),
                    expires_at: None,
                    metadata: HashMap::new(),
                };

                self.keys.insert(key.id.clone(), key.clone());

                self.log_audit(AuditLogEntry {
                    timestamp: Utc::now(),
                    action: "key_import".to_string(),
                    user: "system".to_string(),
                    resource: key.name,
                    status: "success".to_string(),
                    details: None,
                }).await?;

                Ok("Key imported successfully".to_string())
            }

            "export" => {
                if args.len() < 4 {
                    return Ok("Usage: security key export <name> <output_path>".to_string());
                }

                if let Some(key) = self.keys.values()
                    .find(|k| k.name == args[2])
                {
                    let private_key = self.decrypt(&key.encrypted_private_key, &key.salt)?;
                    fs::write(&args[3], private_key).await?;

                    self.log_audit(AuditLogEntry {
                        timestamp: Utc::now(),
                        action: "key_export".to_string(),
                        user: "system".to_string(),
                        resource: key.name.clone(),
                        status: "success".to_string(),
                        details: Some(format!("Exported to {}", args[3])),
                    }).await?;

                    Ok(format!("Key exported to {}", args[3]))
                } else {
                    Ok(format!("Key '{}' not found", args[2]))
                }
            }

            "list" => {
                let mut output = String::new();
                output.push_str(&format!("{:<36} {:<20} {:<30} {:<20}\n",
                    "ID", "NAME", "CREATED AT", "EXPIRES AT"));

                for key in self.keys.values() {
                    output.push_str(&format!("{:<36} {:<20} {:<30} {:<20}\n",
                        key.id,
                        key.name,
                        key.created_at.to_rfc3339(),
                        key.expires_at.map_or("Never".to_string(), |dt| dt.to_rfc3339())));
                }

                Ok(output)
            }

            "delete" => {
                if args.len() < 3 {
                    return Ok("Usage: security key delete <name>".to_string());
                }

                if let Some(key) = self.keys.values()
                    .find(|k| k.name == args[2])
                {
                    self.keys.remove(&key.id);

                    self.log_audit(AuditLogEntry {
                        timestamp: Utc::now(),
                        action: "key_delete".to_string(),
                        user: "system".to_string(),
                        resource: key.name.clone(),
                        status: "success".to_string(),
                        details: None,
                    }).await?;

                    Ok(format!("Key '{}' deleted", args[2]))
                } else {
                    Ok(format!("Key '{}' not found", args[2]))
                }
            }

            _ => Ok("Available commands: generate, import, export, list, delete".to_string()),
        }
    }

    async fn handle_audit(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: security audit [list|export] [args...]".to_string());
        }

        match args[1].as_str() {
            "list" => {
                let home_dir = dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
                let log_path = home_dir.join(AUDIT_LOG_PATH);

                let content = fs::read_to_string(&log_path).await?;
                let mut entries: Vec<AuditLogEntry> = Vec::new();

                for line in content.lines() {
                    if let Ok(entry) = serde_json::from_str(line) {
                        entries.push(entry);
                    }
                }

                let mut output = String::new();
                output.push_str(&format!("{:<30} {:<15} {:<15} {:<20} {:<10}\n",
                    "TIMESTAMP", "ACTION", "USER", "RESOURCE", "STATUS"));

                for entry in entries {
                    output.push_str(&format!("{:<30} {:<15} {:<15} {:<20} {:<10}\n",
                        entry.timestamp.to_rfc3339(),
                        entry.action,
                        entry.user,
                        entry.resource,
                        entry.status));
                }

                Ok(output)
            }

            "export" => {
                if args.len() < 3 {
                    return Ok("Usage: security audit export <output_path>".to_string());
                }

                let home_dir = dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
                let log_path = home_dir.join(AUDIT_LOG_PATH);

                fs::copy(&log_path, &args[2]).await?;

                Ok(format!("Audit log exported to {}", args[2]))
            }

            _ => Ok("Available commands: list, export".to_string()),
        }
    }
}

#[async_trait]
impl Plugin for SecurityPlugin {
    fn name(&self) -> &str {
        "security"
    }

    fn description(&self) -> &str {
        "Security and credential management"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("credential") => self.handle_credential(&command.args).await,
            Some("key") => self.handle_key(&command.args).await,
            Some("audit") => self.handle_audit(&command.args).await,
            _ => Ok("Available commands: credential, key, audit".to_string()),
        }
    }
}
