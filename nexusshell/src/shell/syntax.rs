use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;
use lazy_static::lazy_static;

lazy_static! {
    static ref SYNTAX_SET: SyntaxSet = SyntaxSet::load_defaults_newlines();
    static ref THEME_SET: ThemeSet = ThemeSet::load_defaults();
}

pub struct SyntaxHighlighter {
    syntax_set: &'static SyntaxSet,
    theme_set: &'static ThemeSet,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        SyntaxHighlighter {
            syntax_set: &SYNTAX_SET,
            theme_set: &THEME_SET,
        }
    }

    pub fn highlight_command(&self, input: &str) -> String {
        // Use the shell script syntax for command highlighting
        let syntax = self.syntax_set.find_syntax_by_extension("sh")
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        
        let mut highlighter = HighlightLines::new(syntax, &self.theme_set.themes["base16-ocean.dark"]);
        let ranges: Vec<(Style, &str)> = highlighter.highlight_line(input, self.syntax_set).unwrap();
        
        as_24_bit_terminal_escaped(&ranges[..], false)
    }

    pub fn highlight_file(&self, content: &str, extension: &str) -> String {
        let syntax = self.syntax_set.find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        
        let mut highlighter = HighlightLines::new(syntax, &self.theme_set.themes["base16-ocean.dark"]);
        let mut output = String::new();

        for line in content.lines() {
            let ranges: Vec<(Style, &str)> = highlighter.highlight_line(line, self.syntax_set).unwrap();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            output.push_str(&escaped);
            output.push('\n');
        }

        output
    }

    pub fn highlight_help(&self, content: &str) -> String {
        let syntax = self.syntax_set.find_syntax_by_extension("md")
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        
        let mut highlighter = HighlightLines::new(syntax, &self.theme_set.themes["base16-ocean.dark"]);
        let ranges: Vec<(Style, &str)> = highlighter.highlight_line(content, self.syntax_set).unwrap();
        
        as_24_bit_terminal_escaped(&ranges[..], false)
    }
}
