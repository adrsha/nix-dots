// Brightness for an external monitor over DDC/CI, with ddcutil.
//
// ddcutil needs a second or more per call, so the shell never waits on it.
// `brightness` (the IPC handler) only records what you asked for, and a worker
// thread sends it. Keys pressed quickly collapse into the latest value.
//
//   amane ipc call brightness up
//   amane ipc call brightness down
//   amane ipc call brightness 40

use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;
use std::time::Duration;

use amane::Service;

use crate::island::{Island, Osd};

// the monitor to control, see `ddcutil detect`
const DISPLAY: &str = "1";

// brightness is VCP feature 0x10
const FEATURE: &str = "10";

const STEP: i32 = 5;

const UNKNOWN: i32 = -1;

// the percent you want, or UNKNOWN until the monitor has been read
static WANTED: AtomicI32 = AtomicI32::new(UNKNOWN);

pub fn percent() -> Option<u8> {
    let value = WANTED.load(Ordering::Relaxed);
    (value != UNKNOWN).then_some(value.clamp(0, 100) as u8)
}

pub fn start() {
    thread::spawn(worker);
}

pub fn brightness(arguments: &[String]) -> String {
    let Some(argument) = arguments.first() else {
        return String::from("usage: brightness up|down|show|<0-100>");
    };

    let current = WANTED.load(Ordering::Relaxed);

    if current == UNKNOWN {
        return String::from("brightness isn't ready, is the monitor readable by ddcutil?");
    }

    if argument == "show" {
        Island::write().show_osd(Osd::Brightness, current as u8, false);
        return current.to_string();
    }

    let next = match argument.as_str() {
        "up" => current + STEP,
        "down" => current - STEP,
        number => match number.parse::<i32>() {
            Ok(number) => number,
            Err(_) => return format!("bad argument: {number}"),
        },
    }
    .clamp(0, 100);

    WANTED.store(next, Ordering::Relaxed);

    Island::write().show_osd(Osd::Brightness, next as u8, false);

    next.to_string()
}

fn worker() {
    let mut warned = false;

    // the monitor may not answer right at login, so keep asking
    let (mut applied, maximum) = loop {
        if let Some(found) = read() {
            break found;
        }

        if !warned {
            eprintln!(
                "ddc: can't read the brightness, check `ddcutil detect` and DISPLAY in ddc.rs"
            );

            warned = true;
        }

        thread::sleep(Duration::from_secs(5));
    };

    WANTED.store(applied, Ordering::Relaxed);

    loop {
        thread::sleep(Duration::from_millis(150));

        let wanted = WANTED.load(Ordering::Relaxed);

        if wanted == applied {
            continue;
        }

        // --noverify skips reading the value back, which is the slow half
        amane::output(&format!(
            "ddcutil --display {DISPLAY} --noverify setvcp {FEATURE} {}",
            wanted * maximum / 100
        ));

        applied = wanted;
    }
}

// the current percent and the highest value the monitor accepts
fn read() -> Option<(i32, i32)> {
    let text = amane::output(&format!(
        "ddcutil --display {DISPLAY} --brief getvcp {FEATURE}"
    ));

    // "VCP 10 C 45 100": feature, type, current value, highest value
    let mut parts = text.split_whitespace().skip(3);

    let current: i32 = parts.next()?.parse().ok()?;
    let maximum: i32 = parts.next()?.parse().ok()?;

    if maximum <= 0 {
        return None;
    }

    Some((current * 100 / maximum, maximum))
}
