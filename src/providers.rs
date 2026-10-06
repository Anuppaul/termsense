use crate::CommandEntry;
use std::{
    collections::BTreeSet,
    env, fs,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use wait_timeout::ChildExt;

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Candidate {
    pub(crate) insert_text: String,
    pub(crate) display_text: String,
    pub(crate) kind: &'static str,
    pub(crate) source: &'static str,
    pub(crate) score: i64,
    pub(crate) replacement_start: usize,
    pub(crate) replacement_end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Token<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

const GIT_SUBCOMMANDS: &[&str] = &[
    "add", "bisect", "branch", "check-attr", "check-ignore", "check-ref-format",
    "checkout", "cherry-pick", "clone", "commit", "diff", "fetch", "grep", "init",
    "log", "merge", "mv", "pull", "push", "rebase", "remote", "reset", "restore",
    "revert", "rm", "show", "stash", "status", "switch", "tag", "worktree",
];

const SYSTEMCTL_SUBCOMMANDS: &[&str] = &[
    "cat", "daemon-reload", "default", "disable", "edit", "emergency", "enable",
    "is-active", "is-enabled", "is-failed", "isolate", "kill", "list-unit-files",
    "list-units", "mask", "preset", "reenable", "reload", "reload-or-restart",
    "rescue", "reset-failed", "restart", "show", "start", "status", "stop",
    "try-restart", "unmask",
];

const DOCKER_SUBCOMMANDS: &[&str] = &[
    "build", "compose", "container", "context", "cp", "create", "exec", "image",
    "images", "info", "inspect", "kill", "login", "logout", "logs", "network",
    "port", "ps", "pull", "push", "rename", "restart", "rm", "rmi", "run",
    "search", "start", "stats", "stop", "system", "tag", "top", "version",
    "volume", "wait",
];

const SYSTEMCTL_UNIT_COMMANDS: &[&str] = &[
    "cat", "disable", "edit", "enable", "is-active", "is-enabled", "is-failed",
    "kill", "mask", "reenable", "reload", "reload-or-restart", "reset-failed",
    "restart", "show", "start", "status", "stop", "try-restart", "unmask",
];

const DOCKER_CONTAINER_COMMANDS: &[&str] = &[
    "cp", "exec", "inspect", "kill", "logs", "port", "rename", "restart", "rm",
    "start", "stats", "stop", "top", "wait",
];

const GIT_REF_COMMANDS: &[&str] = &[
    "branch", "checkout", "merge", "rebase", "reset", "restore", "show", "switch",
];

pub(crate) fn suggest(
    commands: &[CommandEntry],
    buffer: &str,
    cursor: usize,
    limit: usize,
) -> Vec<Candidate> {
    if limit == 0 {
        return Vec::new();
    }

    let tokens = tokens_before_cursor(buffer, cursor);
    let effective = strip_sudo(&tokens);
    let mut candidates = Vec::new();

    if effective.len() <= 1 {
        let token = effective.first().copied().unwrap_or(Token {
            text: "",
            start: cursor,
            end: cursor,
        });
        add_path_commands(&mut candidates, commands, token);
        return finish(candidates, limit);
    }

    let command = effective[0].text;
    let current = *effective.last().expect("effective tokens is non-empty");

    match command {
        "git" => add_git_candidates(&mut candidates, effective, current),
        "systemctl" => add_systemctl_candidates(&mut candidates, effective, current),
        "docker" => add_docker_candidates(&mut candidates, effective, current),
        "cd" => add_directory_candidates(&mut candidates, current),
        _ => {}
    }

    finish(candidates, limit)
}

fn tokens_before_cursor(buffer: &str, cursor: usize) -> Vec<Token<'_>> {
    let before = &buffer[..cursor];
    let bytes = before.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        tokens.push(Token {
            text: &before[start..i],
            start,
            end: i,
        });
    }

    if before.is_empty() || before.as_bytes().last().is_some_and(u8::is_ascii_whitespace) {
        tokens.push(Token {
            text: "",
            start: cursor,
            end: cursor,
        });
    }

    tokens
}

