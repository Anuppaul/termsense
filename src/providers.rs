use crate::{
    apt_cache, man_cache, runtime_cache,
    shell_parse::{active_context, quote_candidate, Token},
    usage::UsageState,
    CommandEntry,
};
use std::{
    collections::BTreeSet,
    env, fs,
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use wait_timeout::ChildExt;

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Candidate {
    pub(crate) insert_text: String,
    pub(crate) display_text: String,
    pub(crate) description: String,
    pub(crate) kind: &'static str,
    pub(crate) source: &'static str,
    pub(crate) score: i64,
    pub(crate) replacement_start: usize,
    pub(crate) replacement_end: usize,
    pub(crate) usage_key: String,
}

const GIT_SUBCOMMANDS: &[&str] = &[
    "add",
    "bisect",
    "branch",
    "check-attr",
    "check-ignore",
    "check-ref-format",
    "checkout",
    "cherry-pick",
    "clone",
    "commit",
    "diff",
    "fetch",
    "grep",
    "init",
    "log",
    "merge",
    "mv",
    "pull",
    "push",
    "rebase",
    "remote",
    "reset",
    "restore",
    "revert",
    "rm",
    "show",
    "stash",
    "status",
    "switch",
    "tag",
    "worktree",
];

const GIT_COMMIT_OPTIONS: &[&str] = &[
    "--all",
    "--amend",
    "--author=",
    "--date=",
    "--dry-run",
    "--edit",
    "--message=",
    "--no-edit",
    "--no-verify",
    "--only",
    "--patch",
    "--quiet",
    "--reuse-message=",
    "--signoff",
    "--verbose",
];

const GIT_CHECKOUT_OPTIONS: &[&str] = &[
    "--conflict=",
    "--detach",
    "--force",
    "--guess",
    "--ignore-other-worktrees",
    "--merge",
    "--orphan=",
    "--ours",
    "--patch",
    "--quiet",
    "--theirs",
    "--track",
];

const GIT_SWITCH_OPTIONS: &[&str] = &[
    "--create=",
    "--detach",
    "--discard-changes",
    "--force-create=",
    "--guess",
    "--merge",
    "--no-guess",
    "--orphan=",
    "--quiet",
    "--track",
];

const GIT_LOG_OPTIONS: &[&str] = &[
    "--all",
    "--author=",
    "--decorate",
    "--follow",
    "--graph",
    "--max-count=",
    "--oneline",
    "--patch",
    "--since=",
    "--stat",
    "--until=",
];

const SYSTEMCTL_SUBCOMMANDS: &[&str] = &[
    "cat",
    "daemon-reload",
    "default",
    "disable",
    "edit",
    "emergency",
    "enable",
    "is-active",
    "is-enabled",
    "is-failed",
    "isolate",
    "kill",
    "list-unit-files",
    "list-units",
    "mask",
    "preset",
    "reenable",
    "reload",
    "reload-or-restart",
    "rescue",
    "reset-failed",
    "restart",
    "show",
    "start",
    "status",
    "stop",
    "try-restart",
    "unmask",
];

const SYSTEMCTL_GLOBAL_OPTIONS: &[&str] = &[
    "--all",
    "--failed",
    "--force",
    "--full",
    "--global",
    "--help",
    "--no-ask-password",
    "--no-block",
    "--no-legend",
    "--no-pager",
    "--now",
    "--quiet",
    "--runtime",
    "--system",
    "--type=",
    "--user",
    "--version",
];

const DOCKER_SUBCOMMANDS: &[&str] = &[
    "build",
    "compose",
    "container",
    "context",
    "cp",
    "create",
    "exec",
    "image",
    "images",
    "info",
    "inspect",
    "kill",
    "login",
    "logout",
    "logs",
    "network",
    "port",
    "ps",
    "pull",
    "push",
    "rename",
    "restart",
    "rm",
    "rmi",
    "run",
    "search",
    "start",
    "stats",
    "stop",
    "system",
    "tag",
    "top",
    "version",
    "volume",
    "wait",
];

const DOCKER_LOGS_OPTIONS: &[&str] = &[
    "--details",
    "--follow",
    "--since=",
    "--tail=",
    "--timestamps",
    "--until=",
];

const DOCKER_PS_OPTIONS: &[&str] = &[
    "--all",
    "--filter=",
    "--format=",
    "--last=",
    "--latest",
    "--no-trunc",
    "--quiet",
    "--size",
];

const DOCKER_EXEC_OPTIONS: &[&str] = &[
    "--detach",
    "--env=",
    "--interactive",
    "--privileged",
    "--tty",
    "--user=",
    "--workdir=",
];

const SYSTEMCTL_UNIT_COMMANDS: &[&str] = &[
    "cat",
    "disable",
    "edit",
    "enable",
    "is-active",
    "is-enabled",
    "is-failed",
    "kill",
    "mask",
    "reenable",
    "reload",
    "reload-or-restart",
    "reset-failed",
    "restart",
    "show",
    "start",
    "status",
    "stop",
    "try-restart",
    "unmask",
];

const DOCKER_CONTAINER_COMMANDS: &[&str] = &[
    "cp", "exec", "inspect", "kill", "logs", "port", "rename", "restart", "rm", "start", "stats",
    "stop", "top", "wait",
];

const GIT_REF_COMMANDS: &[&str] = &[
    "branch", "checkout", "merge", "rebase", "reset", "restore", "show", "switch",
];

const CARGO_SUBCOMMANDS: &[&str] = &[
    "add",
    "bench",
    "build",
    "check",
    "clean",
    "doc",
    "fetch",
    "fix",
    "generate-lockfile",
    "help",
    "init",
    "install",
    "locate-project",
    "login",
    "metadata",
    "new",
    "owner",
    "package",
    "publish",
    "remove",
    "report",
    "run",
    "rustc",
    "search",
    "test",
    "tree",
    "uninstall",
    "update",
    "vendor",
    "verify-project",
    "version",
    "yank",
];

const CARGO_BUILD_OPTIONS: &[&str] = &[
    "--all-features",
    "--examples",
    "--features=",
    "--jobs=",
    "--locked",
    "--offline",
    "--package=",
    "--profile=",
    "--quiet",
    "--release",
    "--target=",
    "--verbose",
    "--workspace",
];

const CARGO_TEST_OPTIONS: &[&str] = &[
    "--all-features",
    "--doc",
    "--features=",
    "--jobs=",
    "--lib",
    "--locked",
    "--no-run",
    "--offline",
    "--package=",
    "--quiet",
    "--release",
    "--test=",
    "--tests",
    "--verbose",
    "--workspace",
];

const PNPM_SUBCOMMANDS: &[&str] = &[
    "add", "audit", "create", "deploy", "dlx", "exec", "fetch", "import", "init", "install",
    "link", "list", "outdated", "patch", "prune", "publish", "rebuild", "remove", "run", "store",
    "update", "why",
];

const NPM_SUBCOMMANDS: &[&str] = &[
    "access",
    "adduser",
    "audit",
    "cache",
    "ci",
    "config",
    "dedupe",
    "diff",
    "dist-tag",
    "docs",
    "doctor",
    "exec",
    "explain",
    "explore",
    "help",
    "hook",
    "init",
    "install",
    "link",
    "login",
    "logout",
    "ls",
    "outdated",
    "owner",
    "pack",
    "ping",
    "pkg",
    "prefix",
    "profile",
    "prune",
    "publish",
    "query",
    "rebuild",
    "repo",
    "restart",
    "root",
    "run",
    "search",
    "shrinkwrap",
    "star",
    "stars",
    "start",
    "stop",
    "team",
    "test",
    "token",
    "uninstall",
    "unpublish",
    "unstar",
    "update",
    "version",
    "view",
    "whoami",
];

const YARN_SUBCOMMANDS: &[&str] = &[
    "add",
    "bin",
    "cache",
    "config",
    "create",
    "dedupe",
    "dlx",
    "exec",
    "info",
    "init",
    "install",
    "link",
    "node",
    "npm",
    "pack",
    "patch",
    "plugin",
    "rebuild",
    "remove",
    "run",
    "set",
    "stage",
    "unlink",
    "up",
    "why",
    "workspace",
    "workspaces",
];

const BUN_SUBCOMMANDS: &[&str] = &[
    "add", "build", "create", "install", "link", "pm", "publish", "remove", "repl", "run", "test",
    "unlink", "update", "upgrade", "x",
];

const FILESYSTEM_COMMANDS: &[&str] = &[
    "cat", "chmod", "chown", "cp", "du", "file", "head", "less", "ls", "mkdir", "more", "mv",
    "nano", "readlink", "realpath", "rm", "rmdir", "stat", "tail", "touch", "vi", "vim",
];

const APT_SUBCOMMANDS: &[&str] = &[
    "autoremove",
    "download",
    "edit-sources",
    "full-upgrade",
    "install",
    "list",
    "purge",
    "reinstall",
    "remove",
    "satisfy",
    "search",
    "show",
    "update",
    "upgrade",
];

const APT_PACKAGE_COMMANDS: &[&str] = &[
    "download",
    "install",
    "purge",
    "reinstall",
    "remove",
    "show",
];

const APT_OPTIONS: &[&str] = &[
    "--assume-yes",
    "--download-only",
    "--fix-broken",
    "--no-install-recommends",
    "--only-upgrade",
    "--purge",
    "--quiet",
    "--reinstall",
    "--simulate",
    "--yes",
];

const JOURNALCTL_OPTIONS: &[&str] = &[
    "--boot",
    "--catalog",
    "--disk-usage",
    "--follow",
    "--grep=",
    "--lines=",
    "--list-boots",
    "--no-hostname",
    "--no-pager",
    "--output=",
    "--priority=",
    "--reverse",
    "--since=",
    "--system",
    "--unit=",
    "--until=",
    "--user",
    "--user-unit=",
];

const SSH_OPTIONS: &[&str] = &[
    "-4", "-6", "-A", "-a", "-C", "-F", "-f", "-i", "-J", "-L", "-N", "-n", "-o", "-p", "-R", "-T",
    "-t", "-v",
];

const SUDO_OPTIONS: &[&str] = &[
    "-A",
    "-b",
    "-C",
    "-D",
    "-E",
    "-e",
    "-g",
    "-H",
    "-h",
    "-i",
    "-K",
    "-k",
    "-l",
    "-n",
    "-P",
    "-p",
    "-R",
    "-S",
    "-s",
    "-T",
    "-U",
    "-u",
    "-V",
    "--chdir=",
    "--close-from=",
    "--command-timeout=",
    "--group=",
    "--host=",
    "--login",
    "--non-interactive",
    "--preserve-env",
    "--prompt=",
    "--remove-timestamp",
    "--reset-timestamp",
    "--shell",
    "--user=",
];

const FIND_OPTIONS: &[&str] = &[
    "-amin",
    "-anewer",
    "-atime",
    "-cmin",
    "-cnewer",
    "-ctime",
    "-delete",
    "-depth",
    "-empty",
    "-exec",
    "-executable",
    "-false",
    "-gid",
    "-group",
    "-iname",
    "-inum",
    "-ipath",
    "-iregex",
    "-links",
    "-lname",
    "-ls",
    "-maxdepth",
    "-mindepth",
    "-mmin",
    "-mount",
    "-mtime",
    "-name",
    "-newer",
    "-nogroup",
    "-nouser",
    "-path",
    "-perm",
    "-print",
    "-print0",
    "-prune",
    "-readable",
    "-regex",
    "-samefile",
    "-size",
    "-type",
    "-uid",
    "-user",
    "-xdev",
];

const GREP_OPTIONS: &[&str] = &[
    "--after-context=",
    "--before-context=",
    "--binary-files=",
    "--color=",
    "--context=",
    "--count",
    "--exclude=",
    "--exclude-dir=",
    "--files-with-matches",
    "--fixed-strings",
    "--ignore-case",
    "--include=",
    "--invert-match",
    "--line-number",
    "--max-count=",
    "--no-filename",
    "--only-matching",
    "--quiet",
    "--recursive",
    "--word-regexp",
    "--file=",
    "-E",
    "-F",
    "-H",
    "-I",
    "-L",
    "-f",
    "-l",
    "-n",
    "-o",
    "-q",
    "-r",
    "-R",
    "-v",
    "-w",
];

const TAR_OPTIONS: &[&str] = &[
    "--append",
    "--create",
    "--delete",
    "--directory=",
    "--exclude=",
    "--extract",
    "--file=",
    "--gzip",
    "--list",
    "--verbose",
    "--xz",
    "-cf",
    "-cvf",
    "-tf",
    "-tvf",
    "-xf",
    "-xvf",
];

const CURL_OPTIONS: &[&str] = &[
    "--cacert",
    "--compressed",
    "--connect-timeout",
    "--data",
    "--data-raw",
    "--fail",
    "--follow",
    "--form",
    "--head",
    "--header",
    "--include",
    "--insecure",
    "--location",
    "--max-time",
    "--output",
    "--proxy",
    "--request",
    "--retry",
    "--silent",
    "--user",
    "--user-agent",
    "--verbose",
    "-H",
    "-I",
    "-L",
    "-X",
    "-d",
    "-f",
    "-o",
    "-s",
    "-u",
    "-v",
];

const COMMAND_LOOKUP_OPTIONS: &[&str] = &["--all", "--help", "--version", "-a", "-v", "-V"];

const TERMSENSE_SUBCOMMANDS: &[&str] = &[
    "doctor",
    "index",
    "init",
    "list-commands",
    "status",
    "suggest",
];

const TERMSENSE_SUGGEST_OPTIONS: &[&str] = &["--cursor", "--json", "--limit", "-n"];

pub(crate) fn suggest(
    commands: &[CommandEntry],
    usage: &UsageState,
    buffer: &str,
    cursor: usize,
    limit: usize,
) -> Vec<Candidate> {
    let context = active_context(buffer, cursor);
    if context.suppress_suggestions {
        return Vec::new();
    }

    let tokens = context.tokens;
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    if context.redirection_target {
        let current = tokens.last().expect("redirection target token");
        add_filesystem_candidates(&mut candidates, current, false);
        return finalize(candidates, usage, limit, buffer, cursor);
    }
    if tokens.first().is_some_and(|token| token.text == "sudo")
        && add_sudo_value_candidates(&mut candidates, &tokens)
    {
        return finalize(candidates, usage, limit, buffer, cursor);
    }

    let effective_start = sudo_nested_command_index(&tokens).unwrap_or(0);
    let effective = &tokens[effective_start..];

    if effective.is_empty() || effective.len() <= 1 {
        let current = effective.last().or_else(|| tokens.last()).expect("token");
        add_command_candidates(&mut candidates, commands, current);
        return finalize(candidates, usage, limit, buffer, cursor);
    }

    let command = effective[0].text.as_str();
    let current = effective.last().expect("effective tokens is non-empty");

    match command {
        "git" => add_git_candidates(&mut candidates, effective, current),
        "systemctl" => add_systemctl_candidates(&mut candidates, effective, current),
        "docker" => add_docker_candidates(&mut candidates, effective, current),
        "cd" => add_directory_candidates(&mut candidates, current),
        "cargo" => add_cargo_candidates(&mut candidates, effective, current),
        "pnpm" => add_package_manager_candidates(
            &mut candidates,
            effective,
            current,
            PNPM_SUBCOMMANDS,
            "pnpm",
        ),
        "npm" => add_package_manager_candidates(
            &mut candidates,
            effective,
            current,
            NPM_SUBCOMMANDS,
            "npm",
        ),
        "yarn" => add_package_manager_candidates(
            &mut candidates,
            effective,
            current,
            YARN_SUBCOMMANDS,
            "yarn",
        ),
        "bun" => add_package_manager_candidates(
            &mut candidates,
            effective,
            current,
            BUN_SUBCOMMANDS,
            "bun",
        ),
        "make" => add_make_targets(&mut candidates, effective, current),
        "apt" | "apt-get" => add_apt_candidates(&mut candidates, effective, current),
        "journalctl" => add_journalctl_candidates(&mut candidates, effective, current),
        "ssh" => add_ssh_candidates(&mut candidates, effective, current),
        "find" => add_find_candidates(&mut candidates, effective, current),
        "grep" | "egrep" | "fgrep" => add_grep_candidates(&mut candidates, effective, current),
        "tar" => add_tar_candidates(&mut candidates, effective, current),
        "curl" => add_curl_candidates(&mut candidates, effective, current),
        "which" | "whereis" | "type" | "command" | "man" => {
            add_command_lookup_candidates(&mut candidates, commands, effective, current)
        }
        "termsense" => add_termsense_candidates(&mut candidates, effective, current),
        command if FILESYSTEM_COMMANDS.contains(&command) => {
            add_filesystem_candidates(&mut candidates, current, false)
        }
        _ => {}
    }

    if candidates.is_empty() && current.text.starts_with('-') {
        add_man_option_candidates(&mut candidates, command, current);
    }

    finalize(candidates, usage, limit, buffer, cursor)
}

fn add_man_option_candidates(out: &mut Vec<Candidate>, command: &str, current: &Token) {
    for option in man_cache::options(command) {
        push_match(
            out,
            &option,
            &option,
            &current.text,
            "option",
            "man-cache",
            current.start,
            current.end,
            350,
        );
    }
}

fn add_command_candidates(out: &mut Vec<Candidate>, commands: &[CommandEntry], current: &Token) {
    for entry in commands {
        push_match(
            out,
            &entry.name,
            &entry.name,
            &current.text,
            "command",
            "path",
            current.start,
            current.end,
            0,
        );
    }

    add_shell_names(
        out,
        current,
        "TERMSENSE_SHELL_BUILTINS",
        "shell-builtin",
        30,
    );
    add_shell_names(out, current, "TERMSENSE_SHELL_ALIASES", "shell-alias", 20);
    add_shell_names(
        out,
        current,
        "TERMSENSE_SHELL_FUNCTIONS",
        "shell-function",
        20,
    );
}

fn add_shell_names(
    out: &mut Vec<Candidate>,
    current: &Token,
    variable: &str,
    source: &'static str,
    boost: i64,
) {
    let Ok(raw) = env::var(variable) else {
        return;
    };

    for name in raw.lines().map(str::trim).filter(|name| !name.is_empty()) {
        push_match(
            out,
            name,
            name,
            &current.text,
            "command",
            source,
            current.start,
            current.end,
            boost,
        );
    }
}

fn sudo_nested_command_index(tokens: &[Token]) -> Option<usize> {
    if !tokens.first().is_some_and(|token| token.text == "sudo") {
        return Some(0);
    }

    let mut index = 1;
    while index < tokens.len() {
        let text = tokens[index].text.as_str();

        if text.is_empty() {
            return Some(index);
        }
        if text == "--" {
            return (index + 1 < tokens.len()).then_some(index + 1);
        }
        if sudo_option_takes_value(text) {
            if text.contains('=') {
                index += 1;
            } else {
                index += 2;
            }
            continue;
        }
        if text.starts_with('-') {
            index += 1;
            continue;
        }

        return Some(index);
    }

    None
}

fn add_sudo_value_candidates(out: &mut Vec<Candidate>, tokens: &[Token]) -> bool {
    let Some(current) = tokens.last() else {
        return false;
    };

    if let Some(prefix) = current.text.strip_prefix("--user=") {
        add_assignment_values(
            out,
            current,
            "--user=",
            prefix,
            users(),
            "user",
            "sudo-local",
        );
        return true;
    }
    if let Some(prefix) = current.text.strip_prefix("--group=") {
        add_assignment_values(
            out,
            current,
            "--group=",
            prefix,
            groups(),
            "group",
            "sudo-local",
        );
        return true;
    }

    if tokens.len() >= 2 && current.text.starts_with('-') {
        add_static(out, SUDO_OPTIONS, current, "option", "sudo-schema", 650);
        return true;
    }

    if tokens.len() >= 3 {
        let previous = tokens[tokens.len() - 2].text.as_str();
        if matches!(previous, "-u" | "--user" | "-U" | "--other-user") {
            add_values(out, current, users(), "user", "sudo-local", 750);
            return true;
        }
        if matches!(previous, "-g" | "--group") {
            add_values(out, current, groups(), "group", "sudo-local", 750);
            return true;
        }
    }

    false
}

fn sudo_option_takes_value(value: &str) -> bool {
    matches!(
        value,
        "-u" | "--user"
            | "-U"
            | "-g"
            | "--group"
            | "-h"
            | "--host"
            | "-C"
            | "--close-from"
            | "-T"
            | "--command-timeout"
            | "-D"
            | "--chdir"
            | "-p"
            | "--prompt"
    ) || value.starts_with("--user=")
        || value.starts_with("--group=")
        || value.starts_with("--host=")
        || value.starts_with("--close-from=")
        || value.starts_with("--command-timeout=")
        || value.starts_with("--chdir=")
        || value.starts_with("--prompt=")
}

fn users() -> Vec<String> {
    parse_colon_names("/etc/passwd")
}

fn groups() -> Vec<String> {
    parse_colon_names("/etc/group")
}

fn parse_colon_names(path: &str) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut values = BTreeSet::new();
    for line in raw.lines() {
        let Some((name, _)) = line.split_once(':') else {
            continue;
        };
        if !name.is_empty() {
            values.insert(name.to_owned());
        }
    }
    values.into_iter().collect()
}

