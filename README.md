# repo

Personal Rust CLI for ghq repositories. Select a repository, see matching candidates,
or move directly to one match. Zsh integration changes the current shell directory.

```sh
repo                     # Select and open a repository
repo vite                # Select from names that contain vite
repo go lets-start-go    # Open a single match
repo list vite           # Show matching names
repo list --paths        # Print absolute paths
repo --help              # Show commands and examples
```

Names are matched without case sensitivity. The picker shows repository names, without
the common ghq root path. Enter selects a repository. Esc cancels and keeps the current directory.
Tab completes commands and current ghq repository names in Zsh.

## Install

Requires Rust, ghq, and fzf. Install from this repository:

```sh
cargo install --path . --locked --root ~/.local
```

Keep `~/.local/bin` in PATH. Add this line after `compinit` in `~/.zshrc`:

```sh
eval "$(command repo init zsh)"
```

Start a new terminal, or run the line above in the current terminal.
Without Zsh integration, `repo` and `repo go` print the selected path.

## Develop

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run the install command again after changes. The application and shell integration are
managed here. The dotfiles repository loads the installed application's integration.
