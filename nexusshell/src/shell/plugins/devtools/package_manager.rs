use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use tokio::process::Command;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use semver::Version;
use regex::Regex;
use lazy_static::lazy_static;
use chrono::{DateTime, Utc};
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: Version,
    pub description: Option<String>,
    pub dependencies: HashMap<String, String>,
    pub installed_at: DateTime<Utc>,
    pub size: u64,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManagerConfig {
    pub default_registry: String,
    pub cache_dir: PathBuf,
    pub max_concurrent_downloads: usize,
    pub timeout: std::time::Duration,
}

impl Default for PackageManagerConfig {
    fn default() -> Self {
        PackageManagerConfig {
            default_registry: String::from("https://registry.npmjs.org"),
            cache_dir: dirs::cache_dir().unwrap_or_default().join("nexusshell/packages"),
            max_concurrent_downloads: 5,
            timeout: std::time::Duration::from_secs(300),
        }
    }
}

#[async_trait]
pub trait PackageManager: Send + Sync {
    async fn install(&self, package: &str, version: Option<&str>) -> Result<Package>;
    async fn uninstall(&self, package: &str) -> Result<()>;
    async fn update(&self, package: &str) -> Result<Package>;
    async fn list_installed(&self) -> Result<Vec<Package>>;
    async fn search(&self, query: &str) -> Result<Vec<Package>>;
    async fn get_info(&self, package: &str) -> Result<Package>;
}

pub struct NodePackageManager {
    config: PackageManagerConfig,
    installed_packages: HashMap<String, Package>,
}

impl NodePackageManager {
    pub async fn new(config: PackageManagerConfig) -> Result<Self> {
        fs::create_dir_all(&config.cache_dir).await?;
        Ok(NodePackageManager {
            config,
            installed_packages: HashMap::new(),
        })
    }

    async fn run_npm_command(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("npm")
            .args(args)
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow::anyhow!("npm command failed: {}", 
                String::from_utf8_lossy(&output.stderr)))
        }
    }

    async fn parse_package_json(&self, content: &str) -> Result<Package> {
        let json: serde_json::Value = serde_json::from_str(content)?;
        
        Ok(Package {
            name: json["name"].as_str().unwrap_or_default().to_string(),
            version: Version::parse(json["version"].as_str().unwrap_or("0.0.0"))?,
            description: json["description"].as_str().map(String::from),
            dependencies: json["dependencies"]
                .as_object()
                .map(|deps| deps.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect())
                .unwrap_or_default(),
            installed_at: Utc::now(),
            size: json["size"].as_u64().unwrap_or(0),
            license: json["license"].as_str().map(String::from),
            homepage: json["homepage"].as_str().map(String::from),
            repository: json["repository"]
                .as_object()
                .and_then(|repo| repo["url"].as_str())
                .map(String::from),
        })
    }
}

#[async_trait]
impl PackageManager for NodePackageManager {
    async fn install(&self, package: &str, version: Option<&str>) -> Result<Package> {
        let package_spec = match version {
            Some(v) => format!("{}@{}", package, v),
            None => package.to_string(),
        };

        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"));
        pb.set_message(format!("Installing {}", package_spec));

        let output = self.run_npm_command(&["install", &package_spec]).await?;
        pb.finish_with_message(format!("Installed {}", package_spec));

        // Parse installed package info
        let package_json = self.run_npm_command(&["list", &package_spec, "--json"]).await?;
        self.parse_package_json(&package_json).await
    }

    async fn uninstall(&self, package: &str) -> Result<()> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.red} [{elapsed_precise}] {msg}")
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"));
        pb.set_message(format!("Uninstalling {}", package));

        self.run_npm_command(&["uninstall", package]).await?;
        pb.finish_with_message(format!("Uninstalled {}", package));
        Ok(())
    }

    async fn update(&self, package: &str) -> Result<Package> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.blue} [{elapsed_precise}] {msg}")
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"));
        pb.set_message(format!("Updating {}", package));

        self.run_npm_command(&["update", package]).await?;
        pb.finish_with_message(format!("Updated {}", package));

        let package_json = self.run_npm_command(&["list", package, "--json"]).await?;
        self.parse_package_json(&package_json).await
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        let output = self.run_npm_command(&["list", "--json"]).await?;
        let json: serde_json::Value = serde_json::from_str(&output)?;
        
        let mut packages = Vec::new();
        if let Some(deps) = json["dependencies"].as_object() {
            for (name, info) in deps {
                if let Ok(package) = self.parse_package_json(&serde_json::to_string(info)?).await {
                    packages.push(package);
                }
            }
        }

        Ok(packages)
    }

    async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let output = self.run_npm_command(&["search", query, "--json"]).await?;
        let results: Vec<serde_json::Value> = serde_json::from_str(&output)?;
        
        let mut packages = Vec::new();
        for result in results {
            if let Ok(package) = self.parse_package_json(&serde_json::to_string(&result)?).await {
                packages.push(package);
            }
        }

        Ok(packages)
    }

    async fn get_info(&self, package: &str) -> Result<Package> {
        let output = self.run_npm_command(&["view", package, "--json"]).await?;
        self.parse_package_json(&output).await
    }
}

