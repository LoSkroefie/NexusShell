use async_trait::async_trait;
use super::super::{Command, Environment, Plugin};
use std::fs;
use std::path::Path;
use tokio::fs as async_fs;

pub struct FileOperationsPlugin;

#[async_trait]
impl Plugin for FileOperationsPlugin {
    fn name(&self) -> &str {
        "fileops"
    }

    fn description(&self) -> &str {
        "Advanced file operations plugin"
    }

    async fn execute(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        match command.name.as_str() {
            "ls" => self.list_directory(command, env).await,
            "cp" => self.copy(command, env).await,
            "mv" => self.move_file(command, env).await,
            "rm" => self.remove(command, env).await,
            "mkdir" => self.make_directory(command, env).await,
            "touch" => self.touch(command, env).await,
            "cat" => self.cat(command, env).await,
            _ => Err(anyhow::anyhow!("Unknown file operation command")),
        }
    }
}

impl FileOperationsPlugin {
    async fn list_directory(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        let path = if command.args.is_empty() {
            env.get_current_dir()
        } else {
            env.expand_path(&command.args[0])
        };

        let mut entries = Vec::new();
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        let mut read_dir = async_fs::read_dir(&path).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let metadata = entry.metadata().await?;
            let name = entry.file_name().to_string_lossy().to_string();
            
            if metadata.is_dir() {
                dirs.push(format!("\x1b[1;34m{}/\x1b[0m", name));
            } else {
                let size = metadata.len();
                let size_str = if size < 1024 {
                    format!("{}B", size)
                } else if size < 1024 * 1024 {
                    format!("{:.1}K", size as f64 / 1024.0)
                } else if size < 1024 * 1024 * 1024 {
                    format!("{:.1}M", size as f64 / (1024.0 * 1024.0))
                } else {
                    format!("{:.1}G", size as f64 / (1024.0 * 1024.0 * 1024.0))
                };
                files.push(format!("{:<20} {}", name, size_str));
            }
        }

        dirs.sort();
        files.sort();
        entries.extend(dirs);
        entries.extend(files);

        Ok(entries.join("\n"))
    }

    async fn copy(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        if command.args.len() != 2 {
            return Err(anyhow::anyhow!("Usage: cp <source> <destination>"));
        }

        let source = env.expand_path(&command.args[0]);
        let destination = env.expand_path(&command.args[1]);

        if source.is_dir() {
            copy_dir_all(&source, &destination)?;
        } else {
            async_fs::copy(&source, &destination).await?;
        }

        Ok(format!("Copied {} to {}", 
            source.to_string_lossy(),
            destination.to_string_lossy()))
    }

    async fn move_file(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        if command.args.len() != 2 {
            return Err(anyhow::anyhow!("Usage: mv <source> <destination>"));
        }

        let source = env.expand_path(&command.args[0]);
        let destination = env.expand_path(&command.args[1]);

        async_fs::rename(&source, &destination).await?;

        Ok(format!("Moved {} to {}", 
            source.to_string_lossy(),
            destination.to_string_lossy()))
    }

    async fn remove(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: rm <path> [-r]"));
        }

        let path = env.expand_path(&command.args[0]);
        let recursive = command.flags.contains_key("r") || command.flags.contains_key("recursive");

        if path.is_dir() {
            if recursive {
                async_fs::remove_dir_all(&path).await?;
            } else {
                async_fs::remove_dir(&path).await?;
            }
        } else {
            async_fs::remove_file(&path).await?;
        }

        Ok(format!("Removed {}", path.to_string_lossy()))
    }

    async fn make_directory(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: mkdir <directory>"));
        }

        let path = env.expand_path(&command.args[0]);
        async_fs::create_dir_all(&path).await?;

        Ok(format!("Created directory {}", path.to_string_lossy()))
    }

    async fn touch(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: touch <file>"));
        }

        let path = env.expand_path(&command.args[0]);
        async_fs::File::create(&path).await?;

        Ok(format!("Created file {}", path.to_string_lossy()))
    }

    async fn cat(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        if command.args.is_empty() {
            return Err(anyhow::anyhow!("Usage: cat <file>"));
        }

        let path = env.expand_path(&command.args[0]);
        let content = async_fs::read_to_string(&path).await?;

        Ok(content)
    }
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
