use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    directory: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "repo-cli-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let root = directory.join("repositories with spaces");
        fs::create_dir_all(root.join("github.com/me/alpha")).unwrap();
        fs::create_dir_all(root.join("github.com/me/beta project")).unwrap();
        let fixture = Self { directory, root };
        fixture.script("ghq", r#"#!/bin/sh
if [ "$REPO_TEST_GHQ_FAIL" = 1 ]; then
  printf 'ghq test failure\n' >&2
  exit 7
fi
case "$1" in
  get) shift; printf 'get %s\n' "$*" ;;
  root) printf '%s\n' "$REPO_TEST_ROOT" ;;
  list)
    if [ "$2" = -p ]; then
      printf '%s/github.com/me/alpha\n%s/github.com/me/beta project\n' "$REPO_TEST_ROOT" "$REPO_TEST_ROOT"
    else
      printf 'github.com/me/alpha\ngithub.com/me/beta project\n'
    fi
    ;;
esac
"#);
        fixture.script(
            "fzf",
            r#"#!/bin/sh
while IFS= read -r line; do selected=$line; done
case "$REPO_TEST_PICKER" in
  cancel) exit 130 ;;
  invalid) printf '999\tunknown\n' ;;
  *) printf '%s\n' "$selected" ;;
esac
"#,
        );
        symlink(env!("CARGO_BIN_EXE_repo"), fixture.directory.join("repo")).unwrap();
        fixture
    }

    fn script(&self, name: &str, text: &str) {
        let path = self.directory.join(name);
        fs::write(&path, text).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_repo"));
        command
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.directory.display()),
            )
            .env("REPO_TEST_ROOT", &self.root)
            .current_dir(&self.directory);
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn help_has_command_groups_and_usage_examples() {
    let output = Fixture::new().run(&["--help"]);
    assert!(output.status.success());
    for text in ["Navigate:", "Shell:", "repo list vite", "repo init zsh"] {
        assert!(stdout(&output).contains(text));
    }
}

#[test]
fn get_passes_arguments_to_ghq() {
    let output = Fixture::new().run(&["get", "--shallow", "me/alpha"]);
    assert!(output.status.success());
    assert_eq!(stdout(&output), "get --shallow me/alpha\n");
}

#[test]
fn list_filters_names_without_case_sensitivity() {
    let fixture = Fixture::new();
    let output = fixture.run(&["list", "BETA"]);
    assert!(output.status.success());
    assert_eq!(stdout(&output), "github.com/me/beta project\n");
    let output = fixture.run(&["ls", "--paths", "BETA"]);
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        format!("{}/github.com/me/beta project\n", fixture.root.display())
    );
}

#[test]
fn one_match_opens_without_a_picker() {
    let fixture = Fixture::new();
    let output = fixture
        .command()
        .args(["go", "alpha"])
        .env("REPO_TEST_PICKER", "cancel")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        format!("{}/github.com/me/alpha\n", fixture.root.display())
    );
}

#[test]
fn picker_selects_a_path_with_spaces() {
    let fixture = Fixture::new();
    let output = fixture.run(&[]);
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        format!("{}/github.com/me/beta project\n", fixture.root.display())
    );
}

#[test]
fn cancelled_picker_emits_no_path() {
    let fixture = Fixture::new();
    let output = fixture
        .command()
        .env("REPO_TEST_PICKER", "cancel")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(130));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn unknown_picker_selection_is_rejected() {
    let output = Fixture::new()
        .command()
        .env("REPO_TEST_PICKER", "invalid")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn no_matches_and_ghq_failure_emit_no_path() {
    let fixture = Fixture::new();
    let output = fixture.run(&["go", "absent"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("repo list"));
    let output = fixture
        .command()
        .env("REPO_TEST_GHQ_FAIL", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ghq test failure"));
}

#[test]
fn zsh_integration_moves_and_keeps_directory_for_help_and_cancel() {
    if !std::path::Path::new("/bin/zsh").exists() {
        return;
    }
    let fixture = Fixture::new();
    let output = Command::new("/bin/zsh")
        .args([
            "-f",
            "-c",
            r#"
autoload -Uz compinit
compinit -D
eval "$(command repo init zsh)"
repo go alpha || exit 1
[[ $PWD = "$REPO_TEST_ROOT/github.com/me/alpha" ]] || exit 2
repo go --help >/dev/null || exit 3
[[ $PWD = "$REPO_TEST_ROOT/github.com/me/alpha" ]] || exit 4
export REPO_TEST_PICKER=cancel
repo
[[ $? = 130 ]] || exit 5
[[ $PWD = "$REPO_TEST_ROOT/github.com/me/alpha" ]] || exit 6
unset REPO_TEST_PICKER
repo cd 'beta project' || exit 7
[[ $PWD = "$REPO_TEST_ROOT/github.com/me/beta project" ]] || exit 8
(( $+functions[_repo] )) || exit 9
"#,
        ])
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", fixture.directory.display()),
        )
        .env("REPO_TEST_ROOT", &fixture.root)
        .current_dir(&fixture.directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}
