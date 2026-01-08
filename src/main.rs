use std::fmt::Write as _;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser};
use dark_light::Mode as DarkLightMode;
use eyre::{Result, eyre};
use syntastica::Processor;
use syntastica::language_set::SupportedLanguage;
use syntastica::renderer::{Renderer, TerminalRenderer};
use syntastica::theme::ResolvedTheme;
use syntastica_parsers::{Lang, LanguageSetImpl};
use tft::try_detect;

#[derive(Parser, Debug)]
#[command(
  name = "scat",
  version,
  about = "cat with syntax highlighting",
  long_about = "A modern replacement for cat with syntax highlighting powered by tree-sitter.\n\
                Automatically detects file types and applies appropriate syntax highlighting.\n\
                Supports 100+ programming languages and multiple color themes.",
  after_help = "EXAMPLES:\n    \
    scat main.rs                    Display a file with syntax highlighting\n    \
    scat -n config.toml             Show file with line numbers\n    \
    scat --language rust file.txt   Force Rust syntax highlighting\n    \
    scat --theme dracula main.js    Use Dracula color theme\n    \
    cat file.rs | scat              Read from stdin\n    \
    scat *.py                       Display multiple files\n\n\
    For available themes, see: https://docs.rs/syntastica-themes/latest/syntastica_themes/\n\n\
    To generate shell completions:\n    \
    scat --completions bash > ~/.local/share/bash-completion/completions/scat"
)]
struct Cli {
  #[arg(
    long,
    value_enum,
    help = "Generate shell completions for the specified shell",
    long_help = "Generate shell completion script for the specified shell.\n\
                 Output the completion script to stdout, which you can then save to the\n\
                 appropriate location for your shell.\n\n\
                 Examples:\n  \
                 scat --completions bash > ~/.local/share/bash-completion/completions/scat\n  \
                 scat --completions zsh > ~/.zsh/completion/_scat\n  \
                 scat --completions fish > ~/.config/fish/completions/scat.fish"
  )]
  completions: Option<clap_complete::Shell>,

  #[arg(
    long,
    value_name = "LANG",
    help = "Force a specific programming language",
    long_help = "Override automatic language detection and force a specific language.\n\
                 Useful when the file extension doesn't match the content or for files\n\
                 without extensions.\n\n\
                 Examples:\n  \
                 scat --language rust config.txt\n  \
                 scat --language json response.log\n\n\
                 For a complete list of supported languages, see:\n\
                 https://docs.rs/syntastica-parsers/latest/syntastica_parsers/"
  )]
  language: Option<String>,

  #[arg(
    long,
    value_name = "THEME",
    default_value = "auto",
    help = "Color theme to use for syntax highlighting",
    long_help = "Specify a color theme for syntax highlighting.\n\n\
                 Use 'auto' (default) to automatically detect light/dark mode:\n  \
                 - Light mode: catppuccin-latte\n  \
                 - Dark mode: catppuccin-mocha\n\n\
                 Popular themes include:\n  \
                 dracula, nord, one-dark, one-light, gruvbox-dark, gruvbox-light,\n  \
                 solarized-dark, solarized-light, tokyo-night, catppuccin-mocha,\n  \
                 catppuccin-latte, catppuccin-frappe, catppuccin-macchiato\n\n\
                 For a complete list of available themes, see:\n\
                 https://docs.rs/syntastica-themes/latest/syntastica_themes/"
  )]
  theme: String,

  #[arg(
    long,
    short = 'n',
    help = "Show line numbers",
    long_help = "Display line numbers at the beginning of each line.\n\
                 Line numbers are right-aligned and separated from the content by two spaces."
  )]
  line_numbers: bool,

  #[arg(
    long,
    help = "Generate man page",
    long_help = "Generate a manual page in roff format and print to stdout.\n\
                 You can save this to a file and install it in your man path.\n\n\
                 Example:\n  \
                 scat --man-page > scat.1\n  \
                 sudo cp scat.1 /usr/local/share/man/man1/"
  )]
  man_page: bool,

  #[arg(
    value_name = "FILE",
    help = "Files to display (use '-' or omit for stdin)",
    long_help = "One or more files to display with syntax highlighting.\n\
                 If no files are specified, or if '-' is given, reads from stdin.\n\n\
                 Examples:\n  \
                 scat main.rs lib.rs\n  \
                 cat file.rs | scat\n  \
                 echo 'code' | scat --language rust"
  )]
  files: Vec<PathBuf>,
}

