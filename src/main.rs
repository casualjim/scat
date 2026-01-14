use std::fmt::Write as _;
use std::fs;
use std::io::{self, Cursor, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use clap::{ArgAction, CommandFactory, Parser, ValueEnum};
use dark_light::Mode as DarkLightMode;
use eyre::{Result, eyre};
use palate::detectors;
use syntastica::Processor;
use syntastica::language_set::SupportedLanguage;
use syntastica::renderer::{Renderer, TerminalRenderer};
use syntastica::theme::ResolvedTheme;
use syntastica_parsers_git::{LANGUAGE_NAMES, Lang, LanguageSetImpl};

const MAX_CONTENT_SIZE_BYTES: usize = 51200;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum ColorWhen {
  Auto,
  Never,
  Always,
}

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
    long = "completions",
    alias = "completion",
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
    short = 'p',
    action = ArgAction::Count,
    help = "Only show plain style, no decorations",
    long_help = "Only show plain style, no decorations. When used twice ('-pp'), it also disables paging."
  )]
  plain: u8,

  #[arg(
    long,
    short = 'l',
    value_name = "LANG",
    help = "Force a specific programming language",
    long_help = "Override automatic language detection and force a specific language.\n\
                 Useful when the file extension doesn't match the content or for files\n\
                 without extensions.\n\n\
                 Examples:\n  \
                 scat --language rust config.txt\n  \
                 scat --language json response.log\n\n\
                 For a complete list of supported languages, see:\n\
                 https://docs.rs/syntastica-parsers-git/latest/syntastica_parsers_git/"
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
    value_name = "THEME",
    help = "Theme for light backgrounds (used with --theme=auto/light)"
  )]
  theme_light: Option<String>,

  #[arg(
    long,
    value_name = "THEME",
    help = "Theme for dark backgrounds (used with --theme=auto/dark)"
  )]
  theme_dark: Option<String>,

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
    value_enum,
    default_value = "auto",
    help = "Specify when to use colored output"
  )]
  color: ColorWhen,

  #[arg(
    long,
    value_name = "name",
    help = "Specify the name to display for a file"
  )]
  file_name: Option<PathBuf>,

  #[arg(long, help = "List supported themes")]
  list_themes: bool,

  #[arg(
    long,
    short = 's',
    help = "Squeeze consecutive empty lines into a single empty line"
  )]
  squeeze_blank: bool,

  #[arg(
    long,
    value_name = "squeeze-limit",
    help = "Set the maximum number of consecutive empty lines"
  )]
  squeeze_limit: Option<usize>,

  #[arg(
    long,
    value_name = "components",
    help = "Configure which style components to display"
  )]
  style: Option<String>,

  #[arg(long, short = 'u', help = "No-op, output is always unbuffered")]
  unbuffered: bool,

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
  if cli.list_themes {
    for theme in syntastica_themes::THEMES {
      println!("{theme}");
    }
    return Ok(());
  }
  let mut use_color = io::stdout().is_terminal();
  match cli.color {
    ColorWhen::Auto => {}
    ColorWhen::Never => use_color = false,
    ColorWhen::Always => use_color = true,
  }
  let language_set = LanguageSetImpl::new();
  let theme = resolve_theme_with_overrides(
    &cli.theme,
    cli.theme_light.as_deref(),
    cli.theme_dark.as_deref(),
  );
  let line_numbers = resolve_line_numbers(&cli);
  let squeeze_limit = cli.squeeze_limit.unwrap_or(1);
  let squeeze_blank = cli.squeeze_blank || cli.squeeze_limit.is_some();
  let language_override = match cli.language.as_deref() {
    Some(name) => Some(
      resolve_language(name, &language_set).ok_or_else(|| eyre!("Unsupported language: {name}"))?,
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
        cli.file_name.as_deref(),
        language_override.as_ref(),
        line_numbers,
        use_color,
        squeeze_blank,
        squeeze_limit,
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
          squeeze_blank,
          squeeze_limit,
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
  squeeze_blank: bool,
  squeeze_limit: usize,
  language_set: &LanguageSetImpl,
  processor: &mut Processor<LanguageSetImpl>,
  renderer: &mut TerminalRenderer,
  theme: &ResolvedTheme,
) -> Result<()> {
  let bytes = if squeeze_blank {
    squeeze_blank_lines_bytes(&bytes, squeeze_limit)
  } else {
    bytes
  };
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
  let name = detect_language_name(path, content)?;
  resolve_language(name.to_ascii_lowercase(), language_set)
}

fn resolve_language(name: impl AsRef<str>, language_set: &LanguageSetImpl) -> Option<Lang> {
  let name = name.as_ref();
  <Lang as SupportedLanguage<'_, LanguageSetImpl>>::for_name(name, language_set)
    .ok()
    .or_else(|| <Lang as SupportedLanguage<'_, LanguageSetImpl>>::for_injection(name, language_set))
    .or_else(|| {
      let canonical = LANGUAGE_NAMES
        .iter()
        .copied()
        .find(|candidate| candidate.eq_ignore_ascii_case(name))?;
      <Lang as SupportedLanguage<'_, LanguageSetImpl>>::for_name(canonical, language_set).ok()
    })
}

fn detect_language_name(path: Option<&Path>, content: &str) -> Option<&'static str> {
  let mut extension: Option<String> = None;
  let mut candidates = Vec::new();

  if let Some(path) = path {
    if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
      if let Some(candidate) = detectors::get_language_from_filename(filename) {
        return Some(candidate);
      }

      extension = detectors::get_extension(filename).map(str::to_string);
      candidates = extension
        .as_deref()
        .map(detectors::get_languages_from_extension)
        .unwrap_or_else(Vec::new);
      if candidates.len() == 1 {
        return Some(candidates[0]);
      }
    }
  }

  let shebang_candidates =
    detectors::get_languages_from_shebang(Cursor::new(content)).unwrap_or_default();
  candidates = filter_candidates(candidates, shebang_candidates);
  if candidates.len() == 1 {
    return Some(candidates[0]);
  }

  let content = truncate_to_char_boundary(content, MAX_CONTENT_SIZE_BYTES);
  candidates = if candidates.len() > 1 {
    if let Some(extension) = extension.as_deref() {
      let heuristic_candidates =
        detectors::get_languages_from_heuristics(extension, &candidates, content);
      filter_candidates(candidates, heuristic_candidates)
    } else {
      candidates
    }
  } else {
    candidates
  };

  match candidates.len() {
    0 => None,
    1 => Some(candidates[0]),
    _ => Some(detectors::classify(content, &candidates)),
  }
}

