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

const DF_OPTIONS: &[&str] = &["--all", "--human-readable", "--inodes", "--print-type", "-a", "-h", "-i", "-T"];
const DU_OPTIONS: &[&str] = &[
    "--all",
    "--apparent-size",
    "--human-readable",
    "--max-depth=",
    "--one-file-system",
    "--summarize",
    "-a",
    "-h",
    "-s",
    "-x",
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
        "df" => {
            if current.text.starts_with('-') {
                add_static(&mut candidates, DF_OPTIONS, current, "option", "df-schema", 650);
            }
        }
        "du" => {
            if current.text.starts_with('-') {
                add_static(&mut candidates, DU_OPTIONS, current, "option", "du-schema", 650);
            }
        }
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
            "bisect" => "Binary-search commit history to find where a bug was introduced",
            "branch" => "List, create, rename, or delete branches",
            "check-attr" => "Show gitattributes values for files or paths",
            "check-ignore" => "Show whether paths are excluded by Git ignore rules",
            "check-ref-format" => "Validate or normalize a Git reference name",
            "checkout" => "Switch branches or restore files",
            "cherry-pick" => "Apply an existing commit onto the current branch",
            "clone" => "Copy a remote repository locally",
            "commit" => "Record staged changes in Git history",
            "diff" => "Show changes between working tree, index, commits, or refs",
            "fetch" => "Download refs and objects from a remote",
            "grep" => "Search tracked files or revisions for matching text",
            "init" => "Create a new Git repository",
            "log" => "Show commit history",
            "merge" => "Combine another branch into the current branch",
            "mv" => "Move or rename a tracked file and stage the change",
            "pull" => "Fetch remote changes and integrate them locally",
            "push" => "Upload local commits and refs to a remote",
            "rebase" => "Replay commits onto a different base commit",
            "remote" => "Manage named remote repositories",
            "reset" => "Move HEAD or reset index and working-tree state",
            "restore" => "Restore file contents from the index or another tree",
            "revert" => "Create a commit that reverses an earlier commit",
            "rm" => "Remove tracked files and stage their deletion",
            "show" => "Show details and changes for a Git object",
            "stash" => "Temporarily save uncommitted working-tree changes",
            "status" => "Show working-tree and staging-area status",
            "switch" => "Switch to another branch",
            "tag" => "Create, list, delete, or verify Git tags",
            "worktree" => "Manage multiple working trees attached to one repository",
            _ => "Git behavior",
        },
        "git-commit-schema" => match value {
            "--all" => "Stage modified and deleted tracked files before committing",
            "--amend" => "Replace the tip commit with a new commit",
            "--author=" => "Override the author recorded for the commit",
            "--date=" => "Override the author date recorded for the commit",
            "--dry-run" => "Show what would be committed without creating a commit",
            "--edit" => "Open the editor to modify the commit message",
            "--message=" => "Use the supplied text as the commit message",
            "--no-edit" => "Reuse the selected commit message without opening an editor",
            "--no-verify" => "Skip the pre-commit and commit-msg hooks",
            "--only" => "Commit only the specified paths, ignoring other staged changes",
            "--patch" => "Interactively choose hunks to stage before committing",
            "--quiet" => "Suppress the commit summary and feedback",
            "--reuse-message=" => "Reuse the message and authorship from another commit",
            "--signoff" => "Add a Signed-off-by trailer to the commit message",
            "--verbose" => "Show the diff in the commit-message editor",
            _ => "Git commit behavior",
        },
        "git-checkout-schema" => match value {
            "--conflict=" => "Choose the conflict-marker style for unresolved paths",
            "--detach" => "Check out a commit without attaching HEAD to a branch",
            "--force" => "Force checkout and discard conflicting local changes",
            "--guess" => "Guess a local branch from a matching remote-tracking branch",
            "--ignore-other-worktrees" => "Allow checkout of a branch used by another worktree",
            "--merge" => "Three-way merge local changes while switching branches",
            "--orphan=" => "Create and switch to a new orphan branch",
            "--ours" => "Use the 'ours' side for an unmerged path",
            "--patch" => "Interactively choose hunks to restore from the target",
            "--quiet" => "Suppress checkout progress and feedback",
            "--theirs" => "Use the 'theirs' side for an unmerged path",
            "--track" => "Set the new branch to track its remote branch",
            _ => "Git checkout behavior",
        },
        "git-switch-schema" => match value {
            "--create=" => "Create a new branch and switch to it",
            "--detach" => "Switch to a commit with detached HEAD",
            "--discard-changes" => "Discard local modifications when switching",
            "--force-create=" => "Create or reset a branch, then switch to it",
            "--guess" => "Infer a local branch from a matching remote branch",
            "--merge" => "Three-way merge local changes while switching",
            "--no-guess" => "Disable remote-branch name guessing",
            "--orphan=" => "Create and switch to a new orphan branch",
            "--quiet" => "Suppress branch-switch feedback",
            "--track" => "Configure the new branch to track its start point",
            _ => "Git switch behavior",
        },
        "git-log-schema" => match value {
            "--all" => "Show commits reachable from all refs",
            "--author=" => "Show only commits whose author matches the pattern",
            "--decorate" => "Show branch and tag names beside commits",
            "--follow" => "Continue file history across renames",
            "--graph" => "Draw an ASCII commit graph",
            "--max-count=" => "Limit the number of commits shown",
            "--oneline" => "Show each commit in compact one-line form",
            "--patch" => "Show the patch introduced by each commit",
            "--since=" => "Show commits newer than the given date",
            "--stat" => "Show changed-file statistics for each commit",
            "--until=" => "Show commits older than the given date",
            _ => "Git log behavior",
        },
        "systemctl-schema" => match value {
            "cat" => "Show the backing unit files and drop-ins",
            "daemon-reload" => "Reload systemd unit files and generator output",
            "default" => "Switch the system to its configured default target",
            "disable" => "Remove boot-time enablement links for a unit",
            "edit" => "Open an editor for a unit override or unit file",
            "emergency" => "Enter emergency mode with the minimal environment",
            "enable" => "Create boot-time enablement links for a unit",
            "is-active" => "Exit successfully only if the unit is active",
            "is-enabled" => "Report whether the unit is enabled at boot",
            "is-failed" => "Exit successfully only if the unit is failed",
            "isolate" => "Start one target and stop units not required by it",
            "kill" => "Send a signal to processes belonging to a unit",
            "list-unit-files" => "List installed unit files and enablement states",
            "list-units" => "List units currently loaded by systemd",
            "mask" => "Prevent a unit from being started at all",
            "preset" => "Apply the distribution preset policy to a unit",
            "reenable" => "Disable and then enable a unit again",
            "reload" => "Ask a running service to reload its configuration",
            "reload-or-restart" => "Reload a service when supported, otherwise restart it",
            "rescue" => "Enter rescue mode with basic system services",
            "reset-failed" => "Clear failed state and restart-rate counters",
            "restart" => "Stop and then start one or more units",
            "show" => "Show low-level systemd properties for a unit",
            "start" => "Start one or more systemd units",
            "status" => "Show human-readable runtime status for a unit",
            "stop" => "Stop one or more systemd units",
            "try-restart" => "Restart a unit only when it is already active",
            "unmask" => "Remove a unit mask so it can be started again",
            "--all" => "Include inactive units or normally hidden properties",
            "--failed" => "Limit unit listings to failed units",
            "--force" => "Force an operation that normally requires extra safety checks",
            "--full" => "Do not ellipsize unit names or output fields",
            "--global" => "Operate on global user-unit configuration",
            "--help" => "Show systemctl help and exit",
            "--no-ask-password" => "Never prompt interactively for authentication",
            "--no-block" => "Queue the operation and return without waiting",
            "--no-legend" => "Hide column headers and footer hints",
            "--no-pager" => "Print directly instead of opening a pager",
            "--now" => "Also start or stop the unit immediately when enabling or disabling",
            "--quiet" => "Suppress normal informational output",
            "--runtime" => "Make enablement changes only until the next boot",
            "--system" => "Operate on the system service manager",
            "--type=" => "Filter listed units by unit type",
            "--user" => "Operate on the current user's service manager",
            "--version" => "Show systemd version information",
            _ => "systemctl behavior",
        },
        "docker-schema" => match value {
            "build" => "Build an image from a Dockerfile and build context",
            "compose" => "Define and run multi-container applications",
            "container" => "Manage Docker containers",
            "context" => "Manage Docker endpoint contexts",
            "cp" => "Copy files between the host and a container",
            "create" => "Create a container without starting it",
            "exec" => "Run a new command inside a running container",
            "image" => "Manage Docker images",
            "images" => "List images stored on this Docker host",
            "info" => "Show Docker daemon and host information",
            "inspect" => "Show low-level JSON details for Docker objects",
            "kill" => "Send a kill signal to running containers",
            "login" => "Authenticate to a container registry",
            "logout" => "Remove saved registry authentication",
            "logs" => "Show stdout and stderr from a container",
            "network" => "Manage Docker networks",
            "port" => "Show published port mappings for a container",
            "ps" => "List Docker containers",
            "pull" => "Download an image from a registry",
            "push" => "Upload an image to a registry",
            "rename" => "Change a container's name",
            "restart" => "Restart one or more containers",
            "rm" => "Remove one or more containers",
            "rmi" => "Remove one or more local images",
            "run" => "Create and start a new container from an image",
            "search" => "Search a registry for images",
            "start" => "Start one or more stopped containers",
            "stats" => "Show live container CPU, memory, and I/O usage",
            "stop" => "Gracefully stop one or more running containers",
            "system" => "Manage Docker-wide resources and cleanup",
            "tag" => "Create another name and tag for an image",
            "top" => "Show processes running inside a container",
            "version" => "Show Docker client and server version details",
            "volume" => "Manage persistent Docker volumes",
            "wait" => "Wait for containers to stop and print their exit codes",
            _ => "Docker behavior",
        },
        "docker-logs-schema" => match value {
            "--details" => "Include extra log attributes provided by the daemon",
            "--follow" => "Keep streaming new container log output",
            "--since=" => "Show only logs newer than the given timestamp or duration",
            "--tail=" => "Show only the last N lines of container logs",
            "--timestamps" => "Prefix each log line with its timestamp",
            "--until=" => "Stop showing logs newer than the given time",
            _ => "Docker logs behavior",
        },
        "docker-ps-schema" => match value {
            "--all" => "Show stopped containers as well as running ones",
            "--filter=" => "Filter containers by a key=value condition",
            "--format=" => "Format container output with a Go template",
            "--last=" => "Show the N most recently created containers",
            "--latest" => "Show only the most recently created container",
            "--no-trunc" => "Do not shorten IDs or command output",
            "--quiet" => "Print only container IDs",
            "--size" => "Display each container's filesystem size",
            _ => "Docker ps behavior",
        },
        "docker-exec-schema" => match value {
            "--detach" => "Run the command in the background",
            "--env=" => "Set an environment variable inside the exec process",
            "--interactive" => "Keep standard input open for the exec process",
            "--privileged" => "Give the exec process extended privileges",
            "--tty" => "Allocate a pseudo-TTY for the exec process",
            "--user=" => "Run the exec process as a specific user",
            "--workdir=" => "Set the working directory inside the container",
            _ => "Docker exec behavior",
        },
        "cargo-schema" => match value {
            "add" => "Add a dependency to Cargo.toml",
            "bench" => "Compile and run benchmark targets",
            "build" => "Compile the current Rust project",
            "check" => "Type-check Rust code without producing final binaries",
            "clean" => "Remove Cargo build artifacts",
            "doc" => "Build Rust API documentation",
            "fetch" => "Download dependencies without building",
            "fix" => "Apply compiler-suggested fixes to Rust source",
            "generate-lockfile" => "Create or update Cargo.lock without building",
            "help" => "Show help for Cargo or a Cargo command",
            "init" => "Create a Cargo project in this directory",
            "install" => "Build and install a Rust binary crate",
            "locate-project" => "Print the path to the current Cargo.toml",
            "login" => "Store an API token for a Cargo registry",
            "metadata" => "Output machine-readable package graph metadata",
            "new" => "Create a new Cargo project",
            "owner" => "Manage crate owners on a registry",
            "package" => "Assemble the crate package for publishing",
            "publish" => "Upload a crate to a registry",
            "remove" => "Remove a dependency from Cargo.toml",
            "report" => "Generate Cargo diagnostic reports",
            "run" => "Build and run the current Rust binary",
            "rustc" => "Compile the package while passing extra flags to rustc",
            "search" => "Search crates in a registry",
            "test" => "Build and run Rust tests",
            "tree" => "Display the dependency graph",
            "uninstall" => "Remove a Cargo-installed binary",
            "update" => "Update dependencies recorded in Cargo.lock",
            "vendor" => "Copy dependencies into a local vendor directory",
            "verify-project" => "Check whether the manifest parses successfully",
            "version" => "Show Cargo version information",
            "yank" => "Mark a published crate version as unavailable",
            "--all-features" => "Enable every Cargo feature for this build",
            "--doc" => "Run documentation tests",
            "--examples" => "Build all example targets",
            "--features=" => "Enable the listed Cargo features",
            "--jobs=" => "Set the number of parallel build jobs",
            "--lib" => "Build or test only the library target",
            "--locked" => "Require Cargo.lock to remain unchanged",
            "--no-run" => "Compile tests without running them",
            "--offline" => "Use only dependencies already available locally",
            "--package=" => "Build or test only the named package",
            "--profile=" => "Build with the named Cargo profile",
            "--quiet" => "Suppress Cargo status messages",
            "--release" => "Build with the optimized release profile",
            "--target=" => "Build for the specified target triple",
            "--test=" => "Run only the named integration test target",
            "--tests" => "Build or run all test targets",
            "--verbose" => "Show detailed Cargo command output",
            "--workspace" => "Apply the command to every workspace member",
            _ => "Cargo behavior",
        },
        "package-manager-schema" => match value {
            "access" => "Manage package access permissions on the registry",
            "add" | "install" => "Install dependencies into the project",
            "adduser" | "login" => "Authenticate this package manager with a registry",
            "audit" => "Check installed dependencies for known vulnerabilities",
            "bin" => "Show the package-manager binary directory",
            "cache" => "Inspect or manage the local package cache",
            "ci" => "Install exactly from the lockfile for reproducible builds",
            "config" => "Read or change package-manager configuration",
            "create" => "Run a project scaffolding package",
            "dedupe" => "Reduce duplicate dependency versions in the install tree",
            "deploy" => "Prepare a deployable package from a workspace",
            "diff" => "Show package or dependency differences",
            "dist-tag" => "Manage registry distribution tags such as latest",
            "dlx" => "Download a package temporarily and run its binary",
            "docs" => "Open or show package documentation",
            "doctor" => "Diagnose package-manager environment problems",
            "exec" => "Run a command with project package binaries on PATH",
            "explain" => "Explain why a dependency is installed",
            "explore" => "Open a shell inside an installed package",
            "fetch" => "Fetch packages into the local store without linking",
            "help" => "Show help for a package-manager command",
            "import" => "Generate this manager's lockfile from another lockfile",
            "info" | "view" => "Show registry metadata for a package",
            "init" => "Create a package manifest in the current directory",
            "link" => "Link a local package into another project",
            "list" | "ls" => "List installed project dependencies",
            "node" => "Run Node.js using the package manager's environment",
            "npm" => "Run an npm-compatible registry command",
            "outdated" => "Show dependencies with newer versions available",
            "owner" => "Manage package owners on the registry",
            "pack" => "Create a publishable package archive",
            "patch" => "Create or apply a local dependency patch",
            "ping" => "Test connectivity to the configured registry",
            "pkg" => "Read or edit fields in package.json",
            "plugin" => "Manage package-manager plugins",
            "pm" => "Run package-manager maintenance utilities",
            "prefix" => "Show the installation prefix",
            "profile" => "Manage registry user profile settings",
            "prune" => "Remove dependencies no longer needed by the project",
            "publish" => "Upload the package to a registry",
            "query" => "Query installed dependencies with selectors",
            "rebuild" => "Re-run build scripts for installed dependencies",
            "remove" | "uninstall" => "Remove a dependency from the project",
            "repo" => "Open or show the package source repository",
            "restart" => "Run the package restart lifecycle script",
            "root" => "Show the installed node_modules directory",
            "run" => "Run a script defined by the project",
            "search" => "Search the configured package registry",
            "set" => "Change package-manager or project settings",
            "shrinkwrap" => "Create or rename a lockfile for published packages",
            "stage" => "Stage a release or workspace artifact",
            "star" | "unstar" => "Add or remove a package from registry favorites",
            "stars" => "List packages starred by a registry user",
            "start" => "Run the project's start lifecycle script",
            "stop" => "Run the project's stop lifecycle script",
            "store" => "Manage the shared dependency content-addressable store",
            "team" => "Manage registry organization teams",
            "test" => "Run the project's test lifecycle script",
            "token" => "Manage registry authentication tokens",
            "unlink" => "Remove a previously created package link",
            "unpublish" => "Remove a published package version from the registry",
            "update" | "up" => "Update project dependencies to newer allowed versions",
            "upgrade" => "Upgrade the package manager itself or project tooling",
            "version" => "Show or change package version information",
            "whoami" => "Show the registry account currently authenticated",
            "why" => "Explain why a dependency is present in the project",
            "workspace" => "Run a command in one workspace package",
            "workspaces" => "Run or inspect commands across workspaces",
            "x" => "Download and execute a package binary temporarily",
            _ => "Package-manager behavior",
        },
        "apt-schema" => match value {
            "autoremove" => "Remove dependency packages that are no longer needed",
            "download" => "Download a package file without installing it",
            "edit-sources" => "Edit APT repository source definitions",
            "full-upgrade" => "Upgrade packages even when dependencies must change",
            "install" => "Install one or more APT packages",
            "list" => "List packages matching a pattern",
            "purge" => "Remove packages and their configuration files",
            "reinstall" => "Reinstall packages that are already installed",
            "remove" => "Remove installed packages but keep configuration files",
            "satisfy" => "Satisfy dependency expressions from the command line",
            "search" => "Search package names and descriptions",
            "show" => "Show detailed package metadata",
            "update" => "Refresh package index metadata from configured sources",
            "upgrade" => "Upgrade installed packages without removing packages",
            "--assume-yes" | "--yes" => "Automatically answer yes to package prompts",
            "--download-only" => "Download packages without installing them",
            "--fix-broken" => "Try to repair broken package dependencies",
            "--no-install-recommends" => "Do not install recommended dependency packages",
            "--only-upgrade" => "Upgrade packages only if they are already installed",
            "--purge" => "Remove configuration files when removing packages",
            "--quiet" => "Reduce APT output",
            "--reinstall" => "Reinstall packages even if the version is unchanged",
            "--simulate" => "Show planned package actions without changing the system",
            _ => "APT behavior",
        },
        "journalctl-schema" => match value {
            "--boot" => "Show messages from a specific boot",
            "--catalog" => "Add explanatory catalog text to supported messages",
            "--disk-usage" => "Show how much disk space journal files use",
            "--follow" => "Keep streaming new journal entries live",
            "--grep=" => "Show only messages matching the regular expression",
            "--lines=" => "Limit output to the most recent N journal lines",
            "--list-boots" => "List recorded boots and their time ranges",
            "--no-hostname" => "Hide the hostname field in rendered output",
            "--no-pager" => "Print directly instead of opening a pager",
            "--output=" => "Choose the journal output format",
            "--priority=" => "Filter messages by syslog priority",
            "--reverse" => "Show newest entries first",
            "--since=" => "Show entries on or after the given time",
            "--system" => "Show the system journal",
            "--unit=" => "Filter entries to a systemd unit",
            "--until=" => "Show entries on or before the given time",
            "--user" => "Show the current user's journal",
            "--user-unit=" => "Filter entries to a user systemd unit",
            _ => "journalctl behavior",
        },
        "ssh-schema" => match value {
            "-4" => "Force SSH to use IPv4 addresses only",
            "-6" => "Force SSH to use IPv6 addresses only",
            "-A" => "Forward the local SSH authentication agent",
            "-a" => "Disable SSH authentication-agent forwarding",
            "-C" => "Enable compression for the SSH connection",
            "-F" => "Read SSH configuration from the specified file",
            "-f" => "Send SSH to the background before command execution",
            "-i" => "Use a specific private-key identity file",
            "-J" => "Connect through the specified jump host",
            "-L" => "Forward a local port to a remote destination",
            "-N" => "Do not run a remote command; use forwarding only",
            "-n" => "Redirect SSH standard input from /dev/null",
            "-o" => "Set an SSH configuration option on the command line",
            "-p" => "Connect to the specified remote SSH port",
            "-R" => "Forward a remote port to a local destination",
            "-T" => "Disable pseudo-terminal allocation",
            "-t" => "Force pseudo-terminal allocation",
            "-v" => "Enable verbose SSH connection diagnostics",
            _ => "SSH behavior",
        },
        "sudo-schema" => match value {
            "-A" => "Use an askpass helper instead of reading a password from the terminal",
            "-b" => "Run the command in the background",
            "-C" | "--close-from=" => "Close inherited file descriptors at or above this number",
            "-D" | "--chdir=" => "Change to the specified directory before running the command",
            "-E" => "Preserve the invoking user's environment where policy allows",
            "-e" => "Edit files through sudoedit instead of running a command directly",
            "-g" | "--group=" => "Run the command with the specified primary group",
            "-H" => "Set HOME to the target user's home directory",
            "-h" | "--host=" => "Apply sudo policy as if running on the specified host",
            "-i" | "--login" => "Run a login shell as the target user",
            "-K" => "Remove the user's cached sudo credentials completely",
            "-k" | "--reset-timestamp" => "Invalidate cached sudo credentials for the next command",
            "-l" => "List commands the current user may run with sudo",
            "-n" | "--non-interactive" => "Fail instead of prompting for a password",
            "-P" => "Preserve the invoking user's group vector where supported",
            "-p" | "--prompt=" => "Use a custom password prompt",
            "-R" => "Change the root directory before running the command",
            "-S" => "Read the password from standard input",
            "-s" | "--shell" => "Run the target user's shell",
            "-T" | "--command-timeout=" => "Terminate the command after the specified timeout",
            "-U" => "List privileges for another user",
            "-u" | "--user=" => "Run the command as the specified user",
            "-V" => "Show sudo version and build information",
            "--preserve-env" => "Preserve selected environment variables",
            "--remove-timestamp" => "Delete cached sudo credentials",
            _ => "sudo behavior",
        },
        "find-schema" => match value {
            "-amin" => "Match files by last-access age in minutes",
            "-anewer" => "Match files accessed more recently than the reference file",
            "-atime" => "Match files by last-access age in days",
            "-cmin" => "Match files by metadata-change age in minutes",
            "-cnewer" => "Match files whose metadata changed after the reference file",
            "-ctime" => "Match files by metadata-change age in days",
            "-delete" => "Delete every file or directory that matches",
            "-depth" => "Process directory contents before the directory itself",
            "-empty" => "Match empty files and directories",
            "-exec" => "Run a command for each set of matching paths",
            "-executable" => "Match files executable by the current user",
            "-false" => "Always evaluate false in the find expression",
            "-gid" => "Match files owned by a numeric group ID",
            "-group" => "Match files owned by the specified group",
            "-iname" => "Match file names case-insensitively",
            "-inum" => "Match files by inode number",
            "-ipath" => "Match the full path case-insensitively",
            "-iregex" => "Match the full path using a case-insensitive regex",
            "-links" => "Match files by hard-link count",
            "-lname" => "Match symbolic-link targets by pattern",
            "-ls" => "Print matching paths in detailed ls-style form",
            "-maxdepth" => "Do not descend below the specified depth",
            "-mindepth" => "Skip matches above the specified minimum depth",
            "-mmin" => "Match files by modification age in minutes",
            "-mount" | "-xdev" => "Do not descend into other mounted filesystems",
            "-mtime" => "Match files by modification age in days",
            "-name" => "Match file names against the supplied pattern",
            "-newer" => "Match files modified after the reference file",
            "-nogroup" => "Match files whose group ID has no group entry",
            "-nouser" => "Match files whose user ID has no passwd entry",
            "-path" => "Match the full path against a shell pattern",
            "-perm" => "Match files by permission bits",
            "-print" => "Print each matching path on its own line",
            "-print0" => "Print matching paths separated by NUL bytes",
            "-prune" => "Do not descend into the matched directory",
            "-readable" => "Match files readable by the current user",
            "-regex" => "Match the entire path using a regular expression",
            "-samefile" => "Match paths referring to the same inode as a reference file",
            "-size" => "Match files by file size",
            "-type" => "Match only the specified filesystem object type",
            "-uid" => "Match files owned by a numeric user ID",
            "-user" => "Match files owned by the specified user",
            _ => "find behavior",
        },
        "grep-schema" => match value {
            "--after-context=" => "Print N lines after each matching line",
            "--before-context=" => "Print N lines before each matching line",
            "--binary-files=" => "Choose how grep treats binary files",
            "--color=" => "Control color highlighting of matched text",
            "--context=" => "Print N lines before and after each match",
            "--count" => "Print only the number of matching lines per file",
            "--exclude=" => "Skip files whose names match the pattern",
            "--exclude-dir=" => "Skip directories whose names match the pattern",
            "--file=" | "-f" => "Read search patterns from the specified file",
            "--files-with-matches" | "-l" => "Print only names of files containing matches",
            "--fixed-strings" | "-F" => {
                "Treat patterns as literal strings, not regular expressions"
            }
            "--ignore-case" | "-i" => "Ignore letter case while matching",
            "--include=" => "Search only files whose names match the pattern",
            "--invert-match" | "-v" => "Select lines that do not match",
            "--line-number" | "-n" => "Prefix matching lines with their line numbers",
            "--max-count=" => "Stop after the specified number of matches per file",
            "--no-filename" => "Do not prefix output with file names",
            "--only-matching" | "-o" => "Print only the matching part of each line",
            "--quiet" | "-q" => "Suppress output and stop after the first match",
            "--recursive" | "-r" => "Search files recursively under each directory",
            "--word-regexp" | "-w" => "Match only complete words",
            "-E" => "Interpret the pattern as an extended regular expression",
            "-H" => "Always prefix matches with the file name",
            "-I" => "Ignore binary files",
            "-L" => "Print only names of files with no matching lines",
            "-R" => "Search recursively and follow symbolic links",
            _ => "grep behavior",
        },
        "tar-schema" => match value {
            "--append" => "Append files to the end of an existing archive",
            "--create" | "-cf" => "Create a new archive and write it to the given file",
            "-cvf" => "Create an archive and list files as they are added",
            "--delete" => "Delete members from an archive",
            "--directory=" => "Change directory before processing archive paths",
            "--exclude=" => "Skip files whose names match the pattern",
            "--extract" | "-xf" => "Extract files from the archive",
            "-xvf" => "Extract files and print each extracted member",
            "--file=" => "Read or write the archive at the specified path",
            "--gzip" => "Compress or decompress the archive with gzip",
            "--list" | "-tf" => "List members stored in the archive",
            "-tvf" => "List archive members with verbose metadata",
            "--verbose" => "Print each file as tar processes it",
            "--xz" => "Compress or decompress the archive with xz",
            _ => "tar behavior",
        },
        "curl-schema" => match value {
            "--cacert" => "Verify TLS using the specified CA certificate file",
            "--compressed" => {
                "Request compressed content and decompress the response automatically"
            }
            "--connect-timeout" => "Limit how long curl may spend establishing a connection",
            "--data" | "-d" => "Send data in the HTTP request body",
            "--data-raw" => "Send request data without treating @ specially",
            "--fail" | "-f" => "Return failure for HTTP response codes 400 and above",
            "--follow" | "--location" | "-L" => "Follow HTTP redirect responses",
            "--form" => "Send multipart form data",
            "--head" | "-I" => "Fetch response headers without downloading the response body",
            "--header" | "-H" => "Add or replace an HTTP request header",
            "--include" => "Include response headers in the normal output",
            "--insecure" => "Skip TLS certificate verification",
            "--max-time" => "Limit the total transfer time",
            "--output" | "-o" => "Write response data to the specified file",
            "--proxy" => "Route the request through the specified proxy",
            "--request" | "-X" => "Use the specified HTTP request method",
            "--retry" => "Retry transient transfer failures",
            "--silent" | "-s" => "Hide the progress meter and normal error output",
            "--user" | "-u" => "Send server authentication credentials",
            "--user-agent" => "Set the HTTP User-Agent header",
            "--verbose" | "-v" => "Show detailed request, response, and connection diagnostics",
            _ => "curl behavior",
        },
        "df-schema" => match value {
            "--all" | "-a" => "Include pseudo, duplicate, or inaccessible file systems",
            "--human-readable" | "-h" => "Print sizes in powers of 1024 using readable units",
            "--inodes" | "-i" => "Show inode usage instead of block usage",
            "--print-type" | "-T" => "Print each file system's type",
            _ => "df behavior",
        },
        "du-schema" => match value {
            "--all" | "-a" => "Count every file, not only directories",
            "--apparent-size" => "Show apparent file sizes instead of disk usage",
            "--human-readable" | "-h" => "Print sizes in readable units",
            "--max-depth=" => "Limit directory totals to the specified depth",
            "--one-file-system" | "-x" => "Skip directories on other file systems",
            "--summarize" | "-s" => "Show one total for each argument",
            _ => "du behavior",
        },
        "command-lookup-schema" => match value {
            "--all" | "-a" => "Show every matching executable found in PATH",
            "--help" => "Show help for the command lookup utility",
            "--version" | "-V" => "Show version information",
            "-v" => "Show how the shell would resolve the command",
            _ => "Command lookup behavior",
        },
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
        let checkout = candidates
            .iter()
            .find(|candidate| candidate.insert_text == "checkout")
            .expect("checkout candidate");
        assert_eq!(checkout.description, "Switch branches or restore files");
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
        let all = candidates
            .iter()
            .find(|candidate| candidate.insert_text == "--all")
            .expect("--all candidate");
        let amend = candidates
            .iter()
            .find(|candidate| candidate.insert_text == "--amend")
            .expect("--amend candidate");

        assert_eq!(
            all.description,
            "Stage modified and deleted tracked files before committing"
        );
        assert_eq!(
            amend.description,
            "Replace the tip commit with a new commit"
        );
    }

    #[test]
    fn dedicated_option_schemas_do_not_fall_back_to_generic_labels() {
        let schemas: &[(&[&str], &str, &str)] = &[
            (
                super::GIT_COMMIT_OPTIONS,
                "git-commit-schema",
                "Git commit behavior",
            ),
            (
                super::GIT_CHECKOUT_OPTIONS,
                "git-checkout-schema",
                "Git checkout behavior",
            ),
            (
                super::GIT_SWITCH_OPTIONS,
                "git-switch-schema",
                "Git switch behavior",
            ),
            (super::GIT_LOG_OPTIONS, "git-log-schema", "Git log behavior"),
            (
                super::SYSTEMCTL_GLOBAL_OPTIONS,
                "systemctl-schema",
                "systemctl behavior",
            ),
            (
                super::DOCKER_LOGS_OPTIONS,
                "docker-logs-schema",
                "Docker logs behavior",
            ),
            (
                super::DOCKER_PS_OPTIONS,
                "docker-ps-schema",
                "Docker ps behavior",
            ),
            (
                super::DOCKER_EXEC_OPTIONS,
                "docker-exec-schema",
                "Docker exec behavior",
            ),
            (super::CARGO_BUILD_OPTIONS, "cargo-schema", "Cargo behavior"),
            (super::CARGO_TEST_OPTIONS, "cargo-schema", "Cargo behavior"),
            (super::APT_OPTIONS, "apt-schema", "APT behavior"),
            (
                super::JOURNALCTL_OPTIONS,
                "journalctl-schema",
                "journalctl behavior",
            ),
            (super::SSH_OPTIONS, "ssh-schema", "SSH behavior"),
            (super::SUDO_OPTIONS, "sudo-schema", "sudo behavior"),
            (super::FIND_OPTIONS, "find-schema", "find behavior"),
            (super::GREP_OPTIONS, "grep-schema", "grep behavior"),
            (super::TAR_OPTIONS, "tar-schema", "tar behavior"),
            (super::CURL_OPTIONS, "curl-schema", "curl behavior"),
        ];

        for (values, source, fallback) in schemas {
            for value in *values {
                assert_ne!(
                    super::candidate_description(source, "option", value),
                    *fallback,
                    "{source} {value} must have a behavior-specific description"
                );
            }
        }
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
    fn df_and_du_options_are_contextual_and_descriptive() {
        let df = super::suggest(&[], &UsageState::default(), "df --h", 6, 20);
        let human_readable = df
            .iter()
            .find(|candidate| candidate.insert_text == "--human-readable")
            .expect("df human-readable option");
        assert_eq!(
            human_readable.description,
            "Print sizes in powers of 1024 using readable units"
        );

        let du = super::suggest(&[], &UsageState::default(), "du --s", 6, 20);
        let summarize = du
            .iter()
            .find(|candidate| candidate.insert_text == "--summarize")
            .expect("du summarize option");
        assert_eq!(summarize.description, "Show one total for each argument");

        let wrong_command = super::suggest(&[], &UsageState::default(), "df --su", 7, 20);
        assert!(!wrong_command
            .iter()
            .any(|candidate| candidate.insert_text == "--summarize"));
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
