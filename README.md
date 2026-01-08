# scat

A modern replacement for `cat` with syntax highlighting and automatic language detection.

## Features

- **Syntax highlighting** for 100+ languages using tree-sitter parsers
- **Automatic language detection** based on file extension and content
- **Line numbers** with `-n` flag
- **Theme support** with automatic dark/light mode detection
- **Stdin support** for piping commands
- **Fast** - built with Rust and tree-sitter
- **Shell completions** for bash, zsh, fish, and more

## Installation

### From source

```bash
cargo install --path .
```

Or using `mise` (recommended for development):

```bash
mise install
mise build:debug
```

## Usage

### Getting help

```bash
# Quick help
scat -h

# Detailed help with examples
scat --help

# View man page (after installation)
man scat
```

### Basic usage

Display a file with syntax highlighting:

```bash
scat main.rs
```

Display multiple files:

```bash
scat main.rs lib.rs
```

Read from stdin:

```bash
echo 'fn main() { println!("Hello!"); }' | scat
# or explicitly:
cat main.rs | scat -
```

### Line numbers

Show line numbers with the `-n` or `--line-numbers` flag:

```bash
scat -n main.rs
```

### Language override

Force a specific language when auto-detection fails:

```bash
scat --language rust config.txt
scat --language json response.log
```

To see all supported languages, check the [syntastica documentation](https://docs.rs/syntastica-parsers/latest/syntastica_parsers/).

### Themes

Specify a theme with `--theme`:

```bash
scat --theme dracula main.rs
scat --theme gruvbox-light main.rs
```

By default, `scat` uses `auto` which detects your system's dark/light mode preference:
- **Light mode**: Catppuccin Latte
- **Dark mode**: Catppuccin Mocha

#### Available themes

See the full list of themes in the [syntastica-themes documentation](https://docs.rs/syntastica-themes/latest/syntastica_themes/).

Popular themes include:
- `dracula`
- `gruvbox-dark` / `gruvbox-light`
- `nord`
- `one-dark` / `one-light`
- `catppuccin-mocha` / `catppuccin-latte` / `catppuccin-frappe` / `catppuccin-macchiato`
- `solarized-dark` / `solarized-light`
- `tokyo-night`

### Shell completions

Generate shell completions for your shell:

```bash
# Bash
scat --completions bash > ~/.local/share/bash-completion/completions/scat

# Zsh
scat --completions zsh > ~/.zsh/completion/_scat

# Fish
scat --completions fish > ~/.config/fish/completions/scat.fish

# PowerShell
scat --completions powershell > scat.ps1
```

### Man page

Generate and install a man page:

```bash
# Generate man page
scat --man-page > scat.1

# Install system-wide (requires sudo)
sudo cp scat.1 /usr/local/share/man/man1/

# View the man page
man scat
```

## Examples

```bash
# View a Rust file with line numbers
scat -n src/main.rs

# View JSON with syntax highlighting
scat package.json

# Pipe git diff through scat
git diff | scat --language diff

# Compare files side by side with syntax highlighting
diff <(scat file1.js) <(scat file2.js)

# View logs with Python syntax highlighting
scat --language python app.log

# Use a specific theme
scat --theme nord config.yaml

# View multiple files with line numbers
scat -n *.rs
```

## Supported Languages

`scat` supports 100+ programming languages through tree-sitter parsers, including:

- C, C++, C#, Objective-C
- Rust, Go, Zig
- JavaScript, TypeScript, JSX, TSX
- Python, Ruby, Perl, PHP
- Java, Kotlin, Scala
- Swift, Dart
- HTML, CSS, SCSS, Vue
- Bash, Fish, PowerShell
- SQL, GraphQL
- YAML, TOML, JSON, XML
- Markdown, reStructuredText
- And many more...

For a complete list, see the [syntastica-parsers documentation](https://docs.rs/syntastica-parsers/latest/syntastica_parsers/).

## Why scat?

- **Modern syntax highlighting** using tree-sitter for accurate, grammar-aware highlighting
- **Smart defaults** with automatic theme and language detection
- **Familiar interface** - works just like `cat` but with colors
- **Fast and reliable** - built with Rust for performance and safety
- **No configuration needed** - works out of the box with sensible defaults

## Comparison with other tools

| Feature | scat | bat | ccat | highlight |
|---------|------|-----|------|-----------|
| Syntax highlighting | ✅ Tree-sitter | ✅ syntect | ✅ pygments | ✅ |
| Auto language detection | ✅ | ✅ | ✅ | ✅ |
| Line numbers | ✅ | ✅ | ❌ | ✅ |
| Themes | ✅ Many | ✅ Many | ✅ Few | ✅ Many |
| Git integration | ❌ | ✅ | ❌ | ❌ |
| Paging | ❌ | ✅ | ❌ | ❌ |
| Binary files | ❌ | ✅ | ❌ | ❌ |

`scat` focuses on being a simple, fast `cat` replacement with excellent syntax highlighting. If you need features like git integration or a built-in pager, check out [bat](https://github.com/sharkdp/bat).

## Development

This project uses `mise` for development:

```bash
# Install dependencies
mise install

# Build
mise build:debug

# Run tests
mise test

# Format code
mise format
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Acknowledgments

- [syntastica](https://github.com/RubixDev/syntastica) - Syntax highlighting library
- [tree-sitter](https://tree-sitter.github.io/) - Parser generator tool
- [clap](https://github.com/clap-rs/clap) - Command line argument parser