fn main() -> Result<()> {
  let cli = Cli::parse();
  if let Some(shell) = cli.completions {
    write_completions(shell)?;
    return Ok(());
  }
  if cli.man_page {
    write_man_page()?;
    return Ok(());
  }
  let use_color = io::stdout().is_terminal();
  let language_set = LanguageSetImpl::new();
  let theme = resolve_theme(&cli.theme);
  let line_numbers = cli.line_numbers;
  let language_override = match cli.language.as_deref() {
    Some(name) => Some(
      resolve_language_override(name, &language_set)
        .ok_or_else(|| eyre!("Unsupported language: {name}"))?,
    ),
    None => None,
  };

  let files = if cli.files.is_empty() {
    vec![PathBuf::from("-")]
  } else {
    cli.files
  };

  let mut processor = Processor::new(&language_set);
  let mut renderer = TerminalRenderer::new(None);
  let mut stdout = io::stdout().lock();
  let mut stdin = io::stdin();
  let mut had_error = false;
  let mut stdin_consumed = false;

  for path in files {
    if path == Path::new("-") {
      if stdin_consumed {
        continue;
      }
      stdin_consumed = true;
      let mut buf = Vec::new();
      if let Err(err) = stdin.read_to_end(&mut buf) {
        eprintln!("scat: -: {err}");
        had_error = true;
        continue;
      }
        emit_bytes(
          &mut stdout,
          buf,
          None,
          language_override.as_ref(),
          line_numbers,
          use_color,
          &language_set,
          &mut processor,
          &mut renderer,
          &theme,
      )?;
      continue;
    }

    match fs::read(&path) {
      Ok(buf) => {
        emit_bytes(
          &mut stdout,
          buf,
          Some(&path),
          language_override.as_ref(),
          line_numbers,
          use_color,
          &language_set,
          &mut processor,
          &mut renderer,
          &theme,
        )?;
      }
      Err(err) => {
        eprintln!("scat: {}: {err}", path.display());
        had_error = true;
      }
    }
  }

  stdout.flush()?;
  if had_error {
    std::process::exit(1);
  }
  Ok(())
}

fn write_completions(shell: clap_complete::Shell) -> Result<()> {
  let mut cmd = Cli::command();
  clap_complete::generate(shell, &mut cmd, "scat", &mut io::stdout());
  Ok(())
}

fn write_man_page() -> Result<()> {
  let cmd = Cli::command();
  let man = clap_mangen::Man::new(cmd);
  man.render(&mut io::stdout())?;
  Ok(())
}

fn emit_bytes(
  stdout: &mut impl Write,
  bytes: Vec<u8>,
  path: Option<&Path>,
  language_override: Option<&Lang>,
  line_numbers: bool,
  use_color: bool,
  language_set: &LanguageSetImpl,
  processor: &mut Processor<LanguageSetImpl>,
  renderer: &mut TerminalRenderer,
  theme: &ResolvedTheme,
) -> Result<()> {
  if !use_color && !line_numbers {
    stdout.write_all(&bytes)?;
    return Ok(());
  }

  if use_color {
    match String::from_utf8(bytes) {
      Ok(text) => {
        let language = language_override
          .cloned()
          .or_else(|| detect_language(path, &text, language_set));
        let rendered = render_text(&text, language, line_numbers, processor, renderer, theme);
        stdout.write_all(rendered.as_bytes())?;
        return Ok(());
      }
      Err(err) => {
        let bytes = err.into_bytes();
        if line_numbers {
          write_numbered_bytes(stdout, &bytes)?;
        } else {
          stdout.write_all(&bytes)?;
        }
        return Ok(());
      }
    }
  }

  write_numbered_bytes(stdout, &bytes)?;
  Ok(())
}

fn detect_language(
  path: Option<&Path>,
  content: &str,
  language_set: &LanguageSetImpl,
) -> Option<Lang> {
  let detect_path = path.unwrap_or_else(|| Path::new("stdin"));
  let file_type = try_detect(detect_path, content)?;
  <Lang as SupportedLanguage<'_, LanguageSetImpl>>::for_file_type(file_type, language_set)
}

