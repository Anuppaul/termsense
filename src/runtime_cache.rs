use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

pub(crate) fn root_path() -> Option<PathBuf> {
    env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|root| root.join("termsense"))
}

pub(crate) fn load_lines(key: &str, max_age: Duration) -> Option<Vec<String>> {
    let path = cache_path(key)?;
    let metadata = fs::metadata(&path).ok()?;
    let modified = metadata.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    if age > max_age {
        return None;
    }

    let raw = fs::read_to_string(path).ok()?;
    Some(
        raw.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

pub(crate) fn store_lines(key: &str, values: &[String]) {
    let Some(path) = cache_path(key) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };

    if fs::create_dir_all(parent).is_err() {
        return;
    }

    let mut body = values.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }

    let mut temp = path.clone();
    temp.set_extension(format!("tmp-{}", std::process::id()));

    if fs::write(&temp, body).is_ok() {
        let _ = fs::rename(temp, path);
    } else {
        let _ = fs::remove_file(temp);
    }
}

pub(crate) fn clear() {
    if let Some(root) = root_path() {
        let _ = fs::remove_dir_all(root);
    }
}

fn cache_path(key: &str) -> Option<PathBuf> {
    if key.is_empty()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return None;
    }

    root_path().map(|root| root.join(format!("{key}.txt")))
}

#[cfg(test)]
mod tests {
    use super::cache_path;

    #[test]
    fn rejects_path_traversal_keys() {
        assert!(cache_path("../secret").is_none());
        assert!(cache_path("docker-containers-v1").is_some() || std::env::var_os("XDG_RUNTIME_DIR").is_none());
    }
}
