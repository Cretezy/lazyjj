# lazyjj

TUI for [Jujutsu/jj](https://github.com/martinvonz/jj). Built in Rust with Ratatui. Interacts with `jj` CLI.

https://github.com/Cretezy/lazyjj/assets/2672503/b5e6b4f1-ebdb-448f-af9e-361e86f0c148

## Features

- Log
  - Scroll through the jj log and view change details in side panel
  - Create new changes from selected change with `n`
  - Edit changes with `e`
  - Desribe changes with `d`
  - Abandon changes with `a`
  - Toggle between color words and git diff with `p`
  - See different revset with `r`
  - Set a bookmark to selected change with `b`
  - Fetch/push with `f`/`p`
  - Squash current changes to selected change with `s`
- Files
  - View files in current change and diff in side panel
  - See a change's files from the log tab with `Enter`
  - View conflicts list in current change
  - Toggle between color words and git diff with `w`
- Bookmarks
  - View list of bookmarks, including from all remotes with `a`
  - Create with `c`, rename with `r`, delete with `d`, forget with `f`
  - Track bookmarks with `t`, untrack bookmarks with `T`
- Command log: View every command lazyjj executes
- Config: Configure lazyjj with your jj config
- Command box: Run jj commands directly in lazyjj with `:`
- Help: See all key mappings with `h`/`?`

## Setup

Make sure you have [`jj`](https://martinvonz.github.io/jj/latest/install-and-setup) installed first.

- With [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall): `cargo binstall lazyjj`
- With `cargo install`: `cargo install lazyjj` (may take a few moments to compile)
- With pre-built binaries: [View releases](https://github.com/Cretezy/lazyjj/releases)
- For Arch Linux: `pacman -S lazyjj`

To build and install a pre-release version: `cargo install --git https://github.com/Cretezy/lazyjj.git --locked`

## Configuration

You can optionally configure the following options through your jj config:

- `lazyjj.highlight-color`: Changes the highlight color. Can use named colors. Defaults to `#323264`
- `lazyjj.diff-format`: Change the default diff format. Can be `color-words` or `git`. Defaults to `color_words`
  - If `lazyjj.diff-format` is not set but `ui.diff.format` is, the latter will be used
- `lazyjj.bookmark-prefix`: Change the bookmark name prefix for generated bookmark names. Defaults to `push-`
  - If `lazyjj.bookmark-prefix` is not set but `git.push-bookmark-prefix` is, the latter will be used
- `lazyjj.layout`: Changes the layout of the main and details panel. Can be `horizontal` (default) or `vertical`
- `lazyjj.layout-percent`: Changes the layout split of the main page. Should be number between 0 and 100. Defaults to `50`

Example: `jj config set --user lazyjj.diff-format "color-words"` (for storing in [user config file](https://martinvonz.github.io/jj/latest/config/#user-config-file), repo config is also supported)

## Usage

To start lazyjj for the repository in the current directory: `lazyjj`

To use a different repository: `lazyjj --path ~/path/to/repo`

To start with a different default revset: `lazyjj -r '::@'`

## Key mappings

See all key mappings for the current tab with `h` or `?`.

### Basic navigation

- Quit with `q`
- Change tab with `1`/`2`/`3`
- Scrolling in main panel
  - Scroll down/up by one line with `j`/`k` or down/up arrow
  - Scroll down/up by half page with `J`/`K` or down/up arrow
- Scrolling in details panel
  - Scroll down/up by one line with `Ctrl+e`/`Ctrl+y`
  - Scroll down/up by a half page with `Ctrl+d`/`Ctrl+u`
  - Scroll down/up by a full page with `Ctrl+f`/`Ctrl+b`
- Open a command popup to run jj commands using `:` (jj prefix not required, e.g. write `new main` instead of `jj new main`)

### Log tab

- Select current change with `@`
- View change files in files tab with `Enter`
- Display different revset with `r` (`jj log -r`)
- Change details panel diff format between color words (default) and Git (and diff tool if set) with `w`
- Toggle details panel wrapping with `W`
- Create new change after highlighted change with `n` (`jj new`)
  - Create new change and describe with `N` (`jj new -m`)
- Edit highlighted change `e` (`jj edit`)
- Abandon a change with `a` (`jj abandon`)
- Describe the highlighted change with `d` (`jj describe`)
  - Save with `Ctrl+s`
  - Cancel with `Esc`
- Set a bookmark to the highlighted change with `b` (`jj bookmark set`)
  - Scroll in bookmark list with `j`/`k`
  - Create a new bookmark with `c`
  - Use auto-generated name with `g`
- Squash current changes (in @) to the selected change with `s`
- Git fetch with `f` (`jj git fetch`)
  - Git fetch all remotes with `F` (`jj git fetch --all-remotes`)
- Git push with `p` (`jj git push`)
  - Git push all bookmarks with `P` (`jj git push --all`)
  - Use `Ctrl+p` or `Ctrl+P` to include pushing new bookmarks (`--allow-new`)

### Files tab

- Select current change with `@`
- Change details panel diff format between color words (default) and Git (and diff tool if set) with `w`
- Toggle details panel wrapping with `W`

### Bookmarks tab

- Show bookmarks with all remotes with `a` (`jj bookmark list --all`)
- Create a bookmark with `c` (`jj bookmark create`)
- Rename a bookmark with `r` (`jj bookmark rename`)
- Delete a bookmark with `d` (`jj bookmark delete`)
- Forget a bookmark with `f` (`jj bookmark forget`)
- Track a bookmark with `t` (only works for bookmarks with remotes) (`jj bookmark track`)
- Untrack a bookmark with `T` (only works for bookmarks with remotes) (`jj bookmark untrack`)
- Change details panel diff format between color words (default) and Git (and diff tool if set) with `w`
- Toggle details panel wrapping with `W`
- Create a new change after the highlighted bookmark's change with `n` (`jj new`)
  - Create a new change and describe with `N` (`jj new -m`)

### Command log tab

- Select latest command with `@`
- Toggle details panel wrapping with `W`

### Configuring

Keys can be configured

```toml
[lazyjj.keybinds.log_tab]
save = "ctrl+s"
```

See more in [keybindings.md](docs/keybindings.md)

## Development

### Setup

1. Install Rust and
2. Clone repository
3. Run with `cargo run`
4. Build with `cargo build --release` (output in `target/release`)
5. You can point it to another jj repo with `--path`: `cargo run -- --path ~/other-repo`

### Logging/Tracing

lazyjj has 2 debugging tools:

1. Logging: Enabled by setting `LAZYJJ_LOG=1` when running. Produces a `lazyjj.log` log file
2. Tracing: Enabled by setting `LAZYJJ_TRACE=1` when running. Produces `trace-*.json` Chrome trace file, for `chrome://tracing` or [ui.perfetto.dev](https://ui.perfetto.dev)
