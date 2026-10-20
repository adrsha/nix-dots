//! Read-only link fallback for machines using networkd instead of NetworkManager.
use super::*;
#[derive(Clone, PartialEq, Default)]
pub(super) struct Snapshot {
    pub managed: bool,
    pub links: Vec<String>,
}
struct Status {
    snapshot: Snapshot,
}
impl Service for Status {
    fn new() -> Self {
        Self {
            snapshot: Snapshot::default(),
        }
    }
    fn listen() {
        loop {
            let next = fetch();
            if Self::read().snapshot != next {
                Self::write().snapshot = next;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }
}
pub(super) fn snapshot() -> Snapshot {
    Status::read().snapshot.clone()
}
fn fetch() -> Snapshot {
    let managed = std::process::Command::new("busctl")
        .args([
            "--system",
            "--no-pager",
            "status",
            "org.freedesktop.NetworkManager",
        ])
        .output()
        .is_ok_and(|o| o.status.success());
    let mut links = vec![];
    if let Ok(list) = std::fs::read_dir("/sys/class/net") {
        for entry in list.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "lo" {
                continue;
            }
            let state = std::fs::read_to_string(entry.path().join("operstate"))
                .unwrap_or_else(|_| "unknown".into());
            let addresses = std::process::Command::new("ip")
                .args(["-brief", "address", "show", "dev", &name])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .split_whitespace()
                        .skip(2)
                        .collect::<Vec<_>>()
                        .join("  ")
                })
                .unwrap_or_default();
            links.push(format!("{name} · {}  {addresses}", state.trim()));
        }
    }
    links.sort();
    Snapshot { managed, links }
}
