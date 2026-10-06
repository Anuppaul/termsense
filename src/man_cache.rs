use std::{
    collections::{hash_map::DefaultHasher, BTreeSet},
    env, fs,
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime},
};
use wait_timeout::ChildExt;

const CACHE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub(crate) fn options(command: &str) -> Vec<String> {
    if !safe_command_name(command) {
        return Vec::new();
    }

    if let Some(path) = cache_path(command) {
        if cache_is_fresh(&path) {
            if let Ok(raw) = fs::read_to_string(&path) {
                return parse_cache_lines(&raw);
            }
        }

        let values = refresh_options(command);
        let _ = write_cache(&path, &values);
        return values;
    }

    refresh_options(command)
}

pub(crate) fn cache_root() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir).join("termsense/man-options-v1"));
    }

    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".cache/termsense/man-options-v1"))
}

fn refresh_options(command: &str) -> Vec<String> {
    let Some(output) = run_man(command, 350) else {
        return Vec::new();
    };
    parse_man_options(&output)
}

fn run_man(command: &str, timeout_ms: u64) -> Option<String> {
    let mut child = Command::new("man")
        .args(["--no-hyphenation", "--no-justification", command])
        .env("MANPAGER", "cat")
        .env("PAGER", "cat")
        .env("MANWIDTH", "120")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut output = String::new();
        let mut reader = stdout;
        let _ = reader.read_to_string(&mut output);
        output
    });

    match child.wait_timeout(Duration::from_millis(timeout_ms)).ok()? {
        Some(status) if status.success() => reader.join().ok(),
        Some(_) => {
            let _ = reader.join();
            None
        }
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            None
        }
    }
}

fn parse_man_options(raw: &str) -> Vec<String> {
    let cleaned = remove_overstrikes(raw);
    let mut options = BTreeSet::new();

    for raw_token in cleaned.split_whitespace() {
        for part in raw_token.split(',') {
            if let Some(option) = normalize_option(part) {
                options.insert(option);
            }
        }
    }

    options.into_iter().collect()
}

fn remove_overstrikes(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if chars.peek().is_some_and(|next| *next == '\u{8}') {
            chars.next();
            if let Some(replacement) = chars.next() {
                output.push(replacement);
            } else {
                output.push(ch);
            }
        } else if ch != '\u{8}' {
            output.push(ch);
        }
    }

    output
}

fn normalize_option(token: &str) -> Option<String> {
    let token = token.trim_matches(|ch: char| matches!(ch, '[' | ']' | '(' | ')' | '{' | '}'));

    let start = token.find('-')?;
    let mut value = &token[start..];
    if !value.starts_with('-') {
        return None;
    }

    value = value.trim_end_matches(|ch: char| matches!(ch, ',' | ';' | '.' | ':' | ')' | ']'));

    if let Some(index) = value.find(|ch: char| matches!(ch, '=' | '[' | '<')) {
        let base = &value[..index];
        if base.starts_with("--") && base.len() > 2 {
            value = base;
        }
    }

    if value.starts_with("--") && value.len() > 2 {
        let option = value
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
            .collect::<String>();
        if option.len() > 2 {
            return Some(option);
        }
    }

    if value.starts_with('-')
        && !value.starts_with("--")
        && value.len() >= 2
        && value.as_bytes()[1].is_ascii_alphanumeric()
    {
        return Some(value[..2].to_owned());
    }

    None
}

fn safe_command_name(command: &str) -> bool {
    !command.is_empty()
        && command.len() <= 128
        && command
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+'))
}

fn cache_path(command: &str) -> Option<PathBuf> {
    let root = cache_root()?;
    let mut hasher = DefaultHasher::new();
    command.hash(&mut hasher);
    Some(root.join(format!("{:016x}.txt", hasher.finish())))
}

fn cache_is_fresh(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return false;
    };
    age <= CACHE_MAX_AGE
}

fn parse_cache_lines(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|line| line.starts_with('-') && line.len() <= 128)
        .map(str::to_owned)
        .collect()
}

fn write_cache(path: &Path, options: &[String]) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err("invalid man option cache path".into());
    };
    fs::create_dir_all(parent).map_err(|err| format!("create man cache directory: {err}"))?;

    let mut body = options.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }

    let mut temp = path.to_path_buf();
    temp.set_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temp, body).map_err(|err| format!("write man option cache: {err}"))?;
    fs::rename(&temp, path).map_err(|err| format!("replace man option cache: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{normalize_option, parse_man_options, safe_command_name};

    #[test]
    fn extracts_common_long_and_short_options() {
        let options = parse_man_options("  -a, --all    show all\n  --color=WHEN\n  -q");
        assert!(options.contains(&"-a".to_owned()));
        assert!(options.contains(&"--all".to_owned()));
        assert!(options.contains(&"--color".to_owned()));
        assert!(options.contains(&"-q".to_owned()));
    }

    #[test]
    fn normalizes_man_option_tokens() {
        assert_eq!(
            normalize_option("--output=FILE").as_deref(),
            Some("--output")
        );
        assert_eq!(normalize_option("[-v,").as_deref(), Some("-v"));
    }

    #[test]
    fn rejects_shell_syntax_as_command_name() {
        assert!(safe_command_name("rsync"));
        assert!(!safe_command_name("foo;rm"));
        assert!(!safe_command_name("../foo"));
    }
}
