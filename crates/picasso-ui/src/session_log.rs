//! The session log: what happened since Picasso Suite started, kept in memory, so a bug can be
//! reported with it. Help > Export Log File saves it as a text file; nothing is written while the
//! program runs, and nothing is sent anywhere.
//!
//! It holds the version, the system, the graphics card, what the status line said (imports,
//! saves, renders, errors) and the warnings and errors the program and its graphics libraries
//! log. The last [`MAX_LINES`] lines are kept. The log names the files that were opened, so it
//! holds their paths.
//!
//! The one file written without asking is a crash report: if the program crashes, the log and
//! the crash (with where it happened) go into `crash-<time>.txt` in the data folder's `logs`
//! folder (`%APPDATA%\Picasso Suite\logs` on Windows), since the log in memory ends with the
//! program. The next Export Log File adds the latest crash report to the log it saves.

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// How many lines the log keeps (the oldest go first).
pub const MAX_LINES: usize = 5000;

/// How many crash reports are kept.
const KEEP_CRASHES: usize = 5;

struct Log {
    /// What is running where, at the top of the log.
    heading: String,
    lines: Mutex<VecDeque<String>>,
    started: Instant,
    /// The last status line noted, so one repeated every frame is noted once.
    last_status: Mutex<String>,
}

static LOG: OnceLock<Log> = OnceLock::new();

/// A date and time (UTC) from seconds since 1970, by the civil calendar.
fn civil(secs: u64) -> (i64, u64, u64, u64, u64, u64) {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u64::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u64::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day, rem / 3600, rem / 60 % 60, rem % 60)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `2026-10-27 23:10:05` (UTC).
fn stamp(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil(secs);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

/// A file name with the time in it, so they sort by time: `crash-2026-10-27_23-10-05.txt`.
fn file_name(prefix: &str, secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil(secs);
    format!("{prefix}-{y:04}-{mo:02}-{d:02}_{h:02}-{mi:02}-{s:02}.txt")
}

/// The files in `dir` named `<prefix>-*.txt`, oldest first.
fn reports(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&format!("{prefix}-")) && n.ends_with(".txt"))
        })
        .collect();
    found.sort();
    found
}

/// Deletes the oldest `<prefix>-*.txt` files in `dir` so that at most `keep` are left.
fn prune(dir: &Path, prefix: &str, keep: usize) {
    let found = reports(dir, prefix);
    let extra = found.len().saturating_sub(keep);
    for old in found.into_iter().take(extra) {
        let _ = std::fs::remove_file(old);
    }
}

/// Where crash reports go.
fn crash_dir() -> Option<PathBuf> {
    crate::project_manager::data_dir().map(|d| d.join("logs"))
}

/// Adds lines to `lines`, keeping at most `max`: `[  12.345s] INFO  text`.
fn push(lines: &mut VecDeque<String>, max: usize, secs: f64, level: &str, text: &str) {
    for (i, line) in text.lines().enumerate() {
        lines.push_back(if i == 0 {
            format!("[{secs:9.3}s] {level:<5} {line}")
        } else {
            format!("{:<19}{line}", "")
        });
    }
    while lines.len() > max {
        lines.pop_front();
    }
}

fn write_line(level: &str, text: &str) {
    let Some(log) = LOG.get() else { return };
    let secs = log.started.elapsed().as_secs_f64();
    if let Ok(mut lines) = log.lines.lock() {
        push(&mut lines, MAX_LINES, secs, level, text);
    }
}