fn add_assignment_values(
    out: &mut Vec<Candidate>,
    current: &Token,
    option: &str,
    prefix: &str,
    values: Vec<String>,
    kind: &'static str,
    source: &'static str,
) {
    for value in values {
        if !value.starts_with(prefix) {
            continue;
        }
        let insert = format!("{option}{value}");
        push_match(
            out,
            &insert,
            &insert,
            &current.text,
            kind,
            source,
            current.start,
            current.end,
            750,
        );
    }
}

fn add_values(
    out: &mut Vec<Candidate>,
    current: &Token,
    values: Vec<String>,
    kind: &'static str,
    source: &'static str,
    boost: i64,
) {
    for value in values {
        push_match(
            out,
            &value,
            &value,
            &current.text,
            kind,
            source,
            current.start,
            current.end,
            boost,
        );
    }
}

fn add_git_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if tokens.len() == 2 {
        add_static(
            out,
            GIT_SUBCOMMANDS,
            current,
            "subcommand",
            "git-schema",
            500,
        );
        return;
    }

    let subcommand = tokens[1].text.as_str();
    if current.text.starts_with('-') {
        let options = match subcommand {
            "commit" => GIT_COMMIT_OPTIONS,
            "checkout" => GIT_CHECKOUT_OPTIONS,
            "switch" => GIT_SWITCH_OPTIONS,
            "log" => GIT_LOG_OPTIONS,
            _ => &[],
        };
        let source = match subcommand {
            "commit" => "git-commit-schema",
            "checkout" => "git-checkout-schema",
            "switch" => "git-switch-schema",
            "log" => "git-log-schema",
            _ => "git-schema",
        };
        add_static(out, options, current, "option", source, 650);
        return;
    }

    if tokens.len() == 3 && GIT_REF_COMMANDS.contains(&subcommand) {
        for reference in git_refs() {
            push_match(
                out,
                &reference,
                &reference,
                &current.text,
                "git-ref",
                "git-local",
                current.start,
                current.end,
                700,
            );
        }
    }
}

