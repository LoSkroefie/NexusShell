mod package_manager;
mod formatter;

use async_trait::async_trait;
use super::super::{Command, Environment, Plugin};
use anyhow::Result;
use package_manager::{PackageManager, NodePackageManager, CargoPackageManager, PackageManagerConfig};
use formatter::{FormatterManager, FormatterConfig};
use std::path::PathBuf;
use colored::*;
use std::collections::HashMap;

pub struct DevToolsPlugin {
    npm: NodePackageManager,
    cargo: CargoPackageManager,
    formatter: FormatterManager,
}

impl DevToolsPlugin {
    pub async fn new() -> Result<Self> {
        let package_config = PackageManagerConfig::default();
        let formatter_config = FormatterConfig::default();

        Ok(DevToolsPlugin {
            npm: NodePackageManager::new(package_config.clone()).await?,
            cargo: CargoPackageManager::new(package_config).await?,
            formatter: FormatterManager::new(formatter_config),
        })
    }

    async fn handle_package(&self, args: &[String]) -> Result<String> {
        if args.len() < 3 {
            return Ok("Usage: dev package [npm|cargo] [install|uninstall|update|list|search|info] [args...]".to_string());
        }

        let manager = match args[1].as_str() {
            "npm" => &self.npm as &dyn PackageManager,
            "cargo" => &self.cargo as &dyn PackageManager,
            _ => return Ok("Supported package managers: npm, cargo".to_string()),
        };

        match args[2].as_str() {
            "install" => {
                if args.len() < 4 {
                    return Ok("Usage: dev package [npm|cargo] install <package> [version]".to_string());
                }
                let version = args.get(4).map(|s| s.as_str());
                let package = manager.install(&args[3], version).await?;
                Ok(format!("Installed {} v{}", package.name, package.version))
            }

            "uninstall" => {
                if args.len() < 4 {
                    return Ok("Usage: dev package [npm|cargo] uninstall <package>".to_string());
                }
                manager.uninstall(&args[3]).await?;
                Ok(format!("Uninstalled {}", args[3]))
            }

            "update" => {
                if args.len() < 4 {
                    return Ok("Usage: dev package [npm|cargo] update <package>".to_string());
                }
                let package = manager.update(&args[3]).await?;
                Ok(format!("Updated {} to v{}", package.name, package.version))
            }

            "list" => {
                let packages = manager.list_installed().await?;
                if packages.is_empty() {
                    return Ok("No packages installed".to_string());
                }

                let mut output = String::new();
                output.push_str(&format!("{:<30} {:<15} {:<40}\n",
                    "PACKAGE", "VERSION", "DESCRIPTION"));

                for package in packages {
                    output.push_str(&format!("{:<30} {:<15} {:<40}\n",
                        package.name,
                        package.version,
                        package.description.unwrap_or_default()));
                }

                Ok(output)
            }

            "search" => {
                if args.len() < 4 {
                    return Ok("Usage: dev package [npm|cargo] search <query>".to_string());
                }
                let packages = manager.search(&args[3]).await?;
                if packages.is_empty() {
                    return Ok(format!("No packages found matching '{}'", args[3]));
                }

                let mut output = String::new();
                output.push_str(&format!("{:<30} {:<15} {:<40}\n",
                    "PACKAGE", "VERSION", "DESCRIPTION"));

                for package in packages {
                    output.push_str(&format!("{:<30} {:<15} {:<40}\n",
                        package.name,
                        package.version,
                        package.description.unwrap_or_default()));
                }

                Ok(output)
            }

            "info" => {
                if args.len() < 4 {
                    return Ok("Usage: dev package [npm|cargo] info <package>".to_string());
                }
                let package = manager.get_info(&args[3]).await?;
                
                let mut output = String::new();
                output.push_str(&format!("Package Information for {}\n", package.name.bright_green()));
                output.push_str(&format!("Version: {}\n", package.version));
                if let Some(desc) = package.description {
                    output.push_str(&format!("Description: {}\n", desc));
                }
                if let Some(license) = package.license {
                    output.push_str(&format!("License: {}\n", license));
                }
                if let Some(homepage) = package.homepage {
                    output.push_str(&format!("Homepage: {}\n", homepage));
                }
                if let Some(repo) = package.repository {
                    output.push_str(&format!("Repository: {}\n", repo));
                }
                
                output.push_str("\nDependencies:\n");
                for (dep, version) in package.dependencies {
                    output.push_str(&format!("  {} ({})\n", dep, version));
                }

                Ok(output)
            }

            _ => Ok("Available commands: install, uninstall, update, list, search, info".to_string()),
        }
    }

    async fn handle_format(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: dev format [file|dir] <path> [--recursive]".to_string());
        }

