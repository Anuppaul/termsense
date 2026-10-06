use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    env,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::ExitCode,
    time::UNIX_EPOCH,
};

const INDEX_VERSION: u32 = 1;

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
    /// Refresh the local command index immediately.
    Index,
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
    replacement_start: usize,
    replacement_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandEntry {
    name: String,
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DirStamp {
    path: PathBuf,
    modified_ns: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandIndex {
    version: u32,
    path_env: String,
    dirs: Vec<DirStamp>,
    commands: Vec<CommandEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CommandContext<'a> {
    prefix: &'a str,
    replacement_start: usize,
    replacement_end: usize,
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
            let index = load_or_refresh_index(false)?;
            if json {
                let commands: BTreeMap<&str, &Path> = index
                    .commands
                    .iter()
                    .map(|entry| (entry.name.as_str(), entry.path.as_path()))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string(&commands).map_err(|e| e.to_string())?
                );
            } else {
                for entry in index.commands {
                    println!("{}", entry.name);
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
            let index = load_or_refresh_index(false)?;
            let candidates = suggest(&index, &buffer, cursor, limit);
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&candidates).map_err(|e| e.to_string())?
                );
            } else {
                for c in candidates {
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}",
                        c.insert_text,
                        c.kind,
                        c.source,
                        c.score,
                        c.replacement_start,
                        c.replacement_end
                    );
                }
            }
        }
        Command::Index => {
            let index = load_or_refresh_index(true)?;
            println!("indexed {} commands", index.commands.len());
            if let Some(path) = cache_path() {
                println!("cache: {}", path.display());
            }
        }
        Command::Init { shell } if shell == "bash" => {
            print!("{}", include_str!("../shell/termsense.bash"));
        }
        Command::Init { .. } => unreachable!(),
        Command::Doctor => doctor()?,
        Command::Status => {
            let index = load_or_refresh_index(false)?;
            println!("TermSense {}", env!("CARGO_PKG_VERSION"));
            println!("platform: linux");
            println!("engine: local");
            println!("commands indexed: {}", index.commands.len());
            println!("network required: no");
            if let Some(path) = cache_path() {
                println!("command cache: {}", path.display());
            }
        }
    }
    Ok(())
}

fn doctor() -> Result<(), String> {
    println!("TermSense doctor");
    println!("  Linux: ok");

    match env::var("PATH") {
        Ok(path) if !path.trim().is_empty() => println!("  PATH: ok"),
        _ => return Err("PATH is missing".into()),
    }

    let index = load_or_refresh_index(false)?;
    if index.commands.is_empty() {
        return Err("command discovery returned 0 commands".into());
    }
    println!("  command discovery: ok ({} commands)", index.commands.len());

    if let Some(path) = cache_path() {
        println!("  command cache: {}", path.display());
    } else {
        println!("  command cache: disabled (HOME/XDG_CACHE_HOME unavailable)");
    }

    if env::var("BASH_VERSION").is_ok() {
        println!("  Bash: detected");
    } else {
        println!("  Bash: not detected in this process (shell integration still available)");
    }

    Ok(())
}

fn path_dirs() -> Vec<PathBuf> {
    let Some(path) = env::var_os("PATH") else {
        return Vec::new();
    };

    let mut seen = HashSet::new();
    env::split_paths(&path)
        .filter(|dir| seen.insert(dir.clone()))
        .collect()
}

fn dir_stamps(dirs: &[PathBuf]) -> Vec<DirStamp> {
    dirs.iter()
        .map(|path| {
            let modified_ns = fs::metadata(path)
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| u64::try_from(duration.as_nanos()).ok());
            DirStamp {
                path: path.clone(),
                modified_ns,
            }
        })
        .collect()
}

fn discover_path_commands(dirs: &[PathBuf]) -> Vec<CommandEntry> {
    let mut result: BTreeMap<String, PathBuf> = BTreeMap::new();

    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !is_executable_file(&meta) {
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
        .into_iter()
        .map(|(name, path)| CommandEntry { name, path })
        .collect()
}

fn is_executable_file(meta: &fs::Metadata) -> bool {
    meta.is_file() && meta.permissions().mode() & 0o111 != 0
}

fn cache_path() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_CACHE_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir).join("termsense/commands-v1.json"));
    }
    env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".cache/termsense/commands-v1.json"))
}