fn add_systemctl_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if current.text.starts_with('-') {
        add_static(
            out,
            SYSTEMCTL_GLOBAL_OPTIONS,
            current,
            "option",
            "systemctl-schema",
            650,
        );
        return;
    }

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

    let subcommand = tokens[1].text.as_str();
    if tokens.len() == 3 && SYSTEMCTL_UNIT_COMMANDS.contains(&subcommand) {
        for unit in systemd_units() {
            push_match(
                out,
                &unit,
                &unit,
                &current.text,
                "systemd-unit",
                "systemd-local",
                current.start,
                current.end,
                650,
            );
        }
    }
}

fn add_docker_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
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

    let subcommand = tokens[1].text.as_str();
    if current.text.starts_with('-') {
        let options = match subcommand {
            "logs" => DOCKER_LOGS_OPTIONS,
            "ps" => DOCKER_PS_OPTIONS,
            "exec" => DOCKER_EXEC_OPTIONS,
            _ => &[],
        };
        let source = match subcommand {
            "logs" => "docker-logs-schema",
            "ps" => "docker-ps-schema",
            "exec" => "docker-exec-schema",
            _ => "docker-schema",
        };
        add_static(out, options, current, "option", source, 650);
        return;
    }

    if tokens.len() == 3 && DOCKER_CONTAINER_COMMANDS.contains(&subcommand) {
        for name in docker_container_names() {
            push_match(
                out,
                &name,
                &name,
                &current.text,
                "container",
                "docker-local",
                current.start,
                current.end,
                700,
            );
        }
    }
}

