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
  - Set a branch to selected change with `b`
  - Fetch/push with `f`/`p`
- Files
  - View files in current change and diff in side panel
  - See a change's files from the log tab with `Enter`
  - View conflicts list in current change
  - Toggle between color words and git diff with `w`
- Branches
  - View list of branches, including from all remotes with `a`
  - Create with `c`, rename with `r`, delete with `d`, forget with `f`
  - Track branches with `t`, untrack branches with `T`
- Command log: View every command lazyjj executes
- Config: Configure lazyjj with your jj config
- Help: See all key mappings with `h`/`?`

## Setup

Make sure you have [`jj`](https://martinvonz.github.io/jj/latest/install-and-setup) installed first.

- With [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall): `cargo binstall lazyjj`
- With `cargo install`: `cargo +nightly install lazyjj` (requires nightly, may take a few moments to compile)
- With pre-built binaries: [View releases](https://github.com/Cretezy/lazyjj/releases)
- For Arch Linux: `pacman -S lazyjj`

To build and install a pre-release version: `cargo +nightly install --git https://github.com/Cretezy/lazyjj.git --locked`

## Configuration

You can optionally configure the following options through your jj config:

- `lazyjj.higlight-color`: Changes the highlight color. Can use named colors. Defaults to `#323264`
- `lazyjj.diff-format`: Change the default diff format. Can be `color-words` or `git`. Defaults to `color_words`
  - If `lazyjj.diff-format` is not set but `ui.diff.format` is, the latter will be used
- `lazyjj.branch-prefix`: Change the branch name prefix for generated branch names. Defaults to `push-`
  - If `lazyjj.branch-prefix` is not set but `git.push-branch-prefix` is, the latter will be used

Example: `jj config set --user lazyjj.diff-format "color-words"` (for storing in [user config file](https://martinvonz.github.io/jj/latest/config/#user-config-file), repo config is also supported)

## Usage

To start lazyjj for the repository in the current directory: `lazyjj`

To use a different repository: `lazyjj --path ~/path/to/repo`

## Key mappings

See all key mappings for the current tab with `h` or `?`.

### Basic navigation

- Quit with `q`
- Change tab with `1`/`2`/`3`
- Scrolling in left panel
  - Scroll down/up by one line with `j`/`k` or down/up arrow
  - Scroll down/up by half page with `J`/`K` or down/up arrow
- Scrolling in right panel
  - Scroll down/up by one line with `Ctrl+e`/`Ctrl+y`
  - Scroll down/up by a half page with `Ctrl+d`/`Ctrl+u`
  - Scroll down/up by a full page with `Ctrl+f`/`Ctrl+b`

### Log tab

- Select current change with `@`
- View change files in files tab with `Enter`
- Display different revset with `r` (`jj log -r`)
- Change right panel diff format between color words (default) and Git with `w`
- Toggle right panel wrapping with `W`
- Create new change after highlighted change with `n` (`jj new`)
  - Create new change and describe with `N` (`jj new -m`)
- Edit highlighted change `e` (`jj edit`)
- Abandon a change with `a` (`jj abandon`)
- Describe the highlighted change with `d` (`jj describe`)
  - Save with `Ctrl+s`
  - Cancel with `Esc`
- Set a branch to the highlighted change with `b` (`jj branch set`)
  - Scroll in branch list with `j`/`k`
  - Create a new branch with `c`
  - Use auto-generated name with `g`
- Git fetch with `f` (`jj git fetch`)
  - Git fetch all remotes with `F` (`jj git fetch --all-remotes`)
- Git push with `p` (`jj git push`)
  - Git push all branches with `P` (`jj git push --all`)

### Files tab

- Select current change with `@`
- Change right panel diff format between color words (default) and Git with `w`
- Toggle right panel wrapping with `W`

### Branches tab

- Show branches with all remotes with `a` (`jj branch list --all`)
- Create a branch with `c` (`jj branch create`)
- Rename a branch with `r` (`jj branch rename`)
- Delete a branch with `d` (`jj branch delete`)
- Forget a branch with `f` (`jj branch forget`)
- Track a branch with `t` (only works for branches with remotes) (`jj branch track`)
- Untrack a branch with `T` (only works for branches with remotes) (`jj branch untrack`)
- Change right panel diff format between color words (default) and Git with `w`
- Toggle right panel wrapping with `W`
- Create a new change after the highlighted branch's change with `n` (`jj new`)
  - Create a new change and describe with `N` (`jj new -m`)

### Command log tab

- Select latest command with `@`
- Toggle right panel wrapping with `W`