pub struct CargoPackageManager {
    config: PackageManagerConfig,
    installed_packages: HashMap<String, Package>,
}

impl CargoPackageManager {
    pub async fn new(config: PackageManagerConfig) -> Result<Self> {
        fs::create_dir_all(&config.cache_dir).await?;
        Ok(CargoPackageManager {
            config,
            installed_packages: HashMap::new(),
        })
    }

    async fn run_cargo_command(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("cargo")
            .args(args)
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow::anyhow!("cargo command failed: {}", 
                String::from_utf8_lossy(&output.stderr)))
        }
    }

    async fn parse_cargo_toml(&self, content: &str) -> Result<Package> {
        let toml: toml::Value = toml::from_str(content)?;
        
        Ok(Package {
            name: toml["package"]["name"].as_str().unwrap_or_default().to_string(),
            version: Version::parse(toml["package"]["version"].as_str().unwrap_or("0.0.0"))?,
            description: toml["package"]["description"].as_str().map(String::from),
            dependencies: toml["dependencies"]
                .as_table()
                .map(|deps| deps.iter()
                    .map(|(k, v)| match v {
                        toml::Value::String(s) => (k.clone(), s.clone()),
                        toml::Value::Table(t) => (k.clone(), t.get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string()),
                        _ => (k.clone(), String::new()),
                    })
                    .collect())
                .unwrap_or_default(),
            installed_at: Utc::now(),
            size: 0,
            license: toml["package"]["license"].as_str().map(String::from),
            homepage: toml["package"]["homepage"].as_str().map(String::from),
            repository: toml["package"]["repository"].as_str().map(String::from),
        })
    }
}

#[async_trait]
impl PackageManager for CargoPackageManager {
    async fn install(&self, package: &str, version: Option<&str>) -> Result<Package> {
        let package_spec = match version {
            Some(v) => format!("{}:{}", package, v),
            None => package.to_string(),
        };

        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"));
        pb.set_message(format!("Installing {}", package_spec));

        self.run_cargo_command(&["install", &package_spec]).await?;
        pb.finish_with_message(format!("Installed {}", package_spec));

        // Get package info from crates.io
        let url = format!("https://crates.io/api/v1/crates/{}", package);
        let response = reqwest::get(&url).await?;
        let info: serde_json::Value = response.json().await?;
        
        self.parse_cargo_toml(&serde_json::to_string(&info["crate"])?).await
    }

    async fn uninstall(&self, package: &str) -> Result<()> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.red} [{elapsed_precise}] {msg}")
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"));
        pb.set_message(format!("Uninstalling {}", package));

        self.run_cargo_command(&["uninstall", package]).await?;
        pb.finish_with_message(format!("Uninstalled {}", package));
        Ok(())
    }

    async fn update(&self, package: &str) -> Result<Package> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.blue} [{elapsed_precise}] {msg}")
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"));
        pb.set_message(format!("Updating {}", package));

        self.run_cargo_command(&["install", package, "--force"]).await?;
        pb.finish_with_message(format!("Updated {}", package));

        let url = format!("https://crates.io/api/v1/crates/{}", package);
        let response = reqwest::get(&url).await?;
        let info: serde_json::Value = response.json().await?;
        
        self.parse_cargo_toml(&serde_json::to_string(&info["crate"])?).await
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        let output = self.run_cargo_command(&["install", "--list"]).await?;
        let mut packages = Vec::new();

        lazy_static! {
            static ref PKG_RE: Regex = Regex::new(
                r"(?P<name>[^\s]+)\sv(?P<version>[^\s]+):"
            ).unwrap();
        }

        for line in output.lines() {
            if let Some(caps) = PKG_RE.captures(line) {
                let name = caps.name("name").unwrap().as_str();
                let version = caps.name("version").unwrap().as_str();

                let url = format!("https://crates.io/api/v1/crates/{}", name);
                if let Ok(response) = reqwest::get(&url).await {
                    if let Ok(info) = response.json::<serde_json::Value>().await {
                        if let Ok(package) = self.parse_cargo_toml(
                            &serde_json::to_string(&info["crate"])?
                        ).await {
                            packages.push(package);
                        }
                    }
                }
            }
        }

        Ok(packages)
    }

    async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let url = format!(
            "https://crates.io/api/v1/crates?q={}&per_page=10",
            urlencoding::encode(query)
        );
        let response = reqwest::get(&url).await?;
        let results: serde_json::Value = response.json().await?;
        
        let mut packages = Vec::new();
        if let Some(crates) = results["crates"].as_array() {
            for crate_info in crates {
                if let Ok(package) = self.parse_cargo_toml(
                    &serde_json::to_string(crate_info)?
                ).await {
                    packages.push(package);
                }
            }
        }

        Ok(packages)
    }

    async fn get_info(&self, package: &str) -> Result<Package> {
        let url = format!("https://crates.io/api/v1/crates/{}", package);
        let response = reqwest::get(&url).await?;
        let info: serde_json::Value = response.json().await?;
        
        self.parse_cargo_toml(&serde_json::to_string(&info["crate"])?).await
    }
}