fn add_cargo_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if tokens.len() == 2 {
        add_static(
            out,
            CARGO_SUBCOMMANDS,
            current,
            "subcommand",
            "cargo-schema",
            500,
        );
        return;
    }

    if current.text.starts_with('-') {
        let options = match tokens[1].text.as_str() {
            "build" | "check" | "run" => CARGO_BUILD_OPTIONS,
            "test" => CARGO_TEST_OPTIONS,
            _ => &[],
        };
        add_static(out, options, current, "option", "cargo-schema", 650);
    }
}

fn add_package_manager_candidates(
    out: &mut Vec<Candidate>,
    tokens: &[Token],
    current: &Token,
    schema: &[&str],
    manager: &'static str,
) {
    if tokens.len() == 2 {
        add_static(
            out,
            schema,
            current,
            "subcommand",
            "package-manager-schema",
            400,
        );

        if manager != "npm" {
            for script in package_scripts() {
                push_match(
                    out,
                    &script,
                    &script,
                    &current.text,
                    "project-script",
                    "package-json",
                    current.start,
                    current.end,
                    800,
                );
            }
        }
        return;
    }

    if tokens.len() == 3 && tokens[1].text == "run" {
        for script in package_scripts() {
            push_match(
                out,
                &script,
                &script,
                &current.text,
                "project-script",
                "package-json",
                current.start,
                current.end,
                800,
            );
        }
    }
}

fn add_apt_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if current.text.starts_with('-') {
        add_static(out, APT_OPTIONS, current, "option", "apt-schema", 650);
        return;
    }

    if tokens.len() == 2 {
        add_static(
            out,
            APT_SUBCOMMANDS,
            current,
            "subcommand",
            "apt-schema",
            500,
        );
        return;
    }

    if tokens.len() >= 3 && APT_PACKAGE_COMMANDS.contains(&tokens[1].text.as_str()) {
        for package in apt_cache::packages() {
            push_match(
                out,
                &package,
                &package,
                &current.text,
                "package",
                "apt-local-cache",
                current.start,
                current.end,
                700,
            );
        }
    }
}

fn add_journalctl_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if current.text.starts_with("--unit=") {
        add_unit_assignment_candidates(out, current, "--unit=");
        return;
    }
    if current.text.starts_with("--user-unit=") {
        add_unit_assignment_candidates(out, current, "--user-unit=");
        return;
    }

    if current.text.starts_with('-') {
        add_static(
            out,
            JOURNALCTL_OPTIONS,
            current,
            "option",
            "journalctl-schema",
            650,
        );
        return;
    }

    if tokens.len() >= 3 {
        let previous = tokens[tokens.len() - 2].text.as_str();
        if matches!(previous, "-u" | "--unit" | "--user-unit") {
            for unit in systemd_units() {
                push_match(
                    out,
                    &unit,
                    &unit,
                    &current.text,
                    "systemd-unit",
                    "systemd-local",
                    current.start,
                    current.end,
                    700,
                );
            }
        }
    }
}

fn add_unit_assignment_candidates(out: &mut Vec<Candidate>, current: &Token, option: &str) {
    for unit in systemd_units() {
        let insert = format!("{option}{unit}");
        push_match(
            out,
            &insert,
            &insert,
            &current.text,
            "systemd-unit",
            "systemd-local",
            current.start,
            current.end,
            700,
        );
    }
}

fn add_ssh_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if tokens.len() >= 3 {
        let previous = tokens[tokens.len() - 2].text.as_str();
        if matches!(previous, "-i" | "-F") {
            add_filesystem_candidates(out, current, false);
            return;
        }
    }

    if current.text.starts_with('-') {
        add_static(out, SSH_OPTIONS, current, "option", "ssh-schema", 650);
        return;
    }

    if tokens.len() != 2 {
        return;
    }

    let (user_prefix, host_prefix) = match current.text.rsplit_once('@') {
        Some((user, host)) => (format!("{user}@"), host),
        None => (String::new(), current.text.as_str()),
    };

    for host in ssh_hosts() {
        if !host.starts_with(host_prefix) {
            continue;
        }
        let insert = format!("{user_prefix}{host}");
        push_match(
            out,
            &insert,
            &insert,
            &current.text,
            "ssh-host",
            "ssh-local",
            current.start,
            current.end,
            750,
        );
    }
}

fn add_find_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if tokens.len() >= 3 {
        let previous = tokens[tokens.len() - 2].text.as_str();
        match previous {
            "-user" => {
                add_values(out, current, users(), "user", "find-local", 750);
                return;
            }
            "-group" => {
                add_values(out, current, groups(), "group", "find-local", 750);
                return;
            }
            "-type" => {
                add_static(
                    out,
                    &["b", "c", "d", "f", "l", "p", "s"],
                    current,
                    "argument-value",
                    "find-schema",
                    700,
                );
                return;
            }
            "-newer" | "-anewer" | "-cnewer" | "-samefile" => {
                add_filesystem_candidates(out, current, false);
                return;
            }
            _ => {}
        }
    }

    if current.text.starts_with('-') {
        add_static(out, FIND_OPTIONS, current, "option", "find-schema", 650);
        return;
    }

    if tokens.len() == 2 {
        add_filesystem_candidates(out, current, true);
    }
}

fn add_grep_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if current.text.starts_with("--file=") {
        add_path_assignment_candidates(out, current, "--file=");
        return;
    }

    if tokens.len() >= 3 {
        let previous = tokens[tokens.len() - 2].text.as_str();
        if matches!(previous, "-f" | "--file") {
            add_filesystem_candidates(out, current, false);
            return;
        }
    }

    if current.text.starts_with('-') {
        add_static(out, GREP_OPTIONS, current, "option", "grep-schema", 650);
        return;
    }

    if tokens.len() >= 3 {
        let previous = tokens[tokens.len() - 2].text.as_str();
        if previous.starts_with('-') {
            return;
        }
        add_filesystem_candidates(out, current, false);
    }
}

fn add_tar_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if current.text.starts_with("--file=") {
        add_path_assignment_candidates(out, current, "--file=");
        return;
    }
    if current.text.starts_with("--directory=") {
        add_path_assignment_candidates(out, current, "--directory=");
        return;
    }

    if tokens.len() >= 3 {
        let previous = tokens[tokens.len() - 2].text.as_str();
        if matches!(previous, "-f" | "--file" | "-C" | "--directory")
            || (previous.starts_with('-') && previous.contains('f') && !previous.starts_with("--"))
        {
            add_filesystem_candidates(out, current, false);
            return;
        }
    }

    if current.text.starts_with('-') {
        add_static(out, TAR_OPTIONS, current, "option", "tar-schema", 650);
        return;
    }

    if tokens.len() >= 2 {
        add_filesystem_candidates(out, current, false);
    }
}

fn add_curl_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if current.text.starts_with("--output=") {
        add_path_assignment_candidates(out, current, "--output=");
        return;
    }
    if current.text.starts_with("--cacert=") {
        add_path_assignment_candidates(out, current, "--cacert=");
        return;
    }

    if tokens.len() >= 3 {
        let previous = tokens[tokens.len() - 2].text.as_str();
        if matches!(previous, "-o" | "--output" | "--cacert") {
            add_filesystem_candidates(out, current, false);
            return;
        }
        if matches!(
            previous,
            "-H" | "--header" | "-d" | "--data" | "--data-raw" | "-X" | "--request"
        ) {
            return;
        }
    }

    if current.text.starts_with('-') {
        add_static(out, CURL_OPTIONS, current, "option", "curl-schema", 650);
    }
}

