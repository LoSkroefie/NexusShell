use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use tokio::process::Command;
use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::fs;
use regex::Regex;
use lazy_static::lazy_static;
use std::collections::HashMap;
use ignore::Walk;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatterConfig {
    pub indent_style: String,
    pub indent_size: u8,
    pub line_width: u16,
    pub end_of_line: String,
    pub insert_final_newline: bool,
    pub trim_trailing_whitespace: bool,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        FormatterConfig {
            indent_style: String::from("space"),
            indent_size: 4,
            line_width: 100,
            end_of_line: String::from("lf"),
            insert_final_newline: true,
            trim_trailing_whitespace: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattingStats {
    pub files_processed: usize,
    pub files_changed: usize,
    pub total_changes: usize,
    pub errors: Vec<String>,
}

#[async_trait]
pub trait CodeFormatter: Send + Sync {
    async fn format_file(&self, path: &Path) -> Result<bool>;
    async fn format_directory(&self, path: &Path, recursive: bool) -> Result<FormattingStats>;
    fn supports_language(&self, language: &str) -> bool;
    fn get_config(&self) -> &FormatterConfig;
    fn set_config(&mut self, config: FormatterConfig);
}

pub struct RustFormatter {
    config: FormatterConfig,
}

impl RustFormatter {
    pub fn new(config: FormatterConfig) -> Self {
        RustFormatter { config }
    }

    async fn run_rustfmt(&self, path: &Path) -> Result<bool> {
        let output = Command::new("rustfmt")
            .arg(path)
            .output()
            .await?;

        Ok(output.status.success())
    }
}

#[async_trait]
impl CodeFormatter for RustFormatter {
    async fn format_file(&self, path: &Path) -> Result<bool> {
        if path.extension().map_or(false, |ext| ext == "rs") {
            self.run_rustfmt(path).await
        } else {
            Ok(false)
        }
    }

    async fn format_directory(&self, path: &Path, recursive: bool) -> Result<FormattingStats> {
        let mut stats = FormattingStats {
            files_processed: 0,
            files_changed: 0,
            total_changes: 0,
            errors: Vec::new(),
        };

        let walker = if recursive {
            Walk::new(path)
        } else {
            Walk::new(path).max_depth(1)
        };

        for entry in walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "rs") {
                        stats.files_processed += 1;
                        match self.format_file(path).await {
                            Ok(true) => {
                                stats.files_changed += 1;
                                stats.total_changes += 1;
                            }
                            Err(e) => stats.errors.push(format!("{}: {}", path.display(), e)),
                            _ => {}
                        }
                    }
                }
                Err(e) => stats.errors.push(e.to_string()),
            }
        }

        Ok(stats)
    }

    fn supports_language(&self, language: &str) -> bool {
        language.eq_ignore_ascii_case("rust")
    }

    fn get_config(&self) -> &FormatterConfig {
        &self.config
    }

    fn set_config(&mut self, config: FormatterConfig) {
        self.config = config;
    }
}

pub struct PythonFormatter {
    config: FormatterConfig,
}

impl PythonFormatter {
    pub fn new(config: FormatterConfig) -> Self {
        PythonFormatter { config }
    }

    async fn run_black(&self, path: &Path) -> Result<bool> {
        let output = Command::new("black")
            .arg("--line-length")
            .arg(self.config.line_width.to_string())
            .arg(path)
            .output()
            .await?;

        Ok(output.status.success())
    }
}

#[async_trait]
impl CodeFormatter for PythonFormatter {
    async fn format_file(&self, path: &Path) -> Result<bool> {
        if path.extension().map_or(false, |ext| ext == "py") {
            self.run_black(path).await
        } else {
            Ok(false)
        }
    }

    async fn format_directory(&self, path: &Path, recursive: bool) -> Result<FormattingStats> {
        let mut stats = FormattingStats {
            files_processed: 0,
            files_changed: 0,
            total_changes: 0,
            errors: Vec::new(),
        };

        let walker = if recursive {
            Walk::new(path)
        } else {
            Walk::new(path).max_depth(1)
        };

        for entry in walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "py") {
                        stats.files_processed += 1;
                        match self.format_file(path).await {
                            Ok(true) => {
                                stats.files_changed += 1;
                                stats.total_changes += 1;
                            }
                            Err(e) => stats.errors.push(format!("{}: {}", path.display(), e)),
                            _ => {}
                        }
                    }
                }
                Err(e) => stats.errors.push(e.to_string()),
            }
        }

        Ok(stats)
    }

    fn supports_language(&self, language: &str) -> bool {
        language.eq_ignore_ascii_case("python")
    }

    fn get_config(&self) -> &FormatterConfig {
        &self.config
    }

    fn set_config(&mut self, config: FormatterConfig) {
        self.config = config;
    }
}

