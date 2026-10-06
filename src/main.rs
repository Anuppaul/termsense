use clap::{Parser, Subcommand};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    env,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Parser, Debug)]
#[command(name = "termsense", version, about = "Context-aware terminal suggestions for Linux")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List commands currently discoverable through PATH.
    ListCommands {
        /// Return JSON instead of plain text.
        #[arg(long)]
        json: bool,
    },
    /// Suggest commands for the current command-line buffer.
    Suggest {
        /// Current shell buffer.
        buffer: String,
        /// Cursor position in bytes. Defaults to the end of the buffer.
        #[arg(long)]
        cursor: Option<usize>,
        /// Maximum number of candidates.
        #[arg(short = 'n', long, default_value_t = 12)]
        limit: usize,
        /// Return JSON instead of tab-separated text.
        #[arg(long)]
        json: bool,
    },
    /// Print shell integration code.
    Init {
        #[arg(value_parser = ["bash"])]
        shell: String,
    },
    /// Check whether the current environment can run TermSense.
    Doctor,
    /// Print the current engine status.
    Status,
}

#[derive(Debug, Clone, Serialize)]
struct Candidate {
    insert_text: String,
    display_text: String,
    kind: &'static str,
    source: &'static str,
    score: i64,
}

fn main() -> ExitCode {
    if !cfg!(target_os = "linux") {
        eprintln!("TermSense is Linux-only.");
        return ExitCode::FAILURE;
    }

    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("termsense: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::ListCommands { json } => {
            let commands = discover_path_commands();
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&commands).map_err(|e| e.to_string())?
                );
            } else {
                for command in commands.keys() {
                    println!("{command}");
                }
            }
        }
        Command::Suggest {
            buffer,
            cursor,
            limit,
            json,
        } => {
            let cursor = cursor.unwrap_or(buffer.len());
            if cursor > buffer.len() || !buffer.is_char_boundary(cursor) {
                return Err("cursor is outside the UTF-8 buffer boundary".into());
            }
            let candidates = suggest(&buffer, cursor, limit);
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&candidates).map_err(|e| e.to_string())?
                );
            } else {
                for c in candidates {
                    println!(
                        "{}\t{}\t{}\t{}",
                        c.insert_text, c.kind, c.source, c.score
                    );
                }
            }
        }
        Command::Init { shell } if shell == "bash" => {
            print!("{}", include_str!("../shell/termsense.bash"));
        }
        Command::Init { .. } => unreachable!(),
        Command::Doctor => doctor(),
        Command::Status => {
            let commands = discover_path_commands();
            println!("TermSense 0.1.0");
            println!("platform: linux");
            println!("engine: local");
            println!("commands indexed: {}", commands.len());
            println!("network required: no");
        }
    }
    Ok(())
}

fn doctor() {
    let mut ok = true;
    println!("TermSense doctor");
    println!("  Linux: ok");

    match env::var("PATH") {
        Ok(path) if !path.trim().is_empty() => println!("  PATH: ok"),
        _ => {
            println!("  PATH: missing");
            ok = false;
        }
    }

    let commands = discover_path_commands();
    if commands.is_empty() {
        println!("  command discovery: failed (0 commands)");
        ok = false;
    } else {
        println!("  command discovery: ok ({} commands)", commands.len());
    }

    if env::var("BASH_VERSION").is_ok() {
        println!("  Bash: detected");
    } else {
        println!("  Bash: not detected in this process (shell integration still available)");
    }

    if !ok {
        std::process::exit(1);
    }
}

fn discover_path_commands() -> BTreeMap<String, PathBuf> {
    let mut result = BTreeMap::new();
    let mut visited = HashSet::new();

    let Some(path) = env::var_os("PATH") else {
        return result;
    };

    for dir in env::split_paths(&path) {
        if !visited.insert(dir.clone()) {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !is_executable_file(&path, &meta) {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            result.entry(name.to_owned()).or_insert(path);
        }
    }

    result
}

fn is_executable_file(path: &Path, meta: &fs::Metadata) -> bool {
    (meta.is_file() || path.is_symlink()) && meta.permissions().mode() & 0o111 != 0
}

fn suggest(buffer: &str, cursor: usize, limit: usize) -> Vec<Candidate> {
    if limit == 0 {
        return Vec::new();
    }

    let before_cursor = &buffer[..cursor];
    let token_start = before_cursor
        .rfind(char::is_whitespace)
        .map(|i| i + 1)
        .unwrap_or(0);
    let token = &before_cursor[token_start..];
    let is_command_position = before_cursor[..token_start].trim().is_empty();

    if !is_command_position {
        return Vec::new();
    }

    let normalized = token.strip_prefix("sudo").unwrap_or(token);
    let prefix = if token == "sudo" { "" } else { normalized };

    let mut candidates: Vec<Candidate> = discover_path_commands()
        .into_keys()
        .filter_map(|command| {
            score_prefix(&command, prefix).map(|score| Candidate {
                insert_text: command.clone(),
                display_text: command,
                kind: "command",
                source: "path",
                score,
            })
        })
        .collect();

    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.display_text.len().cmp(&b.display_text.len()))
            .then_with(|| a.display_text.cmp(&b.display_text))
    });
    candidates.truncate(limit);
    candidates
}

fn score_prefix(candidate: &str, query: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(100 - candidate.len() as i64);
    }
    if candidate == query {
        return Some(10_000);
    }
    if candidate.starts_with(query) {
        return Some(5_000 - (candidate.len() as i64 - query.len() as i64));
    }

    let candidate_lower = candidate.to_ascii_lowercase();
    let query_lower = query.to_ascii_lowercase();
    if candidate_lower.starts_with(&query_lower) {
        return Some(4_000 - (candidate.len() as i64 - query.len() as i64));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::score_prefix;

    #[test]
    fn exact_match_wins() {
        assert!(score_prefix("git", "git").unwrap() > score_prefix("gitk", "git").unwrap());
    }

    #[test]
    fn prefix_matches() {
        assert!(score_prefix("systemctl", "sys").is_some());
        assert!(score_prefix("journalctl", "sys").is_none());
    }

    #[test]
    fn empty_query_allows_discovery() {
        assert!(score_prefix("bash", "").is_some());
    }
}