fn add_termsense_candidates(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if tokens.len() == 2 {
        add_static(
            out,
            TERMSENSE_SUBCOMMANDS,
            current,
            "subcommand",
            "termsense-schema",
            800,
        );
        return;
    }

    match tokens[1].text.as_str() {
        "init" if tokens.len() == 3 => {
            add_static(
                out,
                &["bash"],
                current,
                "argument-value",
                "termsense-schema",
                800,
            );
        }
        "suggest" if current.text.starts_with('-') => {
            add_static(
                out,
                TERMSENSE_SUGGEST_OPTIONS,
                current,
                "option",
                "termsense-schema",
                800,
            );
        }
        "list-commands" if current.text.starts_with('-') => {
            add_static(out, &["--json"], current, "option", "termsense-schema", 800);
        }
        _ => {}
    }
}

fn add_command_lookup_candidates(
    out: &mut Vec<Candidate>,
    commands: &[CommandEntry],
    _tokens: &[Token],
    current: &Token,
) {
    if current.text.starts_with('-') {
        add_static(
            out,
            COMMAND_LOOKUP_OPTIONS,
            current,
            "option",
            "command-lookup-schema",
            600,
        );
        return;
    }

    add_command_candidates(out, commands, current);
}

fn add_path_assignment_candidates(out: &mut Vec<Candidate>, current: &Token, option: &str) {
    let Some(prefix) = current.text.strip_prefix(option) else {
        return;
    };

    let synthetic = Token {
        text: prefix.to_owned(),
        start: current.start + option.len(),
        end: current.end,
        quote: current.quote,
    };

    let mut path_candidates = Vec::new();
    add_filesystem_candidates(&mut path_candidates, &synthetic, false);

    for candidate in path_candidates {
        let insert = format!("{option}{}", candidate.insert_text);
        push_match(
            out,
            &insert,
            &insert,
            &current.text,
            candidate.kind,
            "filesystem",
            current.start,
            current.end,
            700,
        );
    }
}

fn package_scripts() -> Vec<String> {
    let Some(path) = find_upwards("package.json") else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    let Some(scripts) = value.get("scripts").and_then(|value| value.as_object()) else {
        return Vec::new();
    };

    let mut names: Vec<String> = scripts.keys().cloned().collect();
    names.sort();
    names
}

fn add_make_targets(out: &mut Vec<Candidate>, tokens: &[Token], current: &Token) {
    if tokens.len() != 2 {
        return;
    }

    for target in make_targets() {
        push_match(
            out,
            &target,
            &target,
            &current.text,
            "make-target",
            "makefile",
            current.start,
            current.end,
            800,
        );
    }
}

fn make_targets() -> Vec<String> {
    let manifest = ["GNUmakefile", "Makefile", "makefile"]
        .iter()
        .find_map(|name| find_upwards(name));

    let Some(path) = manifest else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut targets = BTreeSet::new();
    for line in raw.lines() {
        if line.starts_with('\t') || line.trim_start().starts_with('#') {
            continue;
        }

        let Some((left, _)) = line.split_once(':') else {
            continue;
        };

        if left.contains('=') || left.contains('%') || left.contains(char::from(36u8)) {
            continue;
        }

        for target in left.split_whitespace() {
            if target.is_empty() || target.starts_with('.') {
                continue;
            }
            targets.insert(target.to_owned());
        }
    }

    targets.into_iter().collect()
}

fn find_upwards(name: &str) -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;

    for _ in 0..8 {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }

        if !dir.pop() {
            break;
        }
    }

    None
}

fn add_directory_candidates(out: &mut Vec<Candidate>, current: &Token) {
    add_filesystem_candidates(out, current, true);
}

fn add_filesystem_candidates(out: &mut Vec<Candidate>, current: &Token, directories_only: bool) {
    if current.text.starts_with('-') {
        return;
    }

    let Some((lookup_parent, typed_parent, base)) = directory_query(&current.text) else {
        return;
    };

    let Ok(entries) = fs::read_dir(&lookup_parent) else {
        return;
    };

    let show_hidden = base.starts_with('.');
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        if !name.starts_with(&base) {
            continue;
        }

        let is_dir = entry.path().is_dir();
        if directories_only && !is_dir {
            continue;
        }

        let suffix = if is_dir { "/" } else { "" };
        let logical = format!("{typed_parent}{name}{suffix}");
        let Some(insert) = quote_candidate(&logical, current.quote) else {
            continue;
        };
        let kind = if is_dir { "directory" } else { "file" };

        push_match(
            out,
            &insert,
            &logical,
            &current.text,
            kind,
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

fn ssh_hosts() -> Vec<String> {
    if let Some(cached) = runtime_cache::load_lines("ssh-hosts-v1", Duration::from_secs(5)) {
        return cached;
    }

    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };

    let mut hosts = BTreeSet::new();
    let mut visited = BTreeSet::new();
    parse_ssh_config(
        &home.join(".ssh/config"),
        &home,
        &mut hosts,
        &mut visited,
        0,
    );
    parse_known_hosts(&home.join(".ssh/known_hosts"), &mut hosts);

    let values: Vec<String> = hosts.into_iter().collect();
    runtime_cache::store_lines("ssh-hosts-v1", &values);
    values
}

fn parse_ssh_config(
    path: &Path,
    home: &Path,
    hosts: &mut BTreeSet<String>,
    visited: &mut BTreeSet<PathBuf>,
    depth: usize,
) {
    if depth > 6 {
        return;
    }

    let identity = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(identity) {
        return;
    }

    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };

    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(keyword) = parts.next() else {
            continue;
        };

        if keyword.eq_ignore_ascii_case("host") {
            for host in parts {
                if host.starts_with('!') || host.contains('*') || host.contains('?') {
                    continue;
                }
                hosts.insert(host.to_owned());
            }
            continue;
        }

        if keyword.eq_ignore_ascii_case("include") {
            for pattern in parts {
                for include in expand_ssh_include(pattern, base_dir, home) {
                    parse_ssh_config(&include, home, hosts, visited, depth + 1);
                }
            }
        }
    }
}

fn expand_ssh_include(pattern: &str, base_dir: &Path, home: &Path) -> Vec<PathBuf> {
    let expanded = if pattern == "~" {
        home.to_path_buf()
    } else if let Some(rest) = pattern.strip_prefix("~/") {
        home.join(rest)
    } else {
        let candidate = PathBuf::from(pattern);
        if candidate.is_absolute() {
            candidate
        } else {
            base_dir.join(candidate)
        }
    };

    let Some(file_name) = expanded.file_name().and_then(|value| value.to_str()) else {
        return Vec::new();
    };

    if !file_name.contains('*') && !file_name.contains('?') {
        return vec![expanded];
    }

    let parent = expanded.parent().unwrap_or_else(|| Path::new("."));
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if wildcard_match(file_name, &name) && entry.path().is_file() {
            matches.push(entry.path());
        }
    }
    matches.sort();
    matches
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut p = 0;
    let mut v = 0;
    let mut star = None;
    let mut retry = 0;

    while v < value.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == value[v]) {
            p += 1;
            v += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = v;
        } else if let Some(star_index) = star {
            p = star_index + 1;
            retry += 1;
            v = retry;
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }

    p == pattern.len()
}

fn parse_known_hosts(path: &PathBuf, hosts: &mut BTreeSet<String>) {
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut fields = line.split_whitespace();
        let first = match fields.next() {
            Some(value) if value.starts_with('@') => fields.next(),
            Some(value) => Some(value),
            None => None,
        };
        let Some(field) = first else {
            continue;
        };
        if field.starts_with('|') {
            continue;
        }

        for host in field.split(',') {
            let host = host.trim();
            if host.is_empty() {
                continue;
            }

            let normalized = if host.starts_with('[') {
                host.split(']')
                    .next()
                    .map(|value| value.trim_start_matches('['))
            } else {
                Some(host)
            };

            if let Some(value) = normalized.filter(|value| !value.is_empty()) {
                hosts.insert(value.to_owned());
            }
        }
    }
}

