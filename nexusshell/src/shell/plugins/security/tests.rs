#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use std::path::Path;

    #[tokio::test]
    async fn test_credential_management() {
        let plugin = SecurityPlugin::new().await.unwrap();

        // Test adding credential
        let result = plugin.handle_credential(&vec![
            "credential".to_string(),
            "add".to_string(),
            "test-cred".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        ]).await.unwrap();
        assert!(result.contains("successfully"));

        // Test getting credential
        let result = plugin.handle_credential(&vec![
            "credential".to_string(),
            "get".to_string(),
            "test-cred".to_string(),
        ]).await.unwrap();
        assert!(result.contains("testuser"));
        assert!(result.contains("testpass"));

        // Test listing credentials
        let result = plugin.handle_credential(&vec![
            "credential".to_string(),
            "list".to_string(),
        ]).await.unwrap();
        assert!(result.contains("test-cred"));
        assert!(result.contains("testuser"));

        // Test deleting credential
        let result = plugin.handle_credential(&vec![
            "credential".to_string(),
            "delete".to_string(),
            "test-cred".to_string(),
        ]).await.unwrap();
        assert!(result.contains("deleted"));
    }

    #[tokio::test]
    async fn test_key_management() {
        let plugin = SecurityPlugin::new().await.unwrap();
        let temp_dir = tempdir().unwrap();

        // Test generating key
        let result = plugin.handle_key(&vec![
            "key".to_string(),
            "generate".to_string(),
            "test-key".to_string(),
        ]).await.unwrap();
        assert!(result.contains("successfully"));

        // Test listing keys
        let result = plugin.handle_key(&vec![
            "key".to_string(),
            "list".to_string(),
        ]).await.unwrap();
        assert!(result.contains("test-key"));

        // Test exporting key
        let export_path = temp_dir.path().join("test-key.pem");
        let result = plugin.handle_key(&vec![
            "key".to_string(),
            "export".to_string(),
            "test-key".to_string(),
            export_path.to_str().unwrap().to_string(),
        ]).await.unwrap();
        assert!(result.contains("exported"));
        assert!(export_path.exists());

        // Test importing key
        let result = plugin.handle_key(&vec![
            "key".to_string(),
            "import".to_string(),
            "imported-key".to_string(),
            export_path.to_str().unwrap().to_string(),
        ]).await.unwrap();
        assert!(result.contains("imported"));

        // Test deleting key
        let result = plugin.handle_key(&vec![
            "key".to_string(),
            "delete".to_string(),
            "test-key".to_string(),
        ]).await.unwrap();
        assert!(result.contains("deleted"));
    }

    #[tokio::test]
    async fn test_encryption() {
        let plugin = SecurityPlugin::new().await.unwrap();
        let test_data = b"test data";

        // Test encryption
        let (encrypted, salt) = plugin.encrypt(test_data).unwrap();
        assert!(!encrypted.is_empty());
        assert!(!salt.is_empty());

        // Test decryption
        let decrypted = plugin.decrypt(&encrypted, &salt).unwrap();
        assert_eq!(decrypted, test_data);
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let plugin = SecurityPlugin::new().await.unwrap();
        let temp_dir = tempdir().unwrap();

        // Create some audit events
        plugin.handle_credential(&vec![
            "credential".to_string(),
            "add".to_string(),
            "test-cred".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        ]).await.unwrap();

        // Test listing audit log
        let result = plugin.handle_audit(&vec![
            "audit".to_string(),
            "list".to_string(),
        ]).await.unwrap();
        assert!(result.contains("credential_add"));
        assert!(result.contains("test-cred"));

        // Test exporting audit log
        let export_path = temp_dir.path().join("audit.log");
        let result = plugin.handle_audit(&vec![
            "audit".to_string(),
            "export".to_string(),
            export_path.to_str().unwrap().to_string(),
        ]).await.unwrap();
        assert!(result.contains("exported"));
        assert!(export_path.exists());
    }
}
