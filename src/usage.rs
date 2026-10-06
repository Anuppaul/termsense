use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const USAGE_VERSION: u32 = 2;
const MAX_ENTRIES: usize = 512;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UsageEntry {
    count: u64,
    last_used_epoch: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct UsageState {
    version: u32,
    entries: BTreeMap<String, UsageEntry>,
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
        let now = now_epoch();
        let entry = state.entries.entry(value.to_owned()).or_default();
        entry.count = entry.count.saturating_add(1);
        entry.last_used_epoch = now;
        state.prune();

        let Some(path) = state_path() else {
            return Ok(());
        };
        write_state(&path, &state)
    }

    pub(crate) fn boost(&self, value: &str) -> i64 {
        let Some(entry) = self.entries.get(value) else {
            return 0;
        };

        let frequency = 60 * i64::try_from(entry.count.min(20)).unwrap_or(20);
        let age = now_epoch().saturating_sub(entry.last_used_epoch);
        let recency = match age {
            0..=3_600 => 400,
            3_601..=86_400 => 250,
            86_401..=604_800 => 100,
            _ => 0,
        };

        frequency + recency
    }

    pub(crate) fn entries(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn total_events(&self) -> u64 {
        self.entries.values().map(|entry| entry.count).sum()
    }

    fn empty() -> Self {
        Self {
            version: USAGE_VERSION,
            entries: BTreeMap::new(),
        }
    }

    fn prune(&mut self) {
        if self.entries.len() <= MAX_ENTRIES {
            return;
        }

        let mut ranked: Vec<(String, UsageEntry)> = self
            .entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.clone()))
            .collect();

        ranked.sort_by(|a, b| {
            b.1.last_used_epoch
                .cmp(&a.1.last_used_epoch)
                .then_with(|| b.1.count.cmp(&a.1.count))
                .then_with(|| a.0.len().cmp(&b.0.len()))
                .then_with(|| a.0.cmp(&b.0))
        });
        ranked.truncate(MAX_ENTRIES);
        self.entries = ranked.into_iter().collect();
    }
}

pub(crate) fn state_path() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir).join("termsense/usage-v2.json"));
    }

    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".local/state/termsense/usage-v2.json"))
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn write_state(path: &Path, state: &UsageState) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err("invalid usage state path".into());
    };
    fs::create_dir_all(parent).map_err(|err| format!("create usage state directory: {err}"))?;

    let mut temp = path.to_path_buf();
    temp.set_extension(format!("tmp-{}", std::process::id()));

    let bytes = serde_json::to_vec(state).map_err(|err| format!("serialize usage state: {err}"))?;
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
    use super::{UsageEntry, UsageState};
    use std::collections::BTreeMap;

    #[test]
    fn usage_boost_rewards_frequency_and_recency_without_overriding_exact_match() {
        let now = super::now_epoch();
        let mut entries = BTreeMap::new();
        entries.insert(
            "docker".to_owned(),
            UsageEntry {
                count: 2,
                last_used_epoch: now,
            },
        );
        entries.insert(
            "git".to_owned(),
            UsageEntry {
                count: 100,
                last_used_epoch: now,
            },
        );

        let state = UsageState {
            version: 2,
            entries,
        };

        assert!(state.boost("git") > state.boost("docker"));
        assert_eq!(state.boost("missing"), 0);
        assert!(state.boost("git") < 2_000);
    }
}