fn git_refs() -> Vec<String> {
    let cwd = env::current_dir().unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cwd.hash(&mut hasher);
    let key = format!("git-refs-v1-{:x}", hasher.finish());

    if let Some(cached) = runtime_cache::load_lines(&key, Duration::from_secs(2)) {
        return cached;
    }

    let Some(output) = run_bounded(
        "git",
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
        160,
    ) else {
        runtime_cache::store_lines(&key, &[]);
        return Vec::new();
    };

    let mut refs = BTreeSet::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !line.ends_with("/HEAD") {
            refs.insert(line.to_owned());
        }
    }

    let values: Vec<String> = refs.into_iter().collect();
    runtime_cache::store_lines(&key, &values);
    values
}

fn docker_container_names() -> Vec<String> {
    if let Some(cached) = runtime_cache::load_lines("docker-containers-v1", Duration::from_secs(2))
    {
        return cached;
    }

    let Some(output) = run_bounded("docker", &["ps", "-a", "--format", "{{.Names}}"], 160) else {
        runtime_cache::store_lines("docker-containers-v1", &[]);
        return Vec::new();
    };

    let mut names = BTreeSet::new();
    for name in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        names.insert(name.to_owned());
    }

    let values: Vec<String> = names.into_iter().collect();
    runtime_cache::store_lines("docker-containers-v1", &values);
    values
}

fn systemd_units() -> Vec<String> {
    if let Some(cached) = runtime_cache::load_lines("systemd-units-v1", Duration::from_secs(10)) {
        return cached;
    }

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

    let values: Vec<String> = units.into_iter().collect();
    runtime_cache::store_lines("systemd-units-v1", &values);
    values
}

fn is_unit_name(name: &str) -> bool {
    [
        ".service",
        ".socket",
        ".target",
        ".timer",
        ".mount",
        ".automount",
        ".path",
        ".slice",
        ".scope",
    ]
    .iter()
    .any(|suffix| name.ends_with(suffix))
}

