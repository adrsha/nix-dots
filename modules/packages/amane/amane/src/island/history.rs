//! Private, bounded notification history. Disk IO never holds a service lock.
use super::*;
use std::{
    fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};
const LIMIT: usize = 200;
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Entry {
    pub key: u64,
    pub app: String,
    pub title: String,
    pub body: String,
    pub time: u64,
    pub icon: String,
    pub image: String,
    pub actions: Vec<(String, String)>,
    pub progress: Option<u8>,
    // Never serialized: server IDs must not survive a shell restart.
    pub live_id: Option<u32>,
}
pub(super) struct History {
    pub entries: Vec<Entry>,
}
impl Service for History {
    fn new() -> Self {
        Self {
            entries: fs::metadata(path())
                .ok()
                .filter(|m| m.len() <= 16 * 1024 * 1024)
                .and_then(|_| fs::read_to_string(path()).ok())
                .map(|s| decode(&s))
                .unwrap_or_default(),
        }
    }
    fn listen() {
        let mut previous = String::new();
        loop {
            // Images arriving through D-Bus may live in temporary paths. Copy
            // bounded local files into private history storage outside locks.
            let mut entries = Self::read().entries.clone();
            for entry in &mut entries {
                entry.icon = cache_image(&entry.icon);
                entry.image = cache_image(&entry.image);
            }
            let changed = Self::read().entries.iter().any(|e| {
                entries
                    .iter()
                    .any(|c| e.key == c.key && (e.icon != c.icon || e.image != c.image))
            });
            if changed {
                let mut state = Self::write();
                for cached in &entries {
                    if let Some(e) = state.entries.iter_mut().find(|e| e.key == cached.key) {
                        e.icon = cached.icon.clone();
                        e.image = cached.image.clone();
                    }
                }
            }
            let snapshot = encode(&entries);
            if snapshot != previous {
                if let Err(e) = save(&snapshot) {
                    eprintln!("amane history: {e}");
                } else {
                    previous = snapshot;
                    prune_images(&entries);
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}
fn path() -> PathBuf {
    super::settings::state_dir().join("notifications.v1")
}
fn save(text: &str) -> std::io::Result<()> {
    let path = path();
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    let tmp = path.with_extension("tmp");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    fs::rename(tmp, path)
}
pub(super) fn record(n: &amane::Notification) {
    let time = n
        .received()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let key = time.as_nanos() as u64 ^ u64::from(n.id());
    let mut state = History::write();
    let entry = Entry {
        key,
        app: n.app_name().chars().take(256).collect(),
        title: n.summary().chars().take(2048).collect(),
        body: n.body().chars().take(8192).collect(),
        time: time.as_secs(),
        icon: n.icon().chars().take(4096).collect(),
        image: n.image().chars().take(4096).collect(),
        actions: n
            .actions()
            .iter()
            .take(16)
            .map(|a| {
                (
                    a.key().chars().take(256).collect(),
                    a.label().chars().take(512).collect(),
                )
            })
            .collect(),
        progress: notice_hints::value(n.id()),
        live_id: Some(n.id()),
    };
    if let Some(old) = state.entries.iter_mut().find(|e| e.live_id == Some(n.id())) {
        let old_key = old.key;
        *old = entry;
        old.key = old_key;
    } else {
        state.entries.push(entry);
    }
    if state.entries.len() > LIMIT {
        state.entries.remove(0);
    }
}
pub(super) fn remove(key: u64) {
    History::write().entries.retain(|e| e.key != key);
}
pub(super) fn clear() {
    History::write().entries.clear();
}
pub(super) fn update_progress(id: u32, value: Option<u8>) {
    if value.is_none()
        || !History::read()
            .entries
            .iter()
            .any(|e| e.live_id == Some(id) && e.progress != value)
    {
        return;
    }
    if let Some(e) = History::write()
        .entries
        .iter_mut()
        .find(|e| e.live_id == Some(id))
    {
        e.progress = value;
    }
}
fn prune_images(entries: &[Entry]) {
    let dir = super::settings::state_dir().join("notification-images");
    let Ok(files) = fs::read_dir(&dir) else {
        return;
    };
    for file in files.flatten() {
        let name = file.file_name().to_string_lossy().into_owned();
        let Some((hash, _)) = name.split_once('.') else {
            continue;
        };
        if hash.len() != 16 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let path = file.path().to_string_lossy().into_owned();
        if !entries.iter().any(|e| e.icon == path || e.image == path) {
            let _ = fs::remove_file(file.path());
        }
    }
}
fn cache_image(value: &str) -> String {
    let path = PathBuf::from(value.strip_prefix("file://").unwrap_or(value));
    if !path.is_absolute()
        || path.starts_with(super::settings::state_dir().join("notification-images"))
    {
        return value.into();
    }
    let Ok(meta) = fs::metadata(&path) else {
        return value.into();
    };
    if !meta.is_file() || meta.len() > 12 * 1024 * 1024 {
        return String::new();
    }
    let Ok(bytes) = fs::read(&path) else {
        return value.into();
    };
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |a, b| {
        (a ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    let dir = super::settings::state_dir().join("notification-images");
    if fs::create_dir_all(&dir).is_err() {
        return value.into();
    }
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("png");
    let cached = dir.join(format!("{hash:016x}.{extension}"));
    if !cached.exists() {
        if let Ok(mut f) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&cached)
        {
            if f.write_all(&bytes).is_err() {
                return value.into();
            }
        }
    }
    cached.to_string_lossy().into_owned()
}
fn hex(s: &str) -> String {
    s.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Option<String> {
    if s.len() % 2 != 0 {
        return None;
    }
    let bytes = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}
fn encode(entries: &[Entry]) -> String {
    entries
        .iter()
        .map(|e| {
            let actions = e
                .actions
                .iter()
                .map(|(k, v)| format!("{}:{}", hex(k), hex(v)))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                e.key,
                e.time,
                hex(&e.app),
                hex(&e.title),
                hex(&e.body),
                hex(&e.icon),
                hex(&e.image),
                actions,
                e.progress.map(|v| v.to_string()).unwrap_or_default()
            )
        })
        .collect()
}
fn decode(text: &str) -> Vec<Entry> {
    text.lines()
        .rev()
        .take(LIMIT)
        .filter_map(|line| {
            if line.len() > 65536 {
                return None;
            }
            let f = line.split('\t').collect::<Vec<_>>();
            if f.len() != 5 && f.len() != 9 {
                return None;
            }
            let (icon, image, actions, progress) = if f.len() == 9 {
                let actions = f[7]
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .take(16)
                    .filter_map(|s| {
                        let (k, v) = s.split_once(':')?;
                        Some((unhex(k)?, unhex(v)?))
                    })
                    .collect();
                (
                    unhex(f[5])?,
                    unhex(f[6])?,
                    actions,
                    f[8].parse::<u8>().ok().filter(|v| *v <= 100),
                )
            } else {
                (String::new(), String::new(), vec![], None)
            };
            Some(Entry {
                key: f[0].parse().ok()?,
                time: f[1].parse().ok()?,
                app: unhex(f[2])?,
                title: unhex(f[3])?,
                body: unhex(f[4])?,
                icon,
                image,
                actions,
                progress,
                live_id: None,
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_roundtrip_unicode_and_markup() {
        let list = vec![Entry {
            key: 5,
            time: 99,
            icon: "test-icon".into(),
            image: "/tmp/test.png".into(),
            actions: vec![("open".into(), "Open this".into())],
            progress: Some(45),
            live_id: None,
            app: "メール".into(),
            title: "one\ttwo".into(),
            body: "<b>Test</b>\n& hi".into(),
        }];
        assert_eq!(decode(&encode(&list)), list);
        assert!(decode("invalid").is_empty());
    }
}

#[cfg(test)]
mod rich_tests {
    use super::*;
    #[test]
    fn legacy_text_entries_are_migrated() {
        let list = decode("12\t99\t417070\t5469746c65\t426f6479");
        assert_eq!(list.len(), 1);
        assert!(list[0].actions.is_empty());
        assert_eq!(list[0].live_id, None);
    }
    #[test]
    fn notification_ids_never_survive_restart() {
        let e = Entry {
            key: 1,
            time: 9,
            app: "App".into(),
            title: "Title".into(),
            body: "Body".into(),
            icon: "icon".into(),
            image: "".into(),
            actions: vec![("a".into(), "Go".into())],
            progress: Some(60),
            live_id: Some(44),
        };
        let stored = decode(&encode(&[e]));
        assert_eq!(stored[0].progress, Some(60));
        assert_eq!(stored[0].actions[0].1, "Go");
        assert_eq!(stored[0].live_id, None);
    }
}
