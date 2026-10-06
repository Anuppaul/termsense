mod apt_cache;
mod man_cache;
mod providers;
mod runtime_cache;
mod shell_parse;
mod usage;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::ExitCode,
    time::UNIX_EPOCH,
};

const INDEX_VERSION: u32 = 1;

#[derive(Parser, Debug)]
#[command(
    name = "termsense",
    version,
    about = "Context-aware terminal suggestions for Linux"
)]
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
    /// Suggest commands or arguments for the current command-line buffer.
    Suggest {
        /// Current shell buffer.
        buffer: String,
        /// Cursor position in bytes. Defaults to the end of the buffer.
        #[arg(long)]
        cursor: Option<usize>,
        /// Maximum number of candidates. Use 0 for no truncation.
        #[arg(short = 'n', long, default_value_t = 12)]
        limit: usize,
        /// Return JSON instead of tab-separated text.
        #[arg(long)]
        json: bool,
    },
    /// Refresh the local command index immediately.
    Index,
    /// Record an accepted TermSense suggestion for local ranking.
    #[command(hide = true)]
    Record {
        /// Privacy-safe derived usage key.
        value: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CommandEntry {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
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
            let usage = usage::UsageState::load();
            let candidates = providers::suggest(&index.commands, &usage, &buffer, cursor, limit);
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&candidates).map_err(|e| e.to_string())?
                );
            } else {
                for candidate in candidates {
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                        candidate.insert_text,
                        shell_safe_display(&candidate.display_text),
                        candidate.kind,
                        candidate.source,
                        candidate.score,
                        byte_to_char_offset(&buffer, candidate.replacement_start),
                        byte_to_char_offset(&buffer, candidate.replacement_end),
                        candidate.usage_key,
                        shell_safe_display(&candidate.description)
                    );
                }
            }
        }
        Command::Index => {
            runtime_cache::clear();
            let index = load_or_refresh_index(true)?;
            println!("indexed {} commands", index.commands.len());
            if let Some(path) = cache_path() {
                println!("cache: {}", path.display());
            }
        }
        Command::Record { value } => {
            usage::UsageState::record(&value)?;
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
            println!("context providers: shell, git, systemd, docker, filesystem, ssh, apt, project manifests");
            println!("network required: no");
            if let Some(path) = cache_path() {
                println!("command cache: {}", path.display());
            }
            let usage = usage::UsageState::load();
            println!("accepted usage keys: {}", usage.entries());
            println!("accepted usage events: {}", usage.total_events());
            if let Some(path) = usage::state_path() {
                println!("usage state: {}", path.display());
            }
            if let Some(path) = apt_cache::cache_path() {
                println!("apt package cache: {}", path.display());
            }
            if let Some(path) = man_cache::cache_root() {
                println!("man option cache: {}", path.display());
            }
            if let Some(path) = runtime_cache::root_path() {
                println!("ephemeral runtime cache: {}", path.display());
            }
            if let Some(path) = config_path() {
                println!("config file: {}", path.display());
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
    println!(
        "  command discovery: ok ({} commands)",
        index.commands.len()
    );

    if let Some(path) = cache_path() {
        println!("  command cache: {}", path.display());
    } else {
        println!("  command cache: disabled (HOME/XDG_CACHE_HOME unavailable)");
    }

    println!(
        "  git provider: {}",
        if command_available(&index, "git") {
            "available"
        } else {
            "not installed"
        }
    );
    println!(
        "  docker provider: {}",
        if command_available(&index, "docker") {
            "available"
        } else {
            "not installed"
        }
    );
    println!(
        "  systemctl provider: {}",
        if command_available(&index, "systemctl") {
            "available"
        } else {
            "not installed"
        }
    );

    if let Some(path) = config_path() {
        if path.is_file() {
            println!("  config: {}", path.display());
        } else {
            println!("  config: defaults ({} not found)", path.display());
        }
    }

    let shell = env::var("SHELL").unwrap_or_default();
    if shell.ends_with("/bash") || shell == "bash" {
        println!("  shell: Bash configured ({shell})");
    } else if shell.is_empty() {
        println!("  shell: unknown (SHELL is unset)");
    } else {
        println!("  shell: {shell} (Bash adapter remains available)");
    }

    Ok(())
}

fn byte_to_char_offset(value: &str, byte_offset: usize) -> usize {
    value[..byte_offset].chars().count()
}

fn shell_safe_display(value: &str) -> String {
    let mut output = String::with_capacity(value.len());

    for ch in value.chars() {
        match ch {
            '\n' => output.push_str(" ↵ "),
            '\t' => output.push_str(" ⇥ "),
            '\r' => output.push(' '),
            ch if ch.is_control() => output.push('�'),
            ch => output.push(ch),
        }
    }

    output
}

fn command_available(index: &CommandIndex, name: &str) -> bool {
    index.commands.iter().any(|entry| entry.name == name)
}

fn path_dirs() -> Vec<PathBuf> {
    let Some(path) = env::var_os("PATH") else {
        return Vec::new();
    };

    let cwd = env::current_dir().ok();
    let mut seen = HashSet::new();

    env::split_paths(&path)
        .map(|dir| resolve_path_entry(dir, cwd.as_deref()))
        .filter(|dir| seen.insert(dir.clone()))
        .collect()
}

fn resolve_path_entry(dir: PathBuf, cwd: Option<&Path>) -> PathBuf {
    if dir.as_os_str().is_empty() {
        return cwd
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
    }

    if dir.is_relative() {
        if let Some(cwd) = cwd {
            return cwd.join(dir);
        }
    }

    dir
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
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
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

fn config_path() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir).join("termsense/config.conf"));
    }

    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".config/termsense/config.conf"))
}