pub struct JavaScriptFormatter {
    config: FormatterConfig,
}

impl JavaScriptFormatter {
    pub fn new(config: FormatterConfig) -> Self {
        JavaScriptFormatter { config }
    }

    async fn run_prettier(&self, path: &Path) -> Result<bool> {
        let output = Command::new("prettier")
            .arg("--write")
            .arg("--print-width")
            .arg(self.config.line_width.to_string())
            .arg("--tab-width")
            .arg(self.config.indent_size.to_string())
            .arg("--use-tabs")
            .arg(if self.config.indent_style == "tab" { "true" } else { "false" })
            .arg(path)
            .output()
            .await?;

        Ok(output.status.success())
    }
}

#[async_trait]
impl CodeFormatter for JavaScriptFormatter {
    async fn format_file(&self, path: &Path) -> Result<bool> {
        if path.extension().map_or(false, |ext| ext == "js" || ext == "jsx" || ext == "ts" || ext == "tsx") {
            self.run_prettier(path).await
        } else {
            Ok(false)
        }
    }

    async fn format_directory(&self, path: &Path, recursive: bool) -> Result<FormattingStats> {
        let mut stats = FormattingStats {
            files_processed: 0,
            files_changed: 0,
            total_changes: 0,
            errors: Vec::new(),
        };

        let walker = if recursive {
            Walk::new(path)
        } else {
            Walk::new(path).max_depth(1)
        };

        for entry in walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| 
                        ext == "js" || ext == "jsx" || ext == "ts" || ext == "tsx") {
                        stats.files_processed += 1;
                        match self.format_file(path).await {
                            Ok(true) => {
                                stats.files_changed += 1;
                                stats.total_changes += 1;
                            }
                            Err(e) => stats.errors.push(format!("{}: {}", path.display(), e)),
                            _ => {}
                        }
                    }
                }
                Err(e) => stats.errors.push(e.to_string()),
            }
        }

        Ok(stats)
    }

    fn supports_language(&self, language: &str) -> bool {
        matches!(language.to_lowercase().as_str(), 
            "javascript" | "typescript" | "jsx" | "tsx")
    }

    fn get_config(&self) -> &FormatterConfig {
        &self.config
    }

    fn set_config(&mut self, config: FormatterConfig) {
        self.config = config;
    }
}

pub struct FormatterManager {
    formatters: HashMap<String, Box<dyn CodeFormatter>>,
    config: FormatterConfig,
}

impl FormatterManager {
    pub fn new(config: FormatterConfig) -> Self {
        let mut formatters = HashMap::new();
        formatters.insert("rust".to_string(), 
            Box::new(RustFormatter::new(config.clone())) as Box<dyn CodeFormatter>);
        formatters.insert("python".to_string(), 
            Box::new(PythonFormatter::new(config.clone())) as Box<dyn CodeFormatter>);
        formatters.insert("javascript".to_string(), 
            Box::new(JavaScriptFormatter::new(config.clone())) as Box<dyn CodeFormatter>);

        FormatterManager {
            formatters,
            config,
        }
    }

    pub fn get_formatter(&self, language: &str) -> Option<&Box<dyn CodeFormatter>> {
        self.formatters.values().find(|f| f.supports_language(language))
    }

    pub fn get_formatter_mut(&mut self, language: &str) -> Option<&mut Box<dyn CodeFormatter>> {
        self.formatters.values_mut().find(|f| f.supports_language(language))
    }

    pub async fn format_file(&self, path: &Path) -> Result<bool> {
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid file extension"))?;

        let language = match extension {
            "rs" => "rust",
            "py" => "python",
            "js" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            _ => return Ok(false),
        };

        if let Some(formatter) = self.get_formatter(language) {
            formatter.format_file(path).await
        } else {
            Ok(false)
        }
    }

    pub async fn format_directory(&self, path: &Path, recursive: bool) -> Result<FormattingStats> {
        let mut total_stats = FormattingStats {
            files_processed: 0,
            files_changed: 0,
            total_changes: 0,
            errors: Vec::new(),
        };

        for formatter in self.formatters.values() {
            let stats = formatter.format_directory(path, recursive).await?;
            total_stats.files_processed += stats.files_processed;
            total_stats.files_changed += stats.files_changed;
            total_stats.total_changes += stats.total_changes;
            total_stats.errors.extend(stats.errors);
        }

        Ok(total_stats)
    }

    pub fn update_config(&mut self, config: FormatterConfig) {
        self.config = config.clone();
        for formatter in self.formatters.values_mut() {
            formatter.set_config(config.clone());
        }
    }
}