fn add_static(
    out: &mut Vec<Candidate>,
    values: &[&str],
    current: &Token,
    kind: &'static str,
    source: &'static str,
    boost: i64,
) {
    for value in values {
        push_match(
            out,
            value,
            value,
            &current.text,
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
    match_text: &str,
    prefix: &str,
    kind: &'static str,
    source: &'static str,
    replacement_start: usize,
    replacement_end: usize,
    boost: i64,
) {
    if insert_text.chars().any(char::is_control) || match_text.chars().any(char::is_control) {
        return;
    }

    if let Some(score) = score_prefix(match_text, prefix) {
        out.push(Candidate {
            insert_text: insert_text.to_owned(),
            display_text: match_text.to_owned(),
            description: candidate_description(source, kind, insert_text).to_owned(),
            kind,
            source,
            score: score + boost,
            replacement_start,
            replacement_end,
            usage_key: usage_key_for(source, kind, insert_text),
        });
    }
}

fn candidate_description(source: &str, kind: &str, value: &str) -> &'static str {
    match source {
        "path" => match value {
            "apt" | "apt-get" => "Manage Debian/Ubuntu packages",
            "bash" => "Run the Bash shell",
            "cargo" => "Build and manage Rust projects",
            "cat" => "Print file contents",
            "chmod" => "Change file permissions",
            "chown" => "Change file owner or group",
            "cp" => "Copy files and directories",
            "curl" => "Transfer data over URLs",
            "docker" => "Manage Docker containers and images",
            "find" => "Search for files and directories",
            "git" => "Manage Git repositories and history",
            "grep" | "egrep" | "fgrep" => "Search text for matching patterns",
            "head" => "Show the beginning of files",
            "journalctl" => "Read systemd journal logs",
            "less" => "View text one screen at a time",
            "ls" => "List directory contents",
            "make" => "Build targets from a Makefile",
            "man" => "Read command manual pages",
            "mkdir" => "Create directories",
            "more" => "Page through text output",
            "mv" => "Move or rename files",
            "nano" => "Edit text in the terminal",
            "npm" => "Manage Node.js packages and scripts",
            "pnpm" => "Manage Node.js packages efficiently",
            "ps" => "Show running processes",
            "pwd" => "Print the current directory",
            "rm" => "Remove files or directories",
            "rmdir" => "Remove empty directories",
            "rustc" => "Compile Rust source code",
            "sed" => "Transform text streams",
            "ssh" => "Connect securely to a remote host",
            "sudo" => "Run a command with elevated privileges",
            "systemctl" => "Manage systemd services and units",
            "tail" => "Show the end of files",
            "tar" => "Create or extract archives",
            "termsense" => "Context-aware terminal suggestions",
            "top" => "Monitor processes and system load",
            "touch" => "Create files or update timestamps",
            "vi" | "vim" => "Edit text in the terminal",
            "whereis" => "Locate command binaries and manuals",
            "which" => "Show the executable used for a command",
            _ => "Installed executable command",
        },
        "shell-builtin" => match value {
            "cd" => "Change the current directory",
            "echo" => "Print text or variable values",
            "export" => "Set variables for child processes",
            "history" => "Show or manage Bash command history",
            "printf" => "Print formatted text",
            "pwd" => "Print the current directory",
            "read" => "Read input into shell variables",
            "source" | "." => "Run commands from a file in this shell",
            "type" => "Describe how Bash resolves a command",
            "unset" => "Remove shell variables or functions",
            _ => "Bash built-in command",
        },
        "shell-alias" => "Shell alias",
        "shell-function" => "Shell function",
        "git-local" => "Git branch, tag, or remote ref",
        "docker-local" => "Local Docker container",
        "systemd-local" => "Local systemd unit",
        "ssh-local" => "SSH host from local configuration",
        "apt-local-cache" => "APT package from local package data",
        "package-json" => "Project script from package.json",
        "makefile" => "Build target from the local Makefile",
        "filesystem" => match kind {
            "directory" => "Filesystem directory",
            _ => "Filesystem file or path",
        },
        "sudo-local" | "find-local" => match kind {
            "user" => "Local system user",
            "group" => "Local system group",
            _ => "Local system value",
        },
        "man-cache" => "Option discovered from the local man page",
        "git-schema" => match value {
            "add" => "Stage file changes for the next commit",
            "branch" => "List, create, or delete branches",
            "checkout" => "Switch branches or restore files",
            "cherry-pick" => "Apply an existing commit",
            "clone" => "Copy a remote repository locally",
            "commit" => "Record staged changes in Git history",
            "diff" => "Show changes between Git states",
            "fetch" => "Download refs and objects from a remote",
            "init" => "Create a new Git repository",
            "log" => "Show commit history",
            "merge" => "Combine another branch into the current one",
            "pull" => "Fetch and integrate remote changes",
            "push" => "Upload local commits to a remote",
            "rebase" => "Replay commits onto another base",
            "remote" => "Manage remote repositories",
            "reset" => "Move HEAD or reset tracked changes",
            "restore" => "Restore working-tree files",
            "revert" => "Create a commit that undoes another commit",
            "show" => "Show details of a Git object",
            "stash" => "Temporarily save uncommitted changes",
            "status" => "Show working tree and staging status",
            "switch" => "Switch to another branch",
            "tag" => "Create or manage Git tags",
            "worktree" => "Manage multiple working trees",
            _ => "Git subcommand",
        },
        "git-commit-schema" => match value {
            "--amend" => "Replace the previous commit",
            "--message=" => "Set the commit message",
            "--no-verify" => "Skip commit hooks",
            "--signoff" => "Add a Signed-off-by trailer",
            _ => "Git commit option",
        },
        "git-checkout-schema" => "Git checkout option",
        "git-switch-schema" => "Git switch option",
        "git-log-schema" => match value {
            "--graph" => "Draw an ASCII commit graph",
            "--oneline" => "Show each commit on one line",
            "--stat" => "Show changed-file statistics",
            _ => "Git log option",
        },
        "systemctl-schema" => match value {
            "start" => "Start one or more systemd units",
            "stop" => "Stop one or more systemd units",
            "restart" => "Restart one or more systemd units",
            "reload" => "Reload a unit configuration or service",
            "status" => "Show runtime status for a unit",
            "enable" => "Enable a unit to start automatically",
            "disable" => "Disable automatic unit startup",
            "daemon-reload" => "Reload systemd unit definitions",
            "list-units" => "List loaded systemd units",
            "list-unit-files" => "List installed unit files",
            "is-active" => "Check whether a unit is active",
            "is-enabled" => "Check whether a unit is enabled",
            _ if kind == "option" => "systemctl option",
            _ => "systemctl subcommand",
        },
        "docker-schema" => match value {
            "build" => "Build an image from a Dockerfile",
            "compose" => "Manage multi-container applications",
            "cp" => "Copy files between host and container",
            "exec" => "Run a command inside a running container",
            "images" => "List local Docker images",
            "info" => "Show Docker system information",
            "inspect" => "Show low-level Docker object details",
            "logs" => "Show container logs",
            "ps" => "List Docker containers",
            "pull" => "Download an image from a registry",
            "push" => "Upload an image to a registry",
            "restart" => "Restart one or more containers",
            "rm" => "Remove containers",
            "rmi" => "Remove images",
            "run" => "Create and run a new container",
            "start" => "Start stopped containers",
            "stats" => "Show live container resource usage",
            "stop" => "Stop running containers",
            "volume" => "Manage Docker volumes",
            _ => "Docker subcommand",
        },
        "docker-logs-schema" => match value {
            "--follow" => "Follow new container log output",
            "--tail=" => "Show only the last N log lines",
            "--since=" => "Show logs newer than a time",
            _ => "Docker logs option",
        },
        "docker-ps-schema" => "Docker container-list option",
        "docker-exec-schema" => "Docker exec option",
        "cargo-schema" => match value {
            "add" => "Add a dependency to Cargo.toml",
            "build" => "Compile the current Rust project",
            "check" => "Type-check Rust code without full output",
            "clean" => "Remove Cargo build artifacts",
            "doc" => "Build Rust API documentation",
            "init" => "Create a Cargo project in this directory",
            "install" => "Install a Rust binary crate",
            "new" => "Create a new Cargo project",
            "remove" => "Remove a dependency from Cargo.toml",
            "run" => "Build and run the current Rust binary",
            "test" => "Build and run Rust tests",
            "tree" => "Display the dependency graph",
            "update" => "Update dependencies in Cargo.lock",
            _ if kind == "option" => "Cargo option",
            _ => "Cargo subcommand",
        },
        "package-manager-schema" => match value {
            "add" | "install" => "Install project dependencies",
            "remove" | "uninstall" => "Remove a project dependency",
            "run" => "Run a project package script",
            "test" => "Run the project test script",
            "update" | "up" => "Update project dependencies",
            "outdated" => "Show outdated dependencies",
            "audit" => "Check dependencies for known vulnerabilities",
            _ => "Package-manager subcommand",
        },
        "apt-schema" => match value {
            "install" => "Install one or more APT packages",
            "remove" => "Remove installed packages",
            "purge" => "Remove packages and configuration files",
            "update" => "Refresh package index metadata",
            "upgrade" => "Upgrade installed packages",
            "autoremove" => "Remove unneeded dependency packages",
            "search" => "Search available APT packages",
            "show" => "Show package details",
            "list" => "List packages matching a pattern",
            _ if kind == "option" => "APT option",
            _ => "APT subcommand",
        },
        "journalctl-schema" => match value {
            "--follow" => "Follow new journal entries live",
            "--unit=" => "Filter logs by systemd unit",
            "--since=" => "Show logs since a given time",
            "--until=" => "Show logs until a given time",
            "--boot" => "Show logs from a boot",
            _ => "journalctl option",
        },
        "ssh-schema" => match value {
            "-i" => "Use a specific SSH identity file",
            "-p" => "Connect to a specific SSH port",
            "-J" => "Connect through a jump host",
            "-L" => "Create local port forwarding",
            "-R" => "Create remote port forwarding",
            _ => "SSH connection option",
        },
        "sudo-schema" => match value {
            "-u" | "--user=" => "Run the command as another user",
            "-g" | "--group=" => "Run the command as another group",
            "-H" => "Use the target user's home directory",
            "-i" | "--login" => "Start a login shell as the target user",
            _ => "sudo option",
        },
        "find-schema" => match value {
            "-name" => "Match entries by file name",
            "-iname" => "Case-insensitive file-name match",
            "-type" => "Match entries by file type",
            "-user" => "Match entries owned by a user",
            "-group" => "Match entries owned by a group",
            "-mtime" => "Match by modification age in days",
            "-size" => "Match entries by file size",
            "-delete" => "Delete matching entries",
            _ => "find search option",
        },
        "grep-schema" => match value {
            "-r" | "--recursive" => "Search directories recursively",
            "-R" => "Search recursively and follow symlinks",
            "-i" | "--ignore-case" => "Ignore letter case while matching",
            "-n" | "--line-number" => "Show matching line numbers",
            "-v" | "--invert-match" => "Show non-matching lines",
            "-w" | "--word-regexp" => "Match whole words only",
            _ => "grep search option",
        },
        "tar-schema" => match value {
            "--create" | "-cf" | "-cvf" => "Create a new archive",
            "--extract" | "-xf" | "-xvf" => "Extract files from an archive",
            "--list" | "-tf" | "-tvf" => "List files stored in an archive",
            "--gzip" => "Use gzip compression",
            "--xz" => "Use xz compression",
            _ => "tar archive option",
        },
        "curl-schema" => match value {
            "--head" | "-I" => "Fetch response headers only",
            "--header" | "-H" => "Add an HTTP request header",
            "--location" | "-L" => "Follow HTTP redirects",
            "--output" | "-o" => "Write response data to a file",
            "--request" | "-X" => "Set the HTTP request method",
            "--retry" => "Retry transient request failures",
            "--silent" | "-s" => "Hide progress and error meter output",
            _ => "curl transfer option",
        },
        "command-lookup-schema" => "Command lookup option",
        "termsense-schema" => match value {
            "doctor" => "Check whether TermSense can run correctly",
            "index" => "Refresh the local command index",
            "init" => "Print shell integration code",
            "list-commands" => "List commands discovered on this machine",
            "status" => "Show TermSense engine and cache status",
            "suggest" => "Query suggestions for a command buffer",
            "bash" => "Generate Bash shell integration",
            "--json" => "Return machine-readable JSON output",
            "--limit" | "-n" => "Limit the number of suggestions",
            "--cursor" => "Set the cursor byte position",
            _ => "TermSense command or option",
        },
        _ => match kind {
            "directory" => "Directory",
            "file" => "File",
            "option" => "Command option",
            "subcommand" => "Command subcommand",
            "package" => "Package",
            "user" => "User",
            "group" => "Group",
            _ => "Command suggestion",
        },
    }
}

fn usage_key_for(source: &str, kind: &str, insert_text: &str) -> String {
    match source {
        "path"
        | "shell-builtin"
        | "git-schema"
        | "git-commit-schema"
        | "git-checkout-schema"
        | "git-switch-schema"
        | "git-log-schema"
        | "systemctl-schema"
        | "docker-schema"
        | "docker-logs-schema"
        | "docker-ps-schema"
        | "docker-exec-schema"
        | "cargo-schema"
        | "package-manager-schema"
        | "apt-schema"
        | "journalctl-schema"
        | "ssh-schema"
        | "sudo-schema"
        | "find-schema"
        | "grep-schema"
        | "tar-schema"
        | "curl-schema"
        | "command-lookup-schema"
        | "man-cache"
        | "termsense-schema" => format!("{source}:{kind}:{insert_text}"),
        _ => String::new(),
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

fn finalize(
    mut candidates: Vec<Candidate>,
    usage: &UsageState,
    limit: usize,
    buffer: &str,
    cursor: usize,
) -> Vec<Candidate> {
    for candidate in &mut candidates {
        candidate.display_text = completed_line(
            buffer,
            cursor,
            candidate.replacement_start,
            candidate.replacement_end,
            &candidate.insert_text,
        );
        candidate.score += usage.boost(&candidate.usage_key);
    }

    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.display_text.len().cmp(&b.display_text.len()))
            .then_with(|| a.display_text.cmp(&b.display_text))
    });

    let mut seen = BTreeSet::new();
    candidates.retain(|candidate| {
        seen.insert((
            candidate.insert_text.clone(),
            candidate.replacement_start,
            candidate.replacement_end,
        ))
    });
    if limit > 0 {
        candidates.truncate(limit);
    }
    candidates
}