/// The log as text: its heading, then its lines.
fn text() -> String {
    let Some(log) = LOG.get() else {
        return String::new();
    };
    let mut out = format!("{}\n{}\n", log.heading, "-".repeat(72));
    if let Ok(lines) = log.lines.lock() {
        for line in lines.iter() {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The `tracing` events of every crate (warnings and errors, or more with `PICASSO_LOG`),
/// added to the log.
struct TracingWriter;

impl Write for TracingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let text = text.trim_end();
        if !text.is_empty() {
            write_line("TRACE", text);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Starts this session's log (in memory): a heading that says what is running where, the
/// `tracing` events of every crate, and a crash report written if the program crashes.
pub fn start() {
    let secs = now_secs();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let heading = format!(
        "Picasso Suite session log\n\
         Version:  {}\n\
         Started:  {} UTC\n\
         System:   {} {} ({} logical processors)\n\
         Command:  {}",
        crate::update::version_line(),
        stamp(secs),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(0, |n| n.get()),
        if args.is_empty() {
            "(no arguments)".to_string()
        } else {
            args.join(" ")
        },
    );
    let log = Log {
        heading,
        lines: Mutex::new(VecDeque::new()),
        started: Instant::now(),
        last_status: Mutex::new(String::new()),
    };
    if LOG.set(log).is_err() {
        return;
    }

    // Warnings and errors from every crate; `PICASSO_LOG=info` (or debug, trace) for more.
    let level = std::env::var("PICASSO_LOG")
        .ok()
        .and_then(|l| l.parse::<tracing_subscriber::filter::LevelFilter>().ok())
        .unwrap_or(tracing_subscriber::filter::LevelFilter::WARN);
    let _ = tracing_subscriber::fmt()
        .with_writer(|| TracingWriter)
        .with_ansi(false)
        .without_time()
        .with_max_level(level)
        .try_init();

    // A crash ends the program and the log in memory with it, so the log and the crash (with
    // where it happened) are written to a crash report first.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current();
        write_line(
            "CRASH",
            &format!(
                "the program stopped on thread '{}': {info}\n{backtrace}",
                thread.name().unwrap_or("unnamed")
            ),
        );
        if let Some(dir) = crash_dir() {
            if std::fs::create_dir_all(&dir).is_ok() {
                let _ = std::fs::write(dir.join(file_name("crash", now_secs())), text());
                prune(&dir, "crash", KEEP_CRASHES);
            }
        }
        previous(info);
    }));
}

/// Notes something in the log (`level` is INFO, WARN or ERROR).
pub fn note(level: &str, text: &str) {
    write_line(level, text);
}

/// Notes what the status line shows, unless it says the same as the last time.
pub(crate) fn status(error: bool, text: &str) {
    let Some(log) = LOG.get() else { return };
    if let Ok(mut last) = log.last_status.lock() {
        if *last == text {
            return;
        }
        *last = text.to_string();
    }
    write_line(if error { "ERROR" } else { "INFO" }, text);
}

/// The log to save: this session's, then the latest crash report, if the program crashed before.
fn export_text() -> String {
    let mut out = text();
    let latest = crash_dir().and_then(|d| reports(&d, "crash").pop());
    if let Some(path) = latest {
        if let Ok(crash) = std::fs::read_to_string(&path) {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            out.push_str(&format!(
                "\n{}\nThe last crash report ({}):\n\n{crash}",
                "=".repeat(72),
                name.unwrap_or_default()
            ));
        }
    }
    out
}

impl crate::app::StudioApp {
    /// Help > Export Log File: asks where to save this session's log (and the last crash
    /// report, if there is one) as a text file.
    pub(crate) fn export_log(&mut self, ctx: &egui::Context) {
        if LOG.get().is_none() {
            self.fail("there is no log for this session");
            return;
        }
        note("INFO", "Log exported (Help > Export Log File)");
        crate::platform::save_as(
            self.inbox.clone(),
            ctx.clone(),
            file_name("picasso-suite-log", now_secs()),
            ("Text file", vec!["txt"]),
            export_text().into_bytes(),
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_keeps_its_newest_lines() {
        let mut lines = VecDeque::new();
        for i in 0..10 {
            push(&mut lines, 4, f64::from(i), "INFO", &format!("line {i}"));
        }
        push(&mut lines, 4, 10.5, "CRASH", "first\nsecond");
        let got: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(
            got,
            [
                "[    8.000s] INFO  line 8",
                "[    9.000s] INFO  line 9",
                "[   10.500s] CRASH first",
                "                   second",
            ]
        );
    }

    #[test]
    fn names_sort_by_time_and_old_reports_are_deleted() {
        assert_eq!(file_name("crash", 0), "crash-1970-01-01_00-00-00.txt");
        assert_eq!(stamp(1_798_000_000), "2026-12-23 04:26:40");
        let dir = std::env::temp_dir().join(format!("picasso-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
        for i in 0..5 {
            std::fs::write(dir.join(file_name("crash", 1_000_000 + i * 60)), "x")
                .unwrap_or_else(|e| panic!("{e}"));
        }
        std::fs::write(dir.join("notes.txt"), "kept").unwrap_or_else(|e| panic!("{e}"));
        prune(&dir, "crash", 2);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{e}"))
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                file_name("crash", 1_000_180),
                file_name("crash", 1_000_240),
                "notes.txt".to_string()
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
