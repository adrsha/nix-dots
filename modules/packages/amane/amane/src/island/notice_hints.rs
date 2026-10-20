//! Optional bridge for the `value` hint not exposed by Amane's notification API.
use super::*;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};
struct Hints {
    values: HashMap<u32, u8>,
}
impl Service for Hints {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
    fn listen() {
        if let Ok(mut child) = Command::new("python3")
            .args(["-u", "-c", include_str!("notice_hints.py")])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    let mut fields = line.split_whitespace();
                    if let (Some(id), Some(value)) = (
                        fields.next().and_then(|s| s.parse::<u32>().ok()),
                        fields.next().and_then(|s| s.parse::<i16>().ok()),
                    ) {
                        let mut state = Self::write();
                        if state.values.len() > 512 {
                            state.values.clear();
                        }
                        if value < 0 {
                            state.values.remove(&id);
                        } else {
                            state.values.insert(id, value.min(100) as u8);
                        }
                    }
                }
            }
            let _ = child.wait();
        }
    }
}
pub(super) fn value(id: u32) -> Option<u8> {
    Hints::read().values.get(&id).copied()
}
