# Command Reference

## Core Commands

### Shell Navigation
- `cd <directory>` - Change directory
- `pwd` - Print working directory
- `ls [options] [path]` - List directory contents
- `history` - Show command history

### File Operations
- `cp <source> <dest>` - Copy files/directories
- `mv <source> <dest>` - Move/rename files
- `rm <path>` - Remove files/directories
- `mkdir <path>` - Create directory

## Development Tools

### Package Management
```bash
# NPM Commands
nexus dev package npm install <package>
nexus dev package npm search <query>
nexus dev package npm list
nexus dev package npm update <package>

# Cargo Commands
nexus dev package cargo install <package>
nexus dev package cargo search <query>
nexus dev package cargo list
nexus dev package cargo update <package>
```

### Code Formatting
```bash
# Format single file
nexus dev format file <path>

# Format directory
nexus dev format dir <path> [--recursive]

# Configure formatter
nexus dev config formatter --indent-style <style> --indent-size <size>
```

## Security Features

### Credential Management
```bash
# Add credential
nexus security credential add <name> <username> <password>

# Get credential
nexus security credential get <name>

# List credentials
nexus security credential list

# Delete credential
nexus security credential delete <name>
```

### Key Management
```bash
# Generate key
nexus security key generate <name>

# Import key
nexus security key import <name> <path>

# Export key
nexus security key export <name> <path>

# List keys
nexus security key list

# Delete key
nexus security key delete <name>
```

### Audit Logging
```bash
# View audit log
nexus security audit list

# Export audit log
nexus security audit export <path>
```

## Cloud Integration

### AWS Commands
```bash
# EC2 instances
nexus cloud aws ec2 list
nexus cloud aws ec2 start <instance-id>
nexus cloud aws ec2 stop <instance-id>

# S3 operations
nexus cloud aws s3 ls <bucket>
nexus cloud aws s3 cp <source> <dest>
nexus cloud aws s3 rm <path>
```

### Azure Commands
```bash
# Virtual machines
nexus cloud azure vm list
nexus cloud azure vm start <name>
nexus cloud azure vm stop <name>

# Storage
nexus cloud azure storage list
nexus cloud azure storage upload <source> <dest>
nexus cloud azure storage download <source> <dest>
```

### GCP Commands
```bash
# Compute Engine
nexus cloud gcp compute list
nexus cloud gcp compute start <instance>
nexus cloud gcp compute stop <instance>

# Storage
nexus cloud gcp storage ls <bucket>
nexus cloud gcp storage cp <source> <dest>
nexus cloud gcp storage rm <path>
```

## Container Management

### Docker Commands
```bash
# Container operations
nexus container docker ps
nexus container docker run <image>
nexus container docker stop <container>
nexus container docker rm <container>

# Image operations
nexus container docker images
nexus container docker pull <image>
nexus container docker build <path>
```

### Kubernetes Commands
```bash
# Pod operations
nexus container k8s get pods
nexus container k8s describe pod <name>
nexus container k8s logs <pod>

# Deployment operations
nexus container k8s get deployments
nexus container k8s scale deployment <name> --replicas <count>
nexus container k8s rollout restart deployment <name>
```

## Task Scheduling

### Job Management
```bash
# Schedule tasks
nexus scheduler add <name> --command "<command>" --schedule "<cron>"
nexus scheduler list
nexus scheduler remove <name>
nexus scheduler status <name>
```

## Script Management
```bash
# Script operations
nexus script create <name>
nexus script edit <name>
nexus script run <name>
nexus script list
nexus script delete <name>
```

## Configuration
```bash
# View configuration
nexus config show

# Set configuration
nexus config set <key> <value>

# Reset configuration
nexus config reset
```

## Help and Documentation
```bash
# Get help
nexus help
nexus help <command>
nexus <command> --help

# Version information
nexus --version
```
