mod engine;

use async_trait::async_trait;
use super::super::{Command, Environment, Plugin};
use anyhow::Result;
use engine::{ScriptEngine, Script};
use colored::*;
use std::path::PathBuf;
use tokio::fs;

pub struct ScriptingPlugin {
    engine: ScriptEngine,
}

impl ScriptingPlugin {
    pub async fn new() -> Result<Self> {
        let mut storage_path = dirs::home_dir().unwrap_or_default();
        storage_path.push(".nexusshell");
        storage_path.push("scripts");

        let engine = ScriptEngine::new(storage_path).await?;
        Ok(ScriptingPlugin { engine })
    }

    async fn create_script(&self, args: &[String]) -> Result<String> {
        if args.len() < 4 {
            return Ok("Usage: script create <name> <description> <file_path> [tags...]".to_string());
        }

        let name = args[1].clone();
        let description = args[2].clone();
        let file_path = PathBuf::from(&args[3]);
        let tags = args[4..].to_vec();

        let content = fs::read_to_string(file_path).await?;
        
        // Validate script before creating
        self.engine.validate_script(&content).await?;

        let script_id = self.engine.create_script(
            name,
            description,
            content,
            std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
            tags,
            Vec::new(),
        ).await?;

        Ok(format!("Created script with ID: {}", script_id))
    }

    async fn update_script(&self, args: &[String]) -> Result<String> {
        if args.len() < 3 {
            return Ok("Usage: script update <id> [--name <name>] [--desc <description>] [--file <path>] [--tags <tags>]".to_string());
        }

        let id = args[1].clone();
        let mut name = None;
        let mut description = None;
        let mut content = None;
        let mut tags = None;

        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--name" => {
                    if i + 1 < args.len() {
                        name = Some(args[i + 1].clone());
                        i += 2;
                    }
                }
                "--desc" => {
                    if i + 1 < args.len() {
                        description = Some(args[i + 1].clone());
                        i += 2;
                    }
                }
                "--file" => {
                    if i + 1 < args.len() {
                        let file_path = PathBuf::from(&args[i + 1]);
                        content = Some(fs::read_to_string(file_path).await?);
                        i += 2;
                    }
                }
                "--tags" => {
                    if i + 1 < args.len() {
                        tags = Some(args[i + 1].split(',').map(String::from).collect());
                        i += 2;
                    }
                }
                _ => i += 1,
            }
        }

        if content.is_some() {
            self.engine.validate_script(content.as_ref().unwrap()).await?;
        }

        self.engine.update_script(
            id.clone(),
            name,
            description,
            content,
            tags,
            None,
        ).await?;

        Ok(format!("Updated script {}", id))
    }

    async fn delete_script(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: script delete <id>".to_string());
        }

        let id = &args[1];
        self.engine.delete_script(id).await?;
        Ok(format!("Deleted script {}", id))
    }

    async fn list_scripts(&self, args: &[String]) -> Result<String> {
        let tag = args.get(1);
        let scripts = self.engine.list_scripts(tag.map(|s| s.as_str())).await;

        if scripts.is_empty() {
            return Ok("No scripts found".to_string());
        }

        let mut output = String::new();
        output.push_str(&format!("{:<36} {:<20} {:<40} {:<20}\n",
            "ID", "NAME", "DESCRIPTION", "TAGS"));

        for script in scripts {
            output.push_str(&format!("{:<36} {:<20} {:<40} {:<20}\n",
                script.id,
                script.name,
                if script.description.len() > 37 {
                    format!("{}...", &script.description[..37])
                } else {
                    script.description
                },
                script.tags.join(", ")));
        }

        Ok(output)
    }

    async fn show_script(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: script show <id>".to_string());
        }

        let id = &args[1];
        if let Some(script) = self.engine.get_script(id).await {
            let mut output = String::new();
            output.push_str(&format!("Script Details for {}\n", script.id.bright_green()));
            output.push_str(&format!("Name: {}\n", script.name));
            output.push_str(&format!("Description: {}\n", script.description));
            output.push_str(&format!("Author: {}\n", script.author));
            output.push_str(&format!("Created: {}\n", script.created_at));
            output.push_str(&format!("Updated: {}\n", script.updated_at));
            output.push_str(&format!("Tags: {}\n", script.tags.join(", ")));
            
            if !script.dependencies.is_empty() {
                output.push_str("\nDependencies:\n");
                let deps = self.engine.get_script_dependencies(&script.id).await?;
                for dep in deps {
                    output.push_str(&format!("  - {} ({})\n", dep.name, dep.id));
                }
            }

            output.push_str("\nContent:\n");
            output.push_str("```rhai\n");
            output.push_str(&script.content);
            output.push_str("\n```\n");

            Ok(output)
        } else {
            Ok(format!("Script {} not found", id))
        }
    }

    async fn run_script(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: script run <id> [args...]".to_string());
        }

        let id = &args[1];
        let script_args = args[2..].to_vec();

        let result = self.engine.execute_script(id, &script_args).await?;
        Ok(format!("Script result: {:?}", result))
    }

    async fn search_scripts(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: script search <query>".to_string());
        }

        let query = &args[1];
        let scripts = self.engine.search_scripts(query).await;

        if scripts.is_empty() {
            return Ok(format!("No scripts found matching '{}'", query));
        }

        let mut output = String::new();
        output.push_str(&format!("Search results for '{}'\n", query));
        output.push_str(&format!("{:<36} {:<20} {:<40} {:<20}\n",
            "ID", "NAME", "DESCRIPTION", "TAGS"));

        for script in scripts {
            output.push_str(&format!("{:<36} {:<20} {:<40} {:<20}\n",
                script.id,
                script.name,
                if script.description.len() > 37 {
                    format!("{}...", &script.description[..37])
                } else {
                    script.description
                },
                script.tags.join(", ")));
        }

        Ok(output)
    }

    async fn validate_script(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Ok("Usage: script validate <file_path>".to_string());
        }

        let file_path = PathBuf::from(&args[1]);
        let content = fs::read_to_string(file_path).await?;

        match self.engine.validate_script(&content).await {
            Ok(_) => Ok("Script is valid".to_string()),
            Err(e) => Ok(format!("Script validation failed: {}", e)),
        }
    }
}

#[async_trait]
impl Plugin for ScriptingPlugin {
    fn name(&self) -> &str {
        "script"
    }

    fn description(&self) -> &str {
        "Script management and execution"
    }

    async fn execute(&self, command: &Command, _env: &Environment) -> Result<String> {
        match command.args.first().map(|s| s.as_str()) {
            Some("create") => self.create_script(&command.args).await,
            Some("update") => self.update_script(&command.args).await,
            Some("delete") => self.delete_script(&command.args).await,
            Some("list") => self.list_scripts(&command.args).await,
            Some("show") => self.show_script(&command.args).await,
            Some("run") => self.run_script(&command.args).await,
            Some("search") => self.search_scripts(&command.args).await,
            Some("validate") => self.validate_script(&command.args).await,
            _ => Ok("Available commands: create, update, delete, list, show, run, search, validate".to_string()),
        }
    }
}