fn load_or_refresh_index(force: bool) -> Result<CommandIndex, String> {
    let path_env = env::var("PATH").unwrap_or_default();
    let dirs = path_dirs();
    let stamps = dir_stamps(&dirs);

    if !force {
        if let Some(path) = cache_path() {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(index) = serde_json::from_str::<CommandIndex>(&raw) {
                    if index.version == INDEX_VERSION
                        && index.path_env == path_env
                        && index.dirs == stamps
                    {
                        return Ok(index);
                    }
                }
            }
        }
    }

    let index = CommandIndex {
        version: INDEX_VERSION,
        path_env,
        dirs: stamps,
        commands: discover_path_commands(&dirs),
    };

    if let Some(path) = cache_path() {
        let _ = write_index_cache(&path, &index);
    }

    Ok(index)
}

fn write_index_cache(path: &Path, index: &CommandIndex) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err("invalid cache path".into());
    };
    fs::create_dir_all(parent).map_err(|e| format!("create cache directory: {e}"))?;

    let mut temp = path.to_path_buf();
    temp.set_extension(format!("tmp-{}", std::process::id()));
    let bytes = serde_json::to_vec(index).map_err(|e| format!("serialize command index: {e}"))?;

    let mut file = fs::File::create(&temp).map_err(|e| format!("create command cache: {e}"))?;
    file.write_all(&bytes)
        .map_err(|e| format!("write command cache: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("sync command cache: {e}"))?;
    fs::rename(&temp, path).map_err(|e| format!("replace command cache: {e}"))?;
    Ok(())
}

fn command_context(buffer: &str, cursor: usize) -> Option<CommandContext<'_>> {
    let before = &buffer[..cursor];
    let trimmed = before.trim_start_matches(char::is_whitespace);
    let leading = before.len() - trimmed.len();

    if trimmed.is_empty() {
        return Some(CommandContext {
            prefix: "",
            replacement_start: cursor,
            replacement_end: cursor,
        });
    }

    if let Some(after_sudo) = trimmed.strip_prefix("sudo") {
        if after_sudo.is_empty() {
            return Some(CommandContext {
                prefix: trimmed,
                replacement_start: leading,
                replacement_end: cursor,
            });
        }

        if after_sudo.chars().next().is_some_and(char::is_whitespace) {
            let command = after_sudo.trim_start_matches(char::is_whitespace);
            if command.contains(char::is_whitespace) {
                return None;
            }
            return Some(CommandContext {
                prefix: command,
                replacement_start: cursor - command.len(),
                replacement_end: cursor,
            });
        }
    }

    if trimmed.contains(char::is_whitespace) {
        return None;
    }

    Some(CommandContext {
        prefix: trimmed,
        replacement_start: leading,
        replacement_end: cursor,
    })
}

fn suggest(index: &CommandIndex, buffer: &str, cursor: usize, limit: usize) -> Vec<Candidate> {
    if limit == 0 {
        return Vec::new();
    }

    let Some(context) = command_context(buffer, cursor) else {
        return Vec::new();
    };

    let mut candidates: Vec<Candidate> = index
        .commands
        .iter()
        .filter_map(|entry| {
            score_prefix(&entry.name, context.prefix).map(|score| Candidate {
                insert_text: entry.name.clone(),
                display_text: entry.name.clone(),
                kind: "command",
                source: "path",
                score,
                replacement_start: context.replacement_start,
                replacement_end: context.replacement_end,
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
    use super::{command_context, score_prefix, CommandContext};

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

    #[test]
    fn parses_first_command_token() {
        assert_eq!(
            command_context("  sys", 5),
            Some(CommandContext {
                prefix: "sys",
                replacement_start: 2,
                replacement_end: 5,
            })
        );
    }

    #[test]
    fn parses_command_after_sudo() {
        assert_eq!(
            command_context("sudo sys", 8),
            Some(CommandContext {
                prefix: "sys",
                replacement_start: 5,
                replacement_end: 8,
            })
        );
    }

    #[test]
    fn stops_after_command_arguments_begin() {
        assert_eq!(command_context("git status", 10), None);
    }
}
