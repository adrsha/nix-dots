//! Event-driven client snapshots, decoded with Python's standard JSON library.
use super::*;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Client {
    pub id: u32,
    pub tags: Vec<u32>,
    pub output: String,
    pub app: String,
    pub title: String,
}
pub(super) struct Clients {
    pub list: Vec<Client>,
}
impl Service for Clients {
    fn new() -> Self {
        Self { list: vec![] }
    }
    fn listen() {
        loop {
            let Ok(mut child) = Command::new("python3")
                .args(["-u", "-c", include_str!("clients.py")])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            else {
                return;
            };
            let mut next = vec![];
            if let Some(stdout) = child.stdout.take() {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    match line.as_str() {
                        "SNAP" => next.clear(),
                        "END" => {
                            let changed = Self::read().list != next;
                            if changed {
                                Self::write().list = next.clone();
                            }
                        }
                        _ => {
                            if let Some(client) = decode(&line) {
                                next.push(client);
                            }
                        }
                    }
                }
            }
            let _ = child.wait();
            Self::write().list.clear();
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
fn unhex(s: &str) -> Option<String> {
    if s.len() % 2 != 0 {
        return None;
    }
    let bytes = s
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}
fn decode(s: &str) -> Option<Client> {
    let mut f = s.split('\t');
    Some(Client {
        id: f.next()?.parse().ok()?,
        tags: f
            .next()?
            .split(',')
            .filter_map(|x| x.parse().ok())
            .collect(),
        output: unhex(f.next()?)?,
        app: unhex(f.next()?)?,
        title: unhex(f.next()?)?,
    })
}
pub(super) fn on_workspace(output: &str, tag: u32) -> Vec<Client> {
    Clients::read()
        .list
        .iter()
        .filter(|c| c.output == output && c.tags.contains(&tag))
        .cloned()
        .collect()
}
pub(super) fn focus(id: u32) {
    popups::dismiss();
    std::thread::spawn(move || {
        let Ok(path) = std::env::var("MANGO_INSTANCE_SIGNATURE") else {
            return;
        };
        if let Ok(mut stream) = UnixStream::connect(path) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            if writeln!(stream, "dispatch focusid,client,{id}").is_ok() {
                let mut reply = String::new();
                let _ = BufReader::new(stream).read_line(&mut reply);
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_transport_handles_utf8_and_escaped_titles() {
        let c = decode("12\t2,3\t44502d31\t6b69747479\t61f09f98800a7b7d").unwrap();
        assert_eq!(c.tags, vec![2, 3]);
        assert_eq!(c.title, "a😀\n{}");
        assert!(decode("bad").is_none());
        assert!(unhex("0").is_none());
    }
}