fn filter_candidates(
  previous_candidates: Vec<&'static str>,
  new_candidates: Vec<&'static str>,
) -> Vec<&'static str> {
  if previous_candidates.is_empty() {
    return new_candidates;
  }

  if new_candidates.is_empty() {
    return previous_candidates;
  }

  let filtered_candidates: Vec<&'static str> = previous_candidates
    .iter()
    .filter(|candidate| new_candidates.contains(candidate))
    .copied()
    .collect();

  if filtered_candidates.is_empty() {
    previous_candidates
  } else {
    filtered_candidates
  }
}

fn truncate_to_char_boundary(s: &str, mut max: usize) -> &str {
  if max >= s.len() {
    return s;
  }

  while !s.is_char_boundary(max) {
    max -= 1;
  }

  &s[..max]
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

fn resolve_theme_with_overrides(
  theme: &str,
  theme_light: Option<&str>,
  theme_dark: Option<&str>,
) -> ResolvedTheme {
  let override_name = theme.trim();
  let theme_key = override_name.split(':').next().unwrap_or("auto");

  match theme_key {
    "" | "auto" => resolve_auto_theme(theme_light, theme_dark),
    "dark" => resolve_named_theme(theme_dark, true),
    "light" => resolve_named_theme(theme_light, false),
    _ => {
      if let Some(theme) = syntastica_themes::from_str(theme_key) {
        return theme;
      }
      resolve_auto_theme(theme_light, theme_dark)
    }
  }
}

fn resolve_named_theme(override_name: Option<&str>, prefer_dark: bool) -> ResolvedTheme {
  if let Some(name) = override_name {
    if let Some(theme) = syntastica_themes::from_str(name.trim()) {
      return theme;
    }
  }
  if prefer_dark {
    syntastica_themes::catppuccin::mocha()
  } else {
    syntastica_themes::catppuccin::latte()
  }
}

fn resolve_auto_theme(theme_light: Option<&str>, theme_dark: Option<&str>) -> ResolvedTheme {
  match dark_light::detect() {
    Ok(DarkLightMode::Light) => resolve_named_theme(theme_light, false),
    Ok(DarkLightMode::Dark) => resolve_named_theme(theme_dark, true),
    Ok(DarkLightMode::Unspecified) => resolve_named_theme(theme_dark, true),
    Err(_) => resolve_named_theme(theme_dark, true),
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

fn resolve_line_numbers(cli: &Cli) -> bool {
  let mut line_numbers = cli.line_numbers;
  if let Some(style) = cli.style.as_deref() {
    line_numbers = apply_style_line_numbers(line_numbers, style);
  }
  if cli.plain > 0 {
    line_numbers = false;
  }
  if cli.line_numbers {
    line_numbers = true;
  }
  line_numbers
}

fn apply_style_line_numbers(current: bool, style: &str) -> bool {
  let mut line_numbers = current;
  for raw in style.split(',') {
    let token = raw.trim();
    match token {
      "plain" | "-numbers" => line_numbers = false,
      "numbers" | "+numbers" => line_numbers = true,
      _ => {}
    }
  }
  line_numbers
}

fn squeeze_blank_lines_bytes(bytes: &[u8], limit: usize) -> Vec<u8> {
  if bytes.is_empty() {
    return Vec::new();
  }
  let mut out = Vec::with_capacity(bytes.len());
  let mut blank_count = 0usize;
  let mut start = 0usize;
  for (index, byte) in bytes.iter().enumerate() {
    if *byte == b'\n' {
      let line = &bytes[start..=index];
      let mut content_end = index;
      if content_end > start && bytes[content_end - 1] == b'\r' {
        content_end -= 1;
      }
      let is_blank = content_end == start;
      if is_blank {
        blank_count += 1;
        if blank_count <= limit {
          out.extend_from_slice(line);
        }
      } else {
        blank_count = 0;
        out.extend_from_slice(line);
      }
      start = index + 1;
    }
  }
  if start < bytes.len() {
    let line = &bytes[start..];
    let mut content_end = bytes.len();
    if content_end > start && bytes[content_end - 1] == b'\r' {
      content_end -= 1;
    }
    let is_blank = content_end == start;
    if is_blank {
      blank_count += 1;
      if blank_count <= limit {
        out.extend_from_slice(line);
      }
    } else {
      out.extend_from_slice(line);
    }
  }
  out
}
