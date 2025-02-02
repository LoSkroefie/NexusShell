use std::collections::HashMap;
use rhai::{Engine, Scope, AST, Dynamic, Map, Array};
use anyhow::Result;
use tokio::fs;
use std::path::PathBuf;
use async_trait::async_trait;
use super::super::super::{Command, Environment};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub author: String,
    pub tags: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScriptEngine {
    engine: Arc<Engine>,
    scripts: Arc<RwLock<HashMap<String, Script>>>,
    storage_path: PathBuf,
}

impl ScriptEngine {
    pub async fn new(storage_path: PathBuf) -> Result<Self> {
        let mut engine = Engine::new();

        // Register core modules
        engine.register_global_module(rhai::packages::StandardPackage::new().as_shared_module());
        engine.register_global_module(rhai::packages::BasicArrayPackage::new().as_shared_module());
        engine.register_global_module(rhai::packages::BasicMapPackage::new().as_shared_module());

        // Custom functions
        engine.register_fn("print", |s: &str| println!("{}", s));
        engine.register_fn("now", || Utc::now());
        engine.register_fn("sleep", |ms: i64| std::thread::sleep(std::time::Duration::from_millis(ms as u64)));

        let engine = Arc::new(engine);
        let scripts = Arc::new(RwLock::new(HashMap::new()));

        let script_engine = ScriptEngine {
            engine,
            scripts,
            storage_path,
        };

        script_engine.load_scripts().await?;
        Ok(script_engine)
    }

    async fn load_scripts(&self) -> Result<()> {
        if !self.storage_path.exists() {
            fs::create_dir_all(&self.storage_path).await?;
            return Ok(());
        }

        let mut scripts = self.scripts.write().await;
        let entries = fs::read_dir(&self.storage_path).await?;
        
        for entry in entries.await {
            let entry = entry?;
            if entry.file_type().await?.is_file() && entry.path().extension().map_or(false, |ext| ext == "json") {
                let content = fs::read_to_string(entry.path()).await?;
                let script: Script = serde_json::from_str(&content)?;
                scripts.insert(script.id.clone(), script);
            }
        }

        Ok(())
    }

    async fn save_script(&self, script: &Script) -> Result<()> {
        fs::create_dir_all(&self.storage_path).await?;
        let path = self.storage_path.join(format!("{}.json", script.id));
        let content = serde_json::to_string_pretty(script)?;
        fs::write(path, content).await?;
        Ok(())
    }

    pub async fn create_script(&self, 
        name: String,
        description: String,
        content: String,
        author: String,
        tags: Vec<String>,
        dependencies: Vec<String>
    ) -> Result<String> {
        // Validate script syntax
        self.engine.compile(&content)?;

        let script = Script {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            content,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            author,
            tags,
            dependencies,
        };

        let mut scripts = self.scripts.write().await;
        scripts.insert(script.id.clone(), script.clone());
        self.save_script(&script).await?;

        Ok(script.id)
    }

    pub async fn update_script(&self,
        id: String,
        name: Option<String>,
        description: Option<String>,
        content: Option<String>,
        tags: Option<Vec<String>>,
        dependencies: Option<Vec<String>>
    ) -> Result<()> {
        let mut scripts = self.scripts.write().await;
        
        if let Some(script) = scripts.get_mut(&id) {
            if let Some(name) = name {
                script.name = name;
            }
            if let Some(description) = description {
                script.description = description;
            }
            if let Some(content) = content {
                // Validate new script content
                self.engine.compile(&content)?;
                script.content = content;
            }
            if let Some(tags) = tags {
                script.tags = tags;
            }
            if let Some(dependencies) = dependencies {
                script.dependencies = dependencies;
            }
            script.updated_at = Utc::now();

            self.save_script(script).await?;
        } else {
            return Err(anyhow::anyhow!("Script not found"));
        }

        Ok(())
    }

    pub async fn delete_script(&self, id: &str) -> Result<()> {
        let mut scripts = self.scripts.write().await;
        if scripts.remove(id).is_some() {
            let path = self.storage_path.join(format!("{}.json", id));
            if path.exists() {
                fs::remove_file(path).await?;
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Script not found"))
        }
    }

    pub async fn get_script(&self, id: &str) -> Option<Script> {
        let scripts = self.scripts.read().await;
        scripts.get(id).cloned()
    }

    pub async fn list_scripts(&self, tag: Option<&str>) -> Vec<Script> {
        let scripts = self.scripts.read().await;
        scripts.values()
            .filter(|script| {
                if let Some(tag) = tag {
                    script.tags.contains(&tag.to_string())
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    pub async fn execute_script(&self, id: &str, args: &[String]) -> Result<Dynamic> {
        let scripts = self.scripts.read().await;
        let script = scripts.get(id).ok_or_else(|| anyhow::anyhow!("Script not found"))?;

        // Create a new scope for this execution
        let mut scope = Scope::new();

        // Add arguments to scope
        let args_array: Array = args.iter().map(|s| Dynamic::from(s.clone())).collect();
        scope.push("args", args_array);

        // Add environment variables
        let mut env_map = Map::new();
        for (key, value) in std::env::vars() {
            env_map.insert(Dynamic::from(key), Dynamic::from(value));
        }
        scope.push("env", env_map);

        // Add utility functions
        scope.push_constant("SCRIPT_ID", script.id.clone());
        scope.push_constant("SCRIPT_NAME", script.name.clone());

        // Compile and run the script
        let ast = self.engine.compile(&script.content)?;
        let result = self.engine.run_ast_with_scope(&mut scope, &ast)?;

        Ok(result)
    }

    pub async fn validate_script(&self, content: &str) -> Result<()> {
        self.engine.compile(content)?;
        Ok(())
    }

    pub fn register_function<F>(&self, name: &str, f: F)
    where
        F: rhai::RegisterFn,
    {
        self.engine.register_fn(name, f);
    }

    pub fn register_type<T>(&self)
    where
        T: rhai::RegisterType,
    {
        self.engine.register_type::<T>();
    }

    pub async fn get_script_dependencies(&self, id: &str) -> Result<Vec<Script>> {
        let scripts = self.scripts.read().await;
        let script = scripts.get(id).ok_or_else(|| anyhow::anyhow!("Script not found"))?;
        
        let mut dependencies = Vec::new();
        for dep_id in &script.dependencies {
            if let Some(dep_script) = scripts.get(dep_id) {
                dependencies.push(dep_script.clone());
            }
        }

        Ok(dependencies)
    }

    pub async fn search_scripts(&self, query: &str) -> Vec<Script> {
        let scripts = self.scripts.read().await;
        scripts.values()
            .filter(|script| {
                script.name.contains(query) ||
                script.description.contains(query) ||
                script.content.contains(query) ||
                script.tags.iter().any(|tag| tag.contains(query))
            })
            .cloned()
            .collect()
    }
}