        match args[1].as_str() {
            "file" => {
                if args.len() < 3 {
                    return Ok("Usage: dev format file <path>".to_string());
                }
                let path = PathBuf::from(&args[2]);
                match self.formatter.format_file(&path).await {
                    Ok(true) => Ok(format!("Formatted {}", path.display())),
                    Ok(false) => Ok(format!("No changes needed for {}", path.display())),
                    Err(e) => Ok(format!("Error formatting {}: {}", path.display(), e)),
                }
            }

            "dir" => {
                if args.len() < 3 {
                    return Ok("Usage: dev format dir <path> [--recursive]".to_string());
                }
                let path = PathBuf::from(&args[2]);
                let recursive = args.get(3).map_or(false, |arg| arg == "--recursive");

                let stats = self.formatter.format_directory(&path, recursive).await?;
                
                let mut output = String::new();
                output.push_str(&format!("Formatting Results:\n"));
                output.push_str(&format!("Files processed: {}\n", stats.files_processed));
                output.push_str(&format!("Files changed: {}\n", stats.files_changed));
                output.push_str(&format!("Total changes: {}\n", stats.total_changes));

                if !stats.errors.is_empty() {
                    output.push_str("\nErrors:\n");
                    for error in stats.errors {
                        output.push_str(&format!("  {}\n", error));
                    }
                }

                Ok(output)
            }

            _ => Ok("Available commands: file, dir".to_string()),
        }
    }

    async fn handle_config(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: dev config [formatter|package] [args...]".to_string());
        }

        match args[1].as_str() {
            "formatter" => {
                if args.len() < 3 {
                    let config = self.formatter.get_formatter("rust")
                        .map(|f| f.get_config())
                        .ok_or_else(|| anyhow::anyhow!("No formatter found"))?;

                    let mut output = String::new();
                    output.push_str("Current Formatter Configuration:\n");
                    output.push_str(&format!("Indent Style: {}\n", config.indent_style));
                    output.push_str(&format!("Indent Size: {}\n", config.indent_size));
                    output.push_str(&format!("Line Width: {}\n", config.line_width));
                    output.push_str(&format!("End of Line: {}\n", config.end_of_line));
                    output.push_str(&format!("Insert Final Newline: {}\n", config.insert_final_newline));
                    output.push_str(&format!("Trim Trailing Whitespace: {}\n", config.trim_trailing_whitespace));

                    Ok(output)
                } else {
                    let mut config = FormatterConfig::default();
                    let mut i = 2;
                    while i < args.len() {
                        match args[i].as_str() {
                            "--indent-style" => {
                                if i + 1 < args.len() {
                                    config.indent_style = args[i + 1].clone();
                                    i += 2;
                                }
                            }
                            "--indent-size" => {
                                if i + 1 < args.len() {
                                    config.indent_size = args[i + 1].parse()?;
                                    i += 2;
                                }
                            }
                            "--line-width" => {
                                if i + 1 < args.len() {
                                    config.line_width = args[i + 1].parse()?;
                                    i += 2;
                                }
                            }
                            "--end-of-line" => {
                                if i + 1 < args.len() {
                                    config.end_of_line = args[i + 1].clone();
                                    i += 2;
                                }
                            }
                            "--insert-final-newline" => {
                                if i + 1 < args.len() {
                                    config.insert_final_newline = args[i + 1].parse()?;
                                    i += 2;
                                }
                            }
                            "--trim-trailing-whitespace" => {
                                if i + 1 < args.len() {
                                    config.trim_trailing_whitespace = args[i + 1].parse()?;
                                    i += 2;
                                }
                            }
                            _ => i += 1,
                        }
                    }

                    self.formatter.update_config(config);
                    Ok("Updated formatter configuration".to_string())
                }
            }

            "package" => {
                if args.len() < 3 {
                    return Ok("Usage: dev config package [npm|cargo] [args...]".to_string());
                }

                match args[2].as_str() {
                    "npm" | "cargo" => {
                        if args.len() < 4 {
                            let config = PackageManagerConfig::default();
                            let mut output = String::new();
                            output.push_str(&format!("Current Package Manager Configuration ({}):\n", args[2]));
                            output.push_str(&format!("Default Registry: {}\n", config.default_registry));
                            output.push_str(&format!("Cache Directory: {}\n", config.cache_dir.display()));
                            output.push_str(&format!("Max Concurrent Downloads: {}\n", config.max_concurrent_downloads));
                            output.push_str(&format!("Timeout: {:?}\n", config.timeout));

                            Ok(output)
                        } else {
                            let mut config = PackageManagerConfig::default();
                            let mut i = 3;
                            while i < args.len() {
                                match args[i].as_str() {
                                    "--registry" => {
                                        if i + 1 < args.len() {
                                            config.default_registry = args[i + 1].clone();
                                            i += 2;
                                        }
                                    }
                                    "--cache-dir" => {
                                        if i + 1 < args.len() {
                                            config.cache_dir = PathBuf::from(&args[i + 1]);
                                            i += 2;
                                        }
                                    }
                                    "--max-downloads" => {
                                        if i + 1 < args.len() {
                                            config.max_concurrent_downloads = args[i + 1].parse()?;
                                            i += 2;
                                        }
                                    }
                                    "--timeout" => {
                                        if i + 1 < args.len() {
                                            config.timeout = std::time::Duration::from_secs(args[i + 1].parse()?);
                                            i += 2;
                                        }
                                    }
                                    _ => i += 1,
                                }
                            }

                            match args[2].as_str() {
                                "npm" => {
                                    self.npm = NodePackageManager::new(config).await?;
                                }
                                "cargo" => {
                                    self.cargo = CargoPackageManager::new(config).await?;
                                }
                                _ => unreachable!(),
                            }

                            Ok(format!("Updated {} package manager configuration", args[2]))
                        }
                    }
                    _ => Ok("Supported package managers: npm, cargo".to_string()),
                }
            }

            _ => Ok("Available config types: formatter, package".to_string()),
        }
    }
}

#[async_trait]
impl Plugin for DevToolsPlugin {
    fn name(&self) -> &str {
        "dev"
    }

    fn description(&self) -> &str {
        "Development tools and utilities"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("package") => self.handle_package(&command.args).await,
            Some("format") => self.handle_format(&command.args).await,
            Some("config") => self.handle_config(&command.args).await,
            _ => Ok("Available commands: package, format, config".to_string()),
        }
    }
}