fn cache_path() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir).join("termsense/commands-v1.json"));
    }

    env::var_os("HOME")
        .filter(|value| !value.is_empty())
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
    fs::create_dir_all(parent).map_err(|err| format!("create cache directory: {err}"))?;

    let mut temp = path.to_path_buf();
    temp.set_extension(format!("tmp-{}", std::process::id()));
    let bytes =
        serde_json::to_vec(index).map_err(|err| format!("serialize command index: {err}"))?;

    let mut file = fs::File::create(&temp).map_err(|err| format!("create command cache: {err}"))?;
    file.write_all(&bytes)
        .map_err(|err| format!("write command cache: {err}"))?;
    file.sync_all()
        .map_err(|err| format!("sync command cache: {err}"))?;
    fs::rename(&temp, path).map_err(|err| format!("replace command cache: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        byte_to_char_offset, discover_path_commands, resolve_path_entry, shell_safe_display,
    };
    use std::{fs, os::unix::fs::PermissionsExt};

    #[test]
    fn relative_path_entries_are_resolved_against_cwd() {
        let cwd = std::path::Path::new("/tmp/example");
        assert_eq!(
            resolve_path_entry(std::path::PathBuf::from("bin"), Some(cwd)),
            cwd.join("bin")
        );
        assert_eq!(
            resolve_path_entry(std::path::PathBuf::new(), Some(cwd)),
            cwd
        );
    }

    #[test]
    fn path_discovery_only_keeps_executables() {
        let root = std::env::temp_dir().join(format!(
            "termsense-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("path")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let executable = root.join("hello");
        let plain = root.join("notes");
        fs::write(&executable, "#!/bin/sh\n").unwrap();
        fs::write(&plain, "text\n").unwrap();

        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        let commands = discover_path_commands(&[root.clone()]);
        assert!(commands.iter().any(|entry| entry.name == "hello"));
        assert!(!commands.iter().any(|entry| entry.name == "notes"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn shell_offsets_convert_utf8_bytes_to_character_positions() {
        let value = "écho git";
        assert_eq!(byte_to_char_offset(value, 2), 1);
        assert_eq!(byte_to_char_offset(value, 6), 5);
    }

    #[test]
    fn shell_display_is_single_line_and_strips_controls() {
        assert_eq!(
            shell_safe_display("one\ntwo\tthree\u{1b}"),
            "one ↵ two ⇥ three�"
        );
    }
}