fn completed_line(
    buffer: &str,
    cursor: usize,
    replacement_start: usize,
    replacement_end: usize,
    insert_text: &str,
) -> String {
    let mut result = String::with_capacity(buffer.len() + insert_text.len());
    result.push_str(&buffer[..replacement_start]);
    result.push_str(insert_text);
    result.push_str(&buffer[replacement_end..cursor]);
    result.push_str(&buffer[cursor..]);
    result
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
    use super::{completed_line, directory_query, score_prefix, sudo_nested_command_index};
    use crate::{
        shell_parse::{active_context, active_segment_tokens, tokens_before_cursor},
        usage::UsageState,
    };

    #[test]
    fn sudo_nested_command_is_detected() {
        let input = "sudo -u root git che";
        let tokens = tokens_before_cursor(input, input.len());
        assert_eq!(sudo_nested_command_index(&tokens), Some(3));
    }

    #[test]
    fn directory_query_preserves_tilde_prefix() {
        let (_, typed_parent, base) = directory_query("~/Doc").unwrap();
        assert_eq!(typed_parent, "~/");
        assert_eq!(base, "Doc");
    }

    #[test]
    fn exact_prefix_scores_above_longer_match() {
        assert!(score_prefix("git", "git").unwrap() > score_prefix("gitk", "git").unwrap());
    }

    #[test]
    fn git_subcommands_are_contextual() {
        let candidates = super::suggest(&[], &UsageState::default(), "git che", 7, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.insert_text == "checkout"));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.insert_text == "status"));
    }

    #[test]
    fn sudo_suggestion_displays_complete_command() {
        let candidates = super::suggest(&[], &UsageState::default(), "sudo git che", 12, 20);
        let checkout = candidates
            .iter()
            .find(|candidate| candidate.insert_text == "checkout")
            .expect("checkout candidate");

        assert_eq!(checkout.display_text, "sudo git checkout");
        assert_eq!(checkout.replacement_start, 9);
        assert_eq!(checkout.replacement_end, 12);
    }

    #[test]
    fn complete_line_only_replaces_active_token() {
        assert_eq!(
            completed_line("sudo git che", 12, 9, 12, "checkout"),
            "sudo git checkout"
        );
    }

    #[test]
    fn cargo_subcommands_are_generic() {
        let candidates = super::suggest(&[], &UsageState::default(), "cargo bu", 8, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "cargo build"));
    }

    #[test]
    fn git_commit_flags_are_contextual() {
        let candidates = super::suggest(&[], &UsageState::default(), "git commit --a", 14, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "git commit --amend"));
    }

    #[test]
    fn docker_logs_flags_are_contextual() {
        let candidates = super::suggest(&[], &UsageState::default(), "docker logs --f", 15, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "docker logs --follow"));
    }

    #[test]
    fn apt_subcommands_are_contextual() {
        let candidates = super::suggest(&[], &UsageState::default(), "apt ins", 7, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "apt install"));
    }

    #[test]
    fn find_options_are_contextual() {
        let candidates = super::suggest(&[], &UsageState::default(), "find ./ -na", 11, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "find ./ -name"));
    }

    #[test]
    fn find_type_values_are_contextual() {
        let input = "find ./ -type d";
        let candidates = super::suggest(&[], &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "find ./ -type d"));
    }

    #[test]
    fn grep_options_are_contextual() {
        let candidates = super::suggest(&[], &UsageState::default(), "grep -r", 7, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "grep -r"));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "grep -R"));
    }

    #[test]
    fn curl_options_are_contextual() {
        let candidates = super::suggest(&[], &UsageState::default(), "curl --hea", 10, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "curl --head"));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "curl --header"));
    }

    #[test]
    fn control_char_candidates_are_rejected() {
        let mut out = Vec::new();
        super::push_match(
            &mut out,
            "bad\u{1b}name",
            "bad\u{1b}name",
            "bad",
            "file",
            "filesystem",
            0,
            3,
            0,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn dynamic_resource_candidates_are_not_persisted_for_ranking() {
        assert_eq!(super::usage_key_for("ssh-local", "ssh-host", "prod"), "");
        assert_eq!(
            super::usage_key_for("filesystem", "file", "~/secret.txt"),
            ""
        );
        assert_eq!(
            super::usage_key_for("git-local", "git-ref", "feature/private"),
            ""
        );
        assert_eq!(super::usage_key_for("sudo-local", "user", "alice"), "");
    }

    #[test]
    fn nested_sudo_keeps_full_display_context() {
        let candidates = super::suggest(&[], &UsageState::default(), "sudo -H git che", 15, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "sudo -H git checkout"));
    }

    #[test]
    fn command_lookup_uses_installed_command_pool() {
        let commands = vec![crate::CommandEntry {
            name: "docker".to_owned(),
            path: std::path::PathBuf::from("/usr/bin/docker"),
        }];
        let candidates = super::suggest(&commands, &UsageState::default(), "which do", 8, 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "which docker"));
    }

    #[test]
    fn termsense_completes_its_own_cli() {
        let input = "termsense st";
        let candidates = super::suggest(&[], &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "termsense status"));

        let input = "termsense init b";
        let candidates = super::suggest(&[], &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "termsense init bash"));
    }

    #[test]
    fn ssh_include_wildcard_matching_is_bounded_and_predictable() {
        assert!(super::wildcard_match("*.conf", "work.conf"));
        assert!(super::wildcard_match("host-??", "host-01"));
        assert!(!super::wildcard_match("*.conf", "work.txt"));
    }

    #[test]
    fn command_substitution_routes_to_inner_git() {
        let input = "echo $(git che";
        let candidates = super::suggest(&[], &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "echo $(git checkout"));
    }

    #[test]
    fn backtick_substitution_routes_to_inner_git() {
        let input = "echo `git che";
        let candidates = super::suggest(&[], &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "echo `git checkout"));
    }

    #[test]
    fn redirection_context_is_exposed_by_parser() {
        let input = "echo hi > lo";
        let context = active_context(input, input.len());
        assert!(context.redirection_target);
        assert_eq!(context.tokens.last().unwrap().text, "lo");
    }

    #[test]
    fn process_substitution_routes_to_inner_git_provider() {
        let input = "diff <(git che";
        let candidates = super::suggest(&[], &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "diff <(git checkout"));
    }

    #[test]
    fn paren_group_routes_to_inner_git_provider() {
        let input = "( git che";
        let candidates = super::suggest(&[], &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "( git checkout"));
    }

    #[test]
    fn brace_group_routes_to_inner_docker_provider() {
        let input = "{ docker lo";
        let candidates = super::suggest(&[], &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "{ docker logs"));
    }

    #[test]
    fn pipeline_routes_to_right_hand_command() {
        let commands = vec![crate::CommandEntry {
            name: "grep".to_owned(),
            path: std::path::PathBuf::from("/usr/bin/grep"),
        }];
        let input = "cat file | gre";
        let candidates = super::suggest(&commands, &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "cat file | grep"));
    }

    #[test]
    fn separator_routes_to_right_hand_context() {
        let input = "git status && docker lo";
        let candidates = super::suggest(&[], &UsageState::default(), input, input.len(), 20);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.display_text == "git status && docker logs"));
    }

    #[test]
    fn pipeline_quotes_are_not_boundaries() {
        let input = "grep \"a|b\" --r";
        let tokens = active_segment_tokens(input, input.len());
        assert_eq!(tokens[0].text, "grep");
        assert_eq!(tokens[1].text, "a|b");
    }
}