fn resolve_language_override(name: &str, language_set: &LanguageSetImpl) -> Option<Lang> {
  <Lang as SupportedLanguage<'_, LanguageSetImpl>>::for_name(name, language_set)
    .ok()
    .or_else(|| <Lang as SupportedLanguage<'_, LanguageSetImpl>>::for_injection(name, language_set))
}

fn render_text(
  text: &str,
  language: Option<Lang>,
  line_numbers: bool,
  processor: &mut Processor<LanguageSetImpl>,
  renderer: &mut TerminalRenderer,
  theme: &ResolvedTheme,
) -> String {
  let Some(language) = language else {
    return if line_numbers {
      number_plain_text(text)
    } else {
      text.to_string()
    };
  };

  match processor.process(text, language) {
    Ok(highlights) => {
      if line_numbers {
        render_highlights_with_numbers(&highlights, renderer, theme)
      } else {
        syntastica::render(&highlights, renderer, theme.clone())
      }
    }
    Err(_) => {
      if line_numbers {
        number_plain_text(text)
      } else {
        text.to_string()
      }
    }
  }
}

fn resolve_theme(theme: &str) -> ResolvedTheme {
  let override_name = theme.trim();
  if !override_name.is_empty() && override_name != "auto" {
    if let Some(theme) = syntastica_themes::from_str(override_name) {
      return theme;
    }
  }

  match dark_light::detect() {
    Ok(DarkLightMode::Light) => syntastica_themes::catppuccin::latte(),
    Ok(DarkLightMode::Dark) => syntastica_themes::catppuccin::mocha(),
    Ok(DarkLightMode::Unspecified) => syntastica_themes::catppuccin::mocha(),
    Err(_) => syntastica_themes::catppuccin::mocha(),
  }
}

fn render_highlights_with_numbers(
  highlights: &syntastica::Highlights<'_>,
  renderer: &mut TerminalRenderer,
  theme: &ResolvedTheme,
) -> String {
  if highlights.is_empty() {
    return String::new();
  }

  let width = line_number_width(highlights.len());
  let last_line = highlights.len().saturating_sub(1);
  let mut out = renderer.head().into_owned();

  for (index, line) in highlights.iter().enumerate() {
    let line_no = index + 1;
    let prefix = format!("{:>width$}  ", line_no, width = width);
    let escaped = renderer.escape(&prefix);
    out += &renderer.unstyled(&escaped);

    for (text, style) in line {
      let escaped = renderer.escape(text);
      match style.and_then(|key| theme.find_style(key)) {
        Some(style) => out += &renderer.styled(&escaped, style),
        None => out += &renderer.unstyled(&escaped),
      }
    }

    if index != last_line {
      out += &renderer.newline();
    }
  }

  out + &renderer.tail()
}

fn number_plain_text(text: &str) -> String {
  let line_count = count_lines_bytes(text.as_bytes());
  if line_count == 0 {
    return String::new();
  }

  let width = line_number_width(line_count);
  let mut out = String::new();
  let mut line_no = 1;
  for chunk in text.split_inclusive('\n') {
    let _ = write!(out, "{:>width$}  ", line_no, width = width);
    out.push_str(chunk);
    line_no += 1;
  }
  out
}

fn write_numbered_bytes(stdout: &mut impl Write, bytes: &[u8]) -> Result<()> {
  let line_count = count_lines_bytes(bytes);
  if line_count == 0 {
    return Ok(());
  }

  let width = line_number_width(line_count);
  let mut line_no = 1;
  write_prefix(stdout, line_no, width)?;
  for (index, byte) in bytes.iter().enumerate() {
    stdout.write_all(&[*byte])?;
    if *byte == b'\n' && index + 1 < bytes.len() {
      line_no += 1;
      write_prefix(stdout, line_no, width)?;
    }
  }
  Ok(())
}

fn write_prefix(stdout: &mut impl Write, line_no: usize, width: usize) -> Result<()> {
  write!(stdout, "{:>width$}  ", line_no, width = width)?;
  Ok(())
}

fn count_lines_bytes(bytes: &[u8]) -> usize {
  if bytes.is_empty() {
    return 0;
  }
  let count = bytes.iter().filter(|byte| **byte == b'\n').count();
  if bytes.last() == Some(&b'\n') {
    count
  } else {
    count + 1
  }
}

fn line_number_width(line_count: usize) -> usize {
  let width = line_count.to_string().len();
  if width == 0 { 1 } else { width }
}
