//! Mango's event-driven fullscreen state, not a window-size heuristic.
use amane::Service;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

struct Fullscreen {
    hidden: bool,
}
impl Service for Fullscreen {
    fn new() -> Self {
        Self { hidden: false }
    }
    fn listen() {
        let Ok(path) = std::env::var("MANGO_INSTANCE_SIGNATURE") else {
            return;
        };
        loop {
            if let Ok(mut stream) = UnixStream::connect(&path) {
                if stream.write_all(b"watch all-clients\n").is_ok() {
                    for line in BufReader::new(stream).lines() {
                        let Ok(line) = line else {
                            break;
                        };
                        set(any_fullscreen(&line));
                    }
                }
            }
            set(false);
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
fn set(hidden: bool) {
    if Fullscreen::read().hidden != hidden {
        Fullscreen::write().hidden = hidden;
    }
}
pub(crate) fn blocked() -> bool {
    Fullscreen::read().hidden && !super::visibility::over_fullscreen()
}
pub(super) fn hidden() -> bool {
    blocked() || super::exposure::tucked()
}

// Extract only literal booleans at the top level. Skip escaped JSON strings
// (titles may contain quotes/field names), and ignore nested objects/arrays.
fn boolean(json: &str, key: &str) -> Option<bool> {
    let bytes = json.as_bytes();
    let mut i = 0;
    let mut depth = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                i += 1;
            }
            b'"' => {
                let start = i + 1;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else if bytes[i] == b'"' {
                        break;
                    } else {
                        i += 1;
                    }
                }
                let end = i.min(bytes.len());
                i += 1;
                if depth == 1 && &json[start..end] == key {
                    let rest = json.get(i..)?.trim_start().strip_prefix(':')?.trim_start();
                    if rest.starts_with("true,")
                        || rest.starts_with("true}")
                        || rest.starts_with("true ")
                    {
                        return Some(true);
                    }
                    if rest.starts_with("false,")
                        || rest.starts_with("false}")
                        || rest.starts_with("false ")
                    {
                        return Some(false);
                    }
                    return None;
                }
            }
            _ => i += 1,
        }
    }
    None
}
fn real_fullscreen(json: &str) -> bool {
    boolean(json, "is_visible") == Some(true)
        && boolean(json, "is_fullscreen") == Some(true)
        && boolean(json, "is_fakefullscreen") == Some(false)
}
// Each client is an object one level below the root. Keep string escaping
// separate from brace depth, so braces in window titles cannot split objects.
fn any_fullscreen(json: &str) -> bool {
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    for (i, byte) in json.bytes().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' => {
                    depth += 1;
                    if depth == 2 {
                        start = i;
                    }
                }
                b'}' => {
                    if depth == 2 && real_fullscreen(&json[start..=i]) {
                        return true;
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_fullscreen_need_not_be_focused() {
        assert!(any_fullscreen(
            r#"{"clients":[{"is_visible":true,"is_fullscreen":false,"is_fakefullscreen":false},{"title":"{}\"x", "is_visible":true,"is_fullscreen":true,"is_fakefullscreen":false}]}"#
        ));
        assert!(!any_fullscreen(
            r#"{"clients":[{"is_visible":false,"is_fullscreen":true,"is_fakefullscreen":false}]}"#
        ));
        assert!(!any_fullscreen(r#"{"clients":[]}"#));
    }
    #[test]
    fn distinguishes_real_from_fake_and_invisible() {
        assert!(real_fullscreen(
            r#"{"is_visible":true,"is_fullscreen":true,"is_fakefullscreen":false}"#
        ));
        for json in [
            r#"{"is_visible":true,"is_fullscreen":false,"is_fakefullscreen":true}"#,
            r#"{"is_visible":false,"is_fullscreen":true,"is_fakefullscreen":false}"#,
            r#"{"is_maximized":true}"#,
            r#"{"id":null}"#,
            "garbage",
        ] {
            assert!(!real_fullscreen(json));
        }
        assert_eq!(
            boolean(
                r#"{"title":"\\\"is_fullscreen\\\":true", "is_fullscreen":false}"#,
                "is_fullscreen"
            ),
            Some(false)
        );
    }
}