fn strip_sudo<'a>(tokens: &'a [Token<'a>]) -> &'a [Token<'a>] {
    if tokens.len() >= 2 && tokens.first().is_some_and(|token| token.text == "sudo") {
        &tokens[1..]
    } else {
        tokens
    }
}

fn add_path_commands(out: &mut Vec<Candidate>, commands: &[CommandEntry], token: Token<'_>) {
    for entry in commands {
        push_match(
            out,
            &entry.name,
            &entry.name,
            token.text,
            "command",
            "path",
            token.start,
            token.end,
            0,
        );
    }
}

fn add_git_candidates(out: &mut Vec<Candidate>, tokens: &[Token<'_>], current: Token<'_>) {
    if tokens.len() == 2 {
        add_static(out, GIT_SUBCOMMANDS, current, "subcommand", "git-schema", 500);
        return;
    }

    let subcommand = tokens[1].text;
    if tokens.len() == 3 && GIT_REF_COMMANDS.contains(&subcommand) {
        if let Some(output) = run_bounded(
            "git",
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads",
                "refs/remotes",
                "refs/tags",
            ],
            180,
        ) {
            for line in output.lines().map(str::trim).filter(|line| !line.is_empty()) {
                if line.ends_with("/HEAD") {
                    continue;
                }
                push_match(
                    out,
                    line,
                    line,
                    current.text,
                    "git-ref",
                    "git-local",
                    current.start,
                    current.end,
                    700,
                );
            }
        }
    }
}

fn add_systemctl_candidates(
    out: &mut Vec<Candidate>,
    tokens: &[Token<'_>],
    current: Token<'_>,
) {
    if tokens.len() == 2 {
        add_static(
            out,
            SYSTEMCTL_SUBCOMMANDS,
            current,
            "subcommand",
            "systemctl-schema",
            500,
        );
        return;
    }

    let subcommand = tokens[1].text;
    if tokens.len() == 3 && SYSTEMCTL_UNIT_COMMANDS.contains(&subcommand) {
        for unit in systemd_units() {
            push_match(
                out,
                &unit,
                &unit,
                current.text,
                "systemd-unit",
                "systemd-local",
                current.start,
                current.end,
                650,
            );
        }
    }
}

fn add_docker_candidates(
    out: &mut Vec<Candidate>,
    tokens: &[Token<'_>],
    current: Token<'_>,
) {
    if tokens.len() == 2 {
        add_static(
            out,
            DOCKER_SUBCOMMANDS,
            current,
            "subcommand",
            "docker-schema",
            500,
        );
        return;
    }

    let subcommand = tokens[1].text;
    if tokens.len() == 3 && DOCKER_CONTAINER_COMMANDS.contains(&subcommand) {
        if let Some(output) = run_bounded("docker", &["ps", "-a", "--format", "{{.Names}}"], 220) {
            for name in output.lines().map(str::trim).filter(|line| !line.is_empty()) {
                push_match(
                    out,
                    name,
                    name,
                    current.text,
                    "container",
                    "docker-local",
                    current.start,
                    current.end,
                    700,
                );
            }
        }
    }
}

fn add_directory_candidates(out: &mut Vec<Candidate>, current: Token<'_>) {
    let Some((lookup_parent, typed_parent, base)) = directory_query(current.text) else {
        return;
    };

    let Ok(entries) = fs::read_dir(&lookup_parent) else {
        return;
    };

    let show_hidden = base.starts_with('.');
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() && !file_type.is_symlink() {
            continue;
        }

        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        if !name.starts_with(&base) {
            continue;
        }

        let display = format!("{typed_parent}{name}/");
        let insert = format!("{typed_parent}{}/", escape_path_component(&name));
        push_match(
            out,
            &insert,
            &display,
            current.text,
            "directory",
            "filesystem",
            current.start,
            current.end,
            700,
        );
    }
}

fn directory_query(input: &str) -> Option<(PathBuf, String, String)> {
    let (typed_parent, base) = match input.rfind('/') {
        Some(index) => (input[..=index].to_owned(), input[index + 1..].to_owned()),
        None => (String::new(), input.to_owned()),
    };

    let lookup_parent = if typed_parent.is_empty() {
        env::current_dir().ok()?
    } else if typed_parent == "~/" {
        PathBuf::from(env::var_os("HOME")?)
    } else if let Some(rest) = typed_parent.strip_prefix("~/") {
        PathBuf::from(env::var_os("HOME")?).join(rest)
    } else {
        PathBuf::from(&typed_parent)
    };

    Some((lookup_parent, typed_parent, base))
}

fn escape_path_component(name: &str) -> String {
    let mut escaped = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_whitespace()
            || ch == char::from(96u8)
            || matches!(
                ch,
                '\\' | '\'' | '"' | '$' | '!' | '&' | ';' | '|' | '<' | '>' | '('
                    | ')' | '[' | ']' | '{' | '}' | '*' | '?' | '#'
            )
        {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

fn systemd_units() -> Vec<String> {
    const DIRS: &[&str] = &[
        "/etc/systemd/system",
        "/run/systemd/system",
        "/usr/local/lib/systemd/system",
        "/usr/lib/systemd/system",
        "/lib/systemd/system",
    ];

    let mut units = BTreeSet::new();
    for dir in DIRS {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if is_unit_name(&name) {
                units.insert(name);
            }
        }
    }
    units.into_iter().collect()
}

fn is_unit_name(name: &str) -> bool {
    [
        ".service", ".socket", ".target", ".timer", ".mount", ".automount", ".path",
        ".slice", ".scope",
    ]
    .iter()
    .any(|suffix| name.ends_with(suffix))
}

fn add_static(
    out: &mut Vec<Candidate>,
    values: &[&str],
    current: Token<'_>,
    kind: &'static str,
    source: &'static str,
    boost: i64,
) {
    for value in values {
        push_match(
            out,
            value,
            value,
            current.text,
            kind,
            source,
            current.start,
            current.end,
            boost,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn push_match(
    out: &mut Vec<Candidate>,
    insert_text: &str,
    display_text: &str,
    prefix: &str,
    kind: &'static str,
    source: &'static str,
    replacement_start: usize,
    replacement_end: usize,
    boost: i64,
) {
    if let Some(score) = score_prefix(display_text, prefix) {
        out.push(Candidate {
            insert_text: insert_text.to_owned(),
            display_text: display_text.to_owned(),
            kind,
            source,
            score: score + boost,
            replacement_start,
            replacement_end,
        });
    }
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

fn finish(mut candidates: Vec<Candidate>, limit: usize) -> Vec<Candidate> {
    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.display_text.len().cmp(&b.display_text.len()))
            .then_with(|| a.display_text.cmp(&b.display_text))
    });

    let mut seen = BTreeSet::new();
    candidates.retain(|candidate| seen.insert(candidate.insert_text.clone()));
    candidates.truncate(limit);
    candidates
}

fn run_bounded(program: &str, args: &[&str], timeout_ms: u64) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
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

#[cfg(test)]
mod tests {
    use super::{directory_query, score_prefix, strip_sudo, tokens_before_cursor};

    #[test]
    fn tokenizes_trailing_argument_position() {
        let tokens = tokens_before_cursor("git checkout ", 13);
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[2].text, "");
        assert_eq!(tokens[2].start, 13);
    }

    #[test]
    fn strips_sudo_for_context_routing() {
        let tokens = tokens_before_cursor("sudo git che", 12);
        let effective = strip_sudo(&tokens);
        assert_eq!(effective[0].text, "git");
        assert_eq!(effective[1].text, "che");
    }

    #[test]
    fn exact_prefix_scores_above_longer_match() {
        assert!(score_prefix("git", "git").unwrap() > score_prefix("gitk", "git").unwrap());
    }

    #[test]
    fn directory_query_preserves_tilde_prefix() {
        let (_, typed_parent, base) = directory_query("~/Doc").unwrap();
        assert_eq!(typed_parent, "~/");
        assert_eq!(base, "Doc");
    }
}
