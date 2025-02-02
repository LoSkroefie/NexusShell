mod command;
mod history;
mod parser;
mod plugins;
mod executor;
mod environment;
mod completion;
mod syntax;
mod help;

pub use command::Command;
pub use history::History;
pub use parser::Parser;
pub use plugins::PluginManager;
pub use executor::Executor;
pub use environment::Environment;
pub use completion::Completer;
pub use syntax::SyntaxHighlighter;
pub use help::HelpSystem;

use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::PathBuf;

pub struct Shell {
    history: Arc<Mutex<History>>,
    plugin_manager: Arc<PluginManager>,
    parser: Parser,
    executor: Executor,
    environment: Environment,
    completer: Completer,
    syntax_highlighter: SyntaxHighlighter,
    help_system: HelpSystem,
}

impl Shell {
    pub fn new() -> Self {
        let environment = Environment::new();
        let history = Arc::new(Mutex::new(History::new()));
        let plugin_manager = Arc::new(PluginManager::new());
        let parser = Parser::new();
        let executor = Executor::new(plugin_manager.clone());
        let completer = Completer::new(environment.clone());
        let syntax_highlighter = SyntaxHighlighter::new();
        let help_system = HelpSystem::new();

        Shell {
            history,
            plugin_manager,
            parser,
            executor,
            environment,
            completer,
            syntax_highlighter,
            help_system,
        }
    }

    pub async fn run_command(&mut self, input: &str) -> anyhow::Result<String> {
        // Highlight the command
        let highlighted_input = self.syntax_highlighter.highlight_command(input);
        println!("{}", highlighted_input);

        // Add command to history
        self.history.lock().await.add(input.to_string());

        // Handle help command
        if input.starts_with("help") {
            let args: Vec<&str> = input.split_whitespace().collect();
            return Ok(self.help_system.get_help(args.get(1).copied()));
        }

        // Parse the command
        let command = self.parser.parse(input)?;

        // Check for exit command
        if command.is_exit() {
            std::process::exit(0);
        }

        // Execute the command
        let result = self.executor.execute(&command, &self.environment).await?;

        Ok(result)
    }

    pub async fn get_history(&self) -> Vec<String> {
        self.history.lock().await.get_all()
    }

    pub fn get_current_dir(&self) -> PathBuf {
        self.environment.get_current_dir()
    }

    pub fn change_directory(&mut self, path: PathBuf) -> anyhow::Result<()> {
        self.environment.change_directory(path)
    }

    pub fn complete(&self, line: &str) -> Vec<String> {
        self.completer.complete(line)
    }

    pub fn highlight_file(&self, content: &str, extension: &str) -> String {
        self.syntax_highlighter.highlight_file(content, extension)
    }

    pub fn highlight_help(&self, content: &str) -> String {
        self.syntax_highlighter.highlight_help(content)
    }

    pub fn get_plugin_list(&self) -> Vec<(String, String)> {
        self.plugin_manager.list_plugins()
    }
}
