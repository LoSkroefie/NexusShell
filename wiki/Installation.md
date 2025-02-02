# Installation Guide

## Prerequisites
- Windows, Linux, or macOS
- Internet connection for downloading

## Installation Methods

### 1. Using Pre-built Binaries
1. Download the latest release from our [Releases page](https://github.com/LoSkroefie/NexusShell/releases)
2. Extract the archive
3. Add the binary location to your system's PATH

### 2. Using Package Managers

#### Cargo (Rust)
```bash
cargo install nexusshell
```

#### Homebrew (macOS)
```bash
brew install nexusshell
```

#### APT (Debian/Ubuntu)
```bash
sudo apt-get update
sudo apt-get install nexusshell
```

### 3. Building from Source
1. Clone the source repository:
   ```bash
   git clone https://github.com/LoSkroefie/NexusShell-src.git
   ```
2. Build using Cargo:
   ```bash
   cd NexusShell-src
   cargo build --release
   ```
3. Install:
   ```bash
   cargo install --path .
   ```

## Post-Installation

### Configuration
1. Create configuration directory:
   ```bash
   mkdir -p ~/.nexusshell
   ```
2. Copy default configuration:
   ```bash
   cp /etc/nexusshell/config.toml ~/.nexusshell/
   ```

### Verification
Test the installation:
```bash
nexus --version
nexus help
```

## Troubleshooting

### Common Issues
1. **Command not found**
   - Ensure the binary is in your PATH
   - Restart your terminal

2. **Missing Dependencies**
   - Install required system libraries
   - Update your system

3. **Permission Issues**
   - Check file permissions
   - Use sudo if necessary

### Getting Help
- Visit our [GitHub Issues](https://github.com/LoSkroefie/NexusShell/issues)
- Check the [FAQ](FAQ)
- Join our community forums
