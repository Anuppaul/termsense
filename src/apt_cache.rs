use std::{
    collections::BTreeSet,
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime},
};
use wait_timeout::ChildExt;

const CACHE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

pub(crate) fn packages() -> Vec<String> {
    if let Some(path) = cache_path() {
        if cache_is_fresh(&path) {
            if let Ok(raw) = fs::read_to_string(&path) {
                let packages = parse_lines(&raw);
                if !packages.is_empty() {
                    return packages;
                }
            }
        }

        let packages = refresh_packages();
        if !packages.is_empty() {
            let _ = write_cache(&path, &packages);
        }
        return packages;
    }

    refresh_packages()
}

pub(crate) fn cache_path() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir).join("termsense/apt-packages-v1.txt"));
    }

    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".cache/termsense/apt-packages-v1.txt"))
}

fn refresh_packages() -> Vec<String> {
    if let Some(output) = run_bounded("apt-cache", &["pkgnames"], 1200) {
        let packages = parse_lines(&output);
        if !packages.is_empty() {
            return packages;
        }
    }

    installed_packages()
}

fn installed_packages() -> Vec<String> {
    let Ok(raw) = fs::read_to_string("/var/lib/dpkg/status") else {
        return Vec::new();
    };

    let mut packages = BTreeSet::new();
    for line in raw.lines() {
        if let Some(name) = line.strip_prefix("Package: ") {
            let name = name.trim();
            if !name.is_empty() {
                packages.insert(name.to_owned());
            }
        }
    }

    packages.into_iter().collect()
}

fn parse_lines(raw: &str) -> Vec<String> {
    let mut packages = BTreeSet::new();
    for line in raw.lines() {
        let value = line.trim();
        if !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
        {
            packages.insert(value.to_owned());
        }
    }
    packages.into_iter().collect()
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

fn write_cache(path: &Path, packages: &[String]) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err("invalid apt cache path".into());
    };
    fs::create_dir_all(parent).map_err(|err| format!("create apt cache directory: {err}"))?;

    let mut body = packages.join("\n");
    body.push('\n');

    let mut temp = path.to_path_buf();
    temp.set_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temp, body).map_err(|err| format!("write apt cache: {err}"))?;
    fs::rename(&temp, path).map_err(|err| format!("replace apt cache: {err}"))?;
    Ok(())
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
    use super::parse_lines;

    #[test]
    fn package_parser_deduplicates_and_filters() {
        let packages = parse_lines("curl\ncurl\nlibssl3\ninvalid name\n");
        assert_eq!(packages, vec!["curl", "libssl3"]);
    }
}
