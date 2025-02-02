use colored::*;

pub struct HelpSystem;

impl HelpSystem {
    pub fn new() -> Self {
        HelpSystem
    }

    pub fn get_help(&self, command: Option<&str>) -> String {
        match command {
            None => self.general_help(),
            Some(cmd) => self.command_help(cmd),
        }
    }

    fn general_help(&self) -> String {
        let mut help = String::new();
        help.push_str(&format!("{}\n", "NexusShell Commands".bright_green()));
        help.push_str(&format!("{}\n\n", "=================".bright_green()));

        help.push_str(&format!("{}\n", "File Operations:".yellow()));
        help.push_str("  ls      - List directory contents\n");
        help.push_str("  cd      - Change directory\n");
        help.push_str("  pwd     - Print working directory\n");
        help.push_str("  cp      - Copy files or directories\n");
        help.push_str("  mv      - Move files or directories\n");
        help.push_str("  rm      - Remove files or directories\n");
        help.push_str("  mkdir   - Create directory\n");
        help.push_str("  touch   - Create empty file\n");
        help.push_str("  cat     - Display file contents\n\n");

        help.push_str(&format!("{}\n", "Process Management:".yellow()));
        help.push_str("  ps      - List processes\n");
        help.push_str("  kill    - Terminate process\n");
        help.push_str("  top     - Show system resources\n");
        help.push_str("  bg      - Run process in background\n");
        help.push_str("  fg      - Bring process to foreground\n\n");

        help.push_str(&format!("{}\n", "Network Operations:".yellow()));
        help.push_str("  ping    - Test network connectivity\n");
        help.push_str("  curl    - Transfer data from/to server\n");
        help.push_str("  wget    - Download files\n");
        help.push_str("  netstat - Network statistics\n\n");

        help.push_str(&format!("{}\n", "Git Commands:".yellow()));
        help.push_str("  git status   - Show working tree status\n");
        help.push_str("  git log      - Show commit logs\n");
        help.push_str("  git diff     - Show changes\n");
        help.push_str("  git branch   - List branches\n\n");

        help.push_str(&format!("{}\n", "Shell Control:".yellow()));
        help.push_str("  help    - Show this help\n");
        help.push_str("  clear   - Clear screen\n");
        help.push_str("  exit    - Exit shell\n");
        help.push_str("  history - Show command history\n\n");

        help.push_str(&format!("{}\n", "For detailed help on any command, type:".bright_blue()));
        help.push_str(&format!("{}\n", "  help <command>".bright_blue()));

        help
    }

    fn command_help(&self, command: &str) -> String {
        match command {
            "ls" => format!("{}\n{}\n\n{}\n  ls              List files in current directory\n  ls <dir>         List files in specified directory\n  ls -l            List files in long format\n  ls -a            List all files including hidden\n  ls -la           Combine -l and -a options",
                "ls - List directory contents".bright_green(),
                "=========================".bright_green(),
                "Usage:".yellow()),

            "cd" => format!("{}\n{}\n\n{}\n  cd              Change to home directory\n  cd <dir>         Change to specified directory\n  cd ..           Move up one directory\n  cd -            Change to previous directory",
                "cd - Change directory".bright_green(),
                "====================".bright_green(),
                "Usage:".yellow()),

            "git" => format!("{}\n{}\n\n{}\n  git status      Show working tree status\n  git log         Show commit logs\n  git diff        Show changes\n  git branch      List branches\n  git checkout    Switch branches\n  git commit      Record changes\n  git push        Update remote refs",
                "git - Version control operations".bright_green(),
                "=============================".bright_green(),
                "Usage:".yellow()),

            "ps" => format!("{}\n{}\n\n{}\n  ps              List processes\n  ps -e           List all processes\n  ps -f           Full format listing\n  ps aux          BSD style listing",
                "ps - List processes".bright_green(),
                "==================".bright_green(),
                "Usage:".yellow()),

            _ => format!("No detailed help available for '{}'", command),
        }
    }
}
