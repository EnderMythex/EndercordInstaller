// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

pub static DEBUG: AtomicBool = AtomicBool::new(false);

/// Lines shown in the GUI log panel.
pub static HISTORY: Mutex<Vec<(Level, String)>> = Mutex::new(Vec::new());

pub fn log(level: Level, msg: String) {
    if level == Level::Debug && !DEBUG.load(Ordering::Relaxed) {
        return;
    }

    let (label, color) = match level {
        Level::Debug => ("DEBUG", "\x1b[90m"),
        Level::Info => ("INFO ", "\x1b[36m"),
        Level::Warn => ("WARN ", "\x1b[33m"),
        Level::Error => ("ERROR", "\x1b[31m"),
    };

    if cfg!(windows) {
        eprintln!("{label} {msg}");
    } else {
        eprintln!("{color}{label}\x1b[0m {msg}");
    }

    if level != Level::Debug {
        HISTORY.lock().unwrap().push((level, msg));
    }
}

#[macro_export]
macro_rules! debug { ($($t:tt)*) => { $crate::log::log($crate::log::Level::Debug, format!($($t)*)) } }
#[macro_export]
macro_rules! info { ($($t:tt)*) => { $crate::log::log($crate::log::Level::Info, format!($($t)*)) } }
#[macro_export]
macro_rules! warn { ($($t:tt)*) => { $crate::log::log($crate::log::Level::Warn, format!($($t)*)) } }
#[macro_export]
macro_rules! error { ($($t:tt)*) => { $crate::log::log($crate::log::Level::Error, format!($($t)*)) } }
