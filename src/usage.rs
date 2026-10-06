use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

const USAGE_VERSION: u32 = 1;
const MAX_ENTRIES: usize = 512;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct UsageState {
    version: u32,
    counts: BTreeMap<String, u64>,
}

impl UsageState {
    pub(crate) fn load() -> Self {
        let Some(path) = state_path() else {
            return Self::empty();
        };
        let Ok(raw) = fs::read_to_string(path) else {
            return Self::empty();
        };
        let Ok(state) = serde_json::from_str::<Self>(&raw) else {
            return Self::empty();
        };
        if state.version != USAGE_VERSION {
            return Self::empty();
        }
        state
    }

    pub(crate) fn record(value: &str) -> Result<(), String> {
        let value = value.trim();
        if value.is_empty() || value.len() > 4096 {
            return Ok(());
        }

        let mut state = Self::load();
        *state.counts.entry(value.to_owned()).or_insert(0) += 1;
        state.prune();

        let Some(path) = state_path() else {
            return Ok(());
        };
        write_state(&path, &state)
    }

    pub(crate) fn boost(&self, value: &str) -> i64 {
        let count = self.counts.get(value).copied().unwrap_or(0);
        if count == 0 {
            return 0;
        }

        let capped = count.min(64);
        120 * i64::try_from(capped).unwrap_or(64)
    }

    pub(crate) fn entries(&self) -> usize {
        self.counts.len()
    }

    pub(crate) fn total_events(&self) -> u64 {
        self.counts.values().copied().sum()
    }

    fn empty() -> Self {
        Self {
            version: USAGE_VERSION,
            counts: BTreeMap::new(),
        }
    }

    fn prune(&mut self) {
        if self.counts.len() <= MAX_ENTRIES {
            return;
        }

        let mut ranked: Vec<(String, u64)> = self
            .counts
            .iter()
            .map(|(key, count)| (key.clone(), *count))
            .collect();

        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.len().cmp(&b.0.len()))
                .then_with(|| a.0.cmp(&b.0))
        });
        ranked.truncate(MAX_ENTRIES);

        self.counts = ranked.into_iter().collect();
    }
}

pub(crate) fn state_path() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir).join("termsense/usage-v1.json"));
    }

    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".local/state/termsense/usage-v1.json"))
}

fn write_state(path: &Path, state: &UsageState) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err("invalid usage state path".into());
    };
    fs::create_dir_all(parent).map_err(|err| format!("create usage state directory: {err}"))?;

    let mut temp = path.to_path_buf();
    temp.set_extension(format!("tmp-{}", std::process::id()));

    let bytes =
        serde_json::to_vec(state).map_err(|err| format!("serialize usage state: {err}"))?;
    let mut file = fs::File::create(&temp).map_err(|err| format!("create usage state: {err}"))?;
    file.write_all(&bytes)
        .map_err(|err| format!("write usage state: {err}"))?;
    file.sync_all()
        .map_err(|err| format!("sync usage state: {err}"))?;
    fs::rename(&temp, path).map_err(|err| format!("replace usage state: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::UsageState;
    use std::collections::BTreeMap;

    #[test]
    fn usage_boost_is_monotonic_and_capped() {
        let mut counts = BTreeMap::new();
        counts.insert("docker".to_owned(), 2);
        counts.insert("git".to_owned(), 100);

        let state = UsageState {
            version: 1,
            counts,
        };

        assert!(state.boost("git") > state.boost("docker"));
        assert_eq!(state.boost("missing"), 0);
        assert_eq!(state.boost("git"), 120 * 64);
    }
}
