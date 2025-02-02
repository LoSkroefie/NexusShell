#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use std::path::Path;

    #[tokio::test]
    async fn test_package_manager() {
        let config = PackageManagerConfig::default();
        let npm = NodePackageManager::new(config.clone()).await.unwrap();
        let cargo = CargoPackageManager::new(config).await.unwrap();

        // Test npm package search
        let results = npm.search("express").await.unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|p| p.name == "express"));

        // Test cargo package search
        let results = cargo.search("tokio").await.unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|p| p.name == "tokio"));
    }

    #[tokio::test]
    async fn test_formatter() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main(){println!(\"Hello\");}\n").unwrap();

        let config = FormatterConfig::default();
        let formatter = RustFormatter::new(config);

        // Test formatting
        let formatted = formatter.format_file(&test_file).await.unwrap();
        assert!(formatted);

        // Verify formatting
        let content = fs::read_to_string(&test_file).unwrap();
        assert!(content.contains("fn main() {\n    println!(\"Hello\");\n}\n"));
    }

    #[tokio::test]
    async fn test_formatter_manager() {
        let config = FormatterConfig::default();
        let manager = FormatterManager::new(config);

        assert!(manager.get_formatter("rust").is_some());
        assert!(manager.get_formatter("python").is_some());
        assert!(manager.get_formatter("javascript").is_some());
        assert!(manager.get_formatter("invalid").is_none());
    }

    #[tokio::test]
    async fn test_config() {
        let mut config = FormatterConfig::default();
        config.indent_style = "tab".to_string();
        config.indent_size = 2;
        config.line_width = 80;

        let mut formatter = RustFormatter::new(FormatterConfig::default());
        formatter.set_config(config.clone());

        assert_eq!(formatter.get_config().indent_style, "tab");
        assert_eq!(formatter.get_config().indent_size, 2);
        assert_eq!(formatter.get_config().line_width, 80);
    }
}
