use clap::{Arg, ArgAction, Command, builder::Styles};
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::{Command as Process, ExitCode, Stdio};

const ZSH_INIT: &str = include_str!("../shell/repo.zsh");

fn query() -> Arg {
    Arg::new("query")
        .value_name("QUERY")
        .help("Filter repository names (ignores case)")
}

fn cli() -> Command {
    let styles = Styles::styled()
        .header(clap::builder::styling::AnsiColor::Cyan.on_default().bold())
        .literal(clap::builder::styling::AnsiColor::Green.on_default().bold());
    Command::new("repo")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Select and open your repositories")
        .styles(styles)
        .subcommand_precedence_over_arg(true)
        .arg(query())
        .help_template("{about}\n\n{usage-heading} {usage}\n\nNavigate:\n  go, cd       Select a repository and change directory\n  list, ls     Show repository candidates\n  root         Show the ghq root directory\n\nShell:\n  init         Print shell integration\n  completions  Print shell completions\n\nArguments:\n{positionals}\nOptions:\n{options}\n\nExamples:\n  repo                 Select from all repositories\n  repo vite            Select from names that contain 'vite'\n  repo go lets-start-go Open a single match directly\n  repo list vite       Show matching candidates\n\nSetup:\n  eval \"$(repo init zsh)\"\n")
        .subcommand(Command::new("go").visible_alias("cd")
            .about("Select a repository; print its path for shell integration")
            .arg(query())
            .after_help("With Zsh integration, this command changes directory.\nWithout integration, it prints the selected path. Enter selects; Esc cancels."))
        .subcommand(Command::new("list").visible_alias("ls")
            .about("Show repository candidates")
            .arg(query())
            .arg(Arg::new("paths").short('p').long("paths")
                .action(ArgAction::SetTrue).help("Print absolute paths")))
        .subcommand(Command::new("root").about("Show the ghq root directory"))
        .subcommand(Command::new("init").about("Print shell integration")
            .arg(Arg::new("shell").required(true).value_parser(["zsh"])))
        .subcommand(Command::new("completions").about("Print shell completions")
            .arg(Arg::new("shell").required(true).value_parser(["zsh", "bash", "fish"])))
}

#[derive(Debug)]
struct Failure {
    message: String,
    code: u8,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self { message, code: 1 }
    }
}

fn ghq(args: &[&str]) -> Result<String, Failure> {
    let output = Process::new("ghq")
        .args(args)
        .output()
        .map_err(|error| format!("Cannot run ghq: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ghq {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    String::from_utf8(output.stdout)
        .map_err(|_| "ghq returned a path that is not UTF-8".to_owned().into())
}

#[derive(Debug)]
struct Repository {
    path: String,
    name: String,
}

fn repositories(filter: Option<&String>) -> Result<Vec<Repository>, Failure> {
    let roots = ghq(&["root", "--all"])?;
    let paths = ghq(&["list", "-p"])?;
    let filter = filter.map(|query| query.to_lowercase());
    let repositories: Vec<_> = paths
        .lines()
        .filter(|line| !line.is_empty())
        .map(|path| {
            let name = roots
                .lines()
                .find_map(|root| {
                    Path::new(path)
                        .strip_prefix(root)
                        .ok()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| path.to_owned());
            Repository {
                path: path.to_owned(),
                name,
            }
        })
        .filter(|repo| {
            filter
                .as_ref()
                .is_none_or(|query| repo.name.to_lowercase().contains(query))
        })
        .collect();
    if repositories.is_empty() {
        return Err(match filter {
            Some(query) => {
                format!("No repositories match '{query}'. Use 'repo list' to see candidates.")
            }
            None => "No repositories found. Clone one with 'ghq get OWNER/REPO'.".to_owned(),
        }
        .into());
    }
    Ok(repositories)
}

fn select(repositories: &[Repository]) -> Result<&Repository, Failure> {
    if repositories.len() == 1 {
        return Ok(&repositories[0]);
    }
    let mut child = Process::new("fzf")
        .args([
            "--height=60%",
            "--layout=reverse",
            "--border=rounded",
            "--no-multi",
            "--prompt=repo> ",
            "--delimiter=\t",
            "--with-nth=2..",
            "--header=Select a repository | Enter: open | Esc: cancel",
        ])
        // User defaults can change the output format or enable automatic selection.
        .env_remove("FZF_DEFAULT_OPTS")
        .env_remove("FZF_DEFAULT_OPTS_FILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("Cannot run fzf: {error}. Install fzf to select a repository."))?;
    let write_result = (|| -> io::Result<()> {
        let mut input = child.stdin.take().expect("piped fzf input");
        for (index, repo) in repositories.iter().enumerate() {
            writeln!(input, "{index}\t{}", repo.name)?;
        }
        Ok(())
    })();
    let output = child
        .wait_with_output()
        .map_err(|error| format!("Cannot read fzf output: {error}"))?;
    if output.status.code() == Some(130) {
        return Err(Failure {
            message: String::new(),
            code: 130,
        });
    }
    if !output.status.success() {
        return Err(
            "Repository selection failed. Try 'repo list' to see candidates."
                .to_owned()
                .into(),
        );
    }
    write_result.map_err(|error| format!("Cannot send candidates to fzf: {error}"))?;
    let selected =
        String::from_utf8(output.stdout).map_err(|_| "fzf returned invalid text".to_owned())?;
    let index = selected
        .trim_end_matches(['\r', '\n'])
        .split_once('\t')
        .and_then(|(index, _)| index.parse::<usize>().ok())
        .ok_or_else(|| "fzf returned an invalid selection".to_owned())?;
    repositories
        .get(index)
        .ok_or_else(|| "fzf returned an unknown repository".to_owned().into())
}

fn run() -> Result<(), Failure> {
    let matches = cli().get_matches();
    match matches.subcommand() {
        Some(("root", _)) => print!("{}", ghq(&["root"])?),
        Some(("init", _)) => {
            print!("{ZSH_INIT}");
            clap_complete::generate(
                clap_complete::Shell::Zsh,
                &mut cli(),
                "repo",
                &mut io::stdout(),
            );
            print!("{}", include_str!("../shell/completions.zsh"));
        }
        Some(("completions", args)) => {
            let shell = args
                .get_one::<String>("shell")
                .unwrap()
                .parse::<clap_complete::Shell>()
                .unwrap();
            clap_complete::generate(shell, &mut cli(), "repo", &mut io::stdout());
        }
        Some(("list", args)) => {
            let repos = repositories(args.get_one::<String>("query"))?;
            let paths = args.get_flag("paths");
            let terminal = io::stdout().is_terminal();
            if terminal && !paths {
                println!("\nRepositories ({})\n", repos.len());
            }
            for repo in repos {
                println!("{}", if paths { repo.path } else { repo.name });
            }
            if terminal && !paths {
                println!("\nUse 'repo go <name>' to open a repository.");
            }
        }
        command => {
            let args = command.map_or(&matches, |(_, args)| args);
            let repos = repositories(args.get_one::<String>("query"))?;
            let selected = select(&repos)?;
            if !Path::new(&selected.path).is_dir() {
                return Err(
                    format!("Repository directory does not exist: {}", selected.path).into(),
                );
            }
            println!("{}", selected.path);
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if !error.message.is_empty() {
                eprintln!("repo: {}", error.message);
            }
            ExitCode::from(error.code)
        }
    }
}
