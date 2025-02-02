# NexusShell - Next Generation Command Line Interface

NexusShell is a powerful, modern command-line interface that combines advanced features, AI capabilities, and a user-friendly experience. It's designed to enhance productivity and streamline development workflows.

## Features

### Core Features
- Modern command-line interface with syntax highlighting and auto-completion
- Plugin system for extensibility
- Customizable themes and configurations
- Cross-platform support (Windows, Linux, macOS)

### Cloud Integration
- AWS, Azure, and GCP support
- Container management (Docker and Kubernetes)
- Cloud resource provisioning and management
- Infrastructure as Code support

### Advanced Shell Features
- Task scheduling and job queue system
- Job management and monitoring
- Dependency tracking
- Parallel execution support

### Development Tools
- Package manager integration (npm, cargo)
- Code formatting support
  - Rust (rustfmt)
  - Python (black)
  - JavaScript/TypeScript (prettier)
- Configuration management
- Build and test automation

### Security Features
- Credential management
  - Secure storage of usernames and passwords
  - Encryption using industry-standard algorithms
- Key management
  - SSH key generation and storage
  - Key import/export capabilities
- Audit logging
  - Comprehensive activity tracking
  - Exportable audit logs

### Scripting Engine
- Rhai scripting language integration
- Script management and versioning
- Custom function support
- Environment variable handling

## Installation

```bash
cargo install nexusshell
```

## Usage

### Package Management
```bash
# NPM commands
nexus dev package npm install express
nexus dev package npm search react
nexus dev package npm list

# Cargo commands
nexus dev package cargo install tokio
nexus dev package cargo search async
nexus dev package cargo list
```

### Code Formatting
```bash
# Format a single file
nexus dev format file path/to/file.rs

# Format a directory
nexus dev format dir src/ --recursive

# Configure formatter
nexus dev config formatter --indent-style space --indent-size 4
```

### Security
```bash
# Credential management
nexus security credential add myapp username password
nexus security credential get myapp
nexus security credential list

# Key management
nexus security key generate mykey
nexus security key export mykey ./mykey.pem
nexus security key import imported ./key.pem

# Audit logging
nexus security audit list
nexus security audit export ./audit.log
```

### Task Scheduling
```bash
# Schedule a task
nexus scheduler add mytask --command "echo hello" --schedule "0 * * * *"

# List tasks
nexus scheduler list

# Monitor task execution
nexus scheduler status mytask
```

## Configuration

NexusShell can be configured through the `config.toml` file in the user's home directory:

```toml
[formatter]
indent_style = "space"
indent_size = 4
line_width = 100

[package_manager]
default_registry = "https://registry.npmjs.org"
cache_dir = "~/.nexusshell/cache"
max_concurrent_downloads = 5

[security]
credential_store = "~/.nexusshell/credentials"
key_store = "~/.nexusshell/keys"
audit_log = "~/.nexusshell/audit.log"
```

## Development

### Building from Source
```bash
git clone https://github.com/yourusername/nexusshell.git
cd nexusshell
cargo build --release
```

### Running Tests
```bash
cargo test
```

### Contributing
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests
5. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- The Rust community for their excellent crates and tools
- Contributors who have helped shape this project
- Users who provide valuable feedback and suggestions
