# Contributing to NexusShell

We love your input! We want to make contributing to NexusShell as easy and transparent as possible, whether it's:

- Reporting a bug
- Discussing the current state of the code
- Submitting a fix
- Proposing new features
- Becoming a maintainer

## Development Environment Setup

1. Install Prerequisites:
   - Rust (1.70.0 or higher)
   - Cargo package manager
   - Git

2. Clone and Build:
   ```bash
   git clone https://github.com/yourusername/nexusshell.git
   cd nexusshell
   cargo build
   ```

## Development Process
We use GitHub to host code, to track issues and feature requests, as well as accept pull requests.

1. Fork the repo and create your branch from `main`
2. If you've added code that should be tested, add tests
3. If you've changed APIs, update the documentation
4. Ensure the test suite passes
5. Make sure your code lints
6. Issue that pull request!

## Project Structure

The project follows a modular architecture:

- `src/main.rs`: Entry point
- `src/shell/`: Core shell implementation
  - `mod.rs`: Shell module and trait definitions
  - `command.rs`: Command parsing and representation
  - `executor.rs`: Command execution logic
  - `plugins/`: Plugin implementations
  - `completion.rs`: Auto-completion system
  - `syntax.rs`: Syntax highlighting
  - `help.rs`: Help system

## Creating Plugins

1. Create a new file in `src/shell/plugins/`
2. Implement the Plugin trait:
   ```rust
   #[async_trait]
   pub trait Plugin: Send + Sync {
       fn name(&self) -> &str;
       fn description(&self) -> &str;
       async fn execute(&self, command: &Command, env: &Environment) -> anyhow::Result<String>;
   }
   ```
3. Register your plugin in `plugins/mod.rs`

Example plugin:
```rust
pub struct MyPlugin;

#[async_trait]
impl Plugin for MyPlugin {
    fn name(&self) -> &str {
        "myplugin"
    }

    fn description(&self) -> &str {
        "My awesome plugin"
    }

    async fn execute(&self, command: &Command, env: &Environment) -> anyhow::Result<String> {
        // Implementation
    }
}

## Pull Request Process
1. Update the README.md with details of changes to the interface
2. Update the documentation with any new features or changes
3. The PR will be merged once you have the sign-off of two other developers

## Code Style

1. Follow Rust style guidelines
2. Use meaningful variable names
3. Add comments for complex logic
4. Write unit tests for new features
5. Update documentation

## Testing

1. Unit Tests:
   ```bash
   cargo test
   ```

2. Integration Tests:
   ```bash
   cargo test --test '*'
   ```

3. Manual Testing:
   ```bash
   cargo run
   ```

## Report Bugs Using GitHub's [Issue Tracker](https://github.com/LoSkroefie/NexusShell/issues)
We use GitHub issues to track public bugs. Report a bug by [opening a new issue](https://github.com/LoSkroefie/NexusShell/issues/new).

## Write Bug Reports With Detail, Background, and Sample Code

**Great Bug Reports** tend to have:

- A quick summary and/or background
- Steps to reproduce
  - Be specific!
  - Give sample code if you can
- What you expected would happen
- What actually happens
- Notes (possibly including why you think this might be happening, or stuff you tried that didn't work)

## Use a Consistent Coding Style

* Use 4 spaces for indentation rather than tabs
* Run `cargo fmt` before committing
* Keep line length under 100 characters
* Write documentation for public APIs

## Documentation

When adding features:
1. Update README.md
2. Add inline documentation
3. Update help system
4. Create examples

## Building for Release

```bash
cargo build --release
```

## License
By contributing, you agree that your contributions will be licensed under the MIT License.
