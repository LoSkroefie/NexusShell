mod shell;

use colored::*;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use rustyline::completion::{Completer, Pair};
use rustyline::hint::Hinter;
use rustyline::highlight::Highlighter;
use rustyline::validate::Validator;
use shell::Shell;
use std::borrow::Cow;
use std::path::PathBuf;
use tokio;

struct ShellHelper {
    shell: Shell,
}

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let completions = self.shell.complete(&line[..pos]);
        let pairs: Vec<Pair> = completions
            .into_iter()
            .map(|s| Pair {
                display: s.clone(),
                replacement: s,
            })
            .collect();
        Ok((0, pairs))
    }
}

impl Hinter for ShellHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &rustyline::Context<'_>) -> Option<String> {
        if line.is_empty() || pos < line.len() {
            return None;
        }

        let completions = self.shell.complete(line);
        completions.first().map(|s| s[pos..].to_string().dimmed().to_string())
    }
}

impl Highlighter for ShellHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Owned(line.to_string())
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        Cow::Owned(prompt.to_string())
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(hint.dimmed().to_string())
    }

    fn highlight_candidate<'c>(
        &self,
        candidate: &'c str,
        _completion: rustyline::CompletionType,
    ) -> Cow<'c, str> {
        Cow::Owned(candidate.blue().to_string())
    }

    fn highlight_char(&self, _line: &str, _pos: usize) -> bool {
        false
    }
}

impl Validator for ShellHelper {}

impl rustyline::Helper for ShellHelper {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("{}", "Welcome to NexusShell - Next Generation CLI".bright_green());
    println!("{}", "Type 'help' for available commands or 'exit' to quit\n".bright_blue());

    let mut shell = Shell::new();
    let helper = ShellHelper { shell: Shell::new() };
    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));

    if let Err(err) = rl.load_history("history.txt") {
        println!("No previous history: {}", err);
    }

    loop {
        let prompt = format!(
            "{}:{} {} ",
            "nexus".bright_green(),
            shell.get_current_dir().to_string_lossy().bright_blue(),
            ">".bright_yellow()
        );

        match rl.readline(&prompt) {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                
                match shell.run_command(&line).await {
                    Ok(output) => {
                        if !output.is_empty() {
                            println!("{}", output);
                        }
                    }
                    Err(e) => {
                        eprintln!("{}: {}", "Error".bright_red(), e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("exit");
                break;
            }
            Err(err) => {
                println!("Error: {}", err);
                break;
            }
        }
    }

    rl.save_history("history.txt")?;
    Ok(())
}
