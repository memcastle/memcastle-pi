//! The Pi history source: the conversation history of the Pi coding agent, read from its session files.
//!
//! Pi writes one JSONL file per session under `<root>/<working directory, slugged>/`, appending one entry per line as
//! the session goes on. This source reads those files directly: no Pi process, no model, no tokens, and it works on
//! history from sessions that ended long ago.
//!
//! What it files, per session: a header (session id and working directory) and each user and assistant message as text,
//! with the time it was written. What it leaves out on purpose: model reasoning (large, and not what was said), tool
//! results (large, and usually the contents of files that can be mined as files), and everything that is not a
//! conversation message. Tool *calls* are kept as one-line markers so the shape of the work stays visible.
//! A line that is not valid JSON, or an entry type this source does not know, is skipped rather than failing the
//! session: Pi's format is Pi's to evolve, and a reader that stopped at the first surprise would stop mining all history.
//!
//! The format is Pi's session format version 3. The source never opens anything but `*.jsonl` files, and in particular
//! not Pi's credentials file. Sessions are appended to, so a modification-time watermark finds the ones that grew, and
//! MemCastle's revision check plus chunk hashes make re-ingesting a grown session cost only its new tail.
//!
//! Acquisition is all this does: MemCastle chunks, deduplicates, files and remembers what it returns.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

wit_bindgen::generate!({
    path: "wit",
    world: "source",
});

use exports::memcastle::source::adapter::Guest;
use memcastle::source::types::{
    Candidate, CanonicalDocument, Discovery, RawDocument, Segment, SourceError, SourceKind,
    SourceRef,
};

/// This source's name; it must match `source.name` in the manifest.
const NAME: &str = "pi";

/// The longest title taken from a session's first prompt, in characters.
const TITLE_CHARS: usize = 80;

struct Pi;

/// A session file found by the walk: its path under the root (the document's identity) and its modification time.
struct Entry {
    key: String,
    mtime_ns: i64,
}

/// Where Pi keeps its sessions, when the caller names no locator. `HOME` is the one variable the manifest lists.
fn default_root() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok().filter(|home| !home.is_empty())?;
    Some(
        PathBuf::from(home)
            .join(".pi")
            .join("agent")
            .join("sessions"),
    )
}

/// Collect the session files under `root`: `*.jsonl` directly in it and in each directory one level down (Pi's layout
/// is one directory per working directory). Symlinks are not followed, and nothing but `.jsonl` is ever listed.
fn collect(root: &Path, out: &mut Vec<Entry>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if file_type.is_file() {
            push_if_session(&entry, name, out);
        } else if file_type.is_dir() {
            let Ok(inner) = std::fs::read_dir(entry.path()) else {
                continue;
            };
            for session in inner.flatten() {
                let Ok(inner_type) = session.file_type() else {
                    continue;
                };
                let Some(inner_name) = session.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                if inner_type.is_file() {
                    push_if_session(&session, format!("{name}/{inner_name}"), out);
                }
            }
        }
    }
}

/// Add `entry` as `key` if it is a `.jsonl` file.
fn push_if_session(entry: &std::fs::DirEntry, key: String, out: &mut Vec<Entry>) {
    if !Path::new(&key)
        .extension()
        .is_some_and(|extension| extension == "jsonl")
    {
        return;
    }
    if let Ok(metadata) = entry.metadata() {
        out.push(Entry {
            key,
            mtime_ns: mtime_ns(&metadata),
        });
    }
}

fn mtime_ns(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_nanos()).ok())
        .unwrap_or(0)
}

/// The number of days in `month` of `year`.
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to a civil date (after Howard Hinnant), the inverse of the conversion to a date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// A `since` value (`2026-09`, `2026-09-14` or an RFC 3339 time) as epoch milliseconds, UTC.
///
/// Written out rather than taken from a crate: it is all a source needs of a calendar, and a component carries what it
/// links.
fn parse_since(value: &str) -> Result<i64, String> {
    let value = value.trim();
    let bad =
        || format!("`{value}` is not a date; use `2026-09`, `2026-09-14` or an RFC 3339 time");
    let number = |text: &str, digits: usize| -> Option<i64> {
        if text.len() == digits && text.bytes().all(|byte| byte.is_ascii_digit()) {
            text.parse().ok()
        } else {
            None
        }
    };
    let (date, time) = match value.find(['T', 't']) {
        Some(at) => (&value[..at], Some(&value[at + 1..])),
        None => (value, None),
    };
    let mut parts = date.split('-');
    let year = parts
        .next()
        .and_then(|part| number(part, 4))
        .ok_or_else(bad)?;
    let month = parts
        .next()
        .and_then(|part| number(part, 2))
        .filter(|month| (1..=12).contains(month))
        .ok_or_else(bad)?;
    let day = match parts.next() {
        Some(part) => number(part, 2).ok_or_else(bad)?,
        None => 1,
    };
    if parts.next().is_some() || day < 1 || day > days_in_month(year, month) {
        return Err(bad());
    }
    let mut seconds = days_from_civil(year, month, day) * 86_400;
    let mut fraction_ms = 0;
    if let Some(time) = time {
        // The zone is `Z` or `+HH:MM`/`-HH:MM`; without one the time would mean something different on every machine.
        let (clock, offset) = if let Some(clock) = time.strip_suffix(['Z', 'z']) {
            (clock, 0)
        } else {
            let at = time.rfind(['+', '-']).ok_or_else(bad)?;
            let (clock, zone) = time.split_at(at);
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let (hours, minutes) = zone[1..].split_once(':').ok_or_else(bad)?;
            let hours = number(hours, 2).ok_or_else(bad)?;
            let minutes = number(minutes, 2).ok_or_else(bad)?;
            (clock, sign * (hours * 3600 + minutes * 60))
        };
        let (whole, fraction) = match clock.split_once('.') {
            Some((whole, fraction)) => (whole, Some(fraction)),
            None => (clock, None),
        };
        let mut fields = whole.split(':');
        let mut next = |limit: i64| {
            fields
                .next()
                .and_then(|part| number(part, 2))
                .filter(|field| *field < limit)
        };
        let (hour, minute, second) = (
            next(24).ok_or_else(bad)?,
            next(60).ok_or_else(bad)?,
            next(60).ok_or_else(bad)?,
        );
        if fields.next().is_some() {
            return Err(bad());
        }
        seconds += hour * 3600 + minute * 60 + second - offset;
        if let Some(fraction) = fraction {
            if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(bad());
            }
            let padded: String = fraction.chars().chain("000".chars()).take(3).collect();
            fraction_ms = padded.parse().map_err(|_| bad())?;
        }
    }
    Ok(seconds * 1000 + fraction_ms)
}

/// The working directory a session was started in, from the first line of its file, or `None` when it has none.
fn cwd_of(path: &Path) -> Option<String> {
    use std::io::BufRead;
    let mut line = String::new();
    std::io::BufReader::new(std::fs::File::open(path).ok()?)
        .read_line(&mut line)
        .ok()?;
    let header: Value = serde_json::from_str(line.trim()).ok()?;
    header.get("cwd")?.as_str().map(str::to_string)
}

/// A `dir` value as it is compared with what OpenCode and Pi store, which is the working directory as the program saw it:
/// separated by `/`, with no empty or `.` segment and no trailing `/`, and with `..` resolved against the literal
/// segment before it. A segment holding a `*` is left alone, since what it stands for is not known.
///
/// Lexical only: this component has no file system to resolve a link with, so `memcastle mine` resolves one on the
/// machine it runs on before the value gets here.
fn normalize_dir(value: &str) -> String {
    let absolute = value.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." if parts
                .last()
                .is_some_and(|last| *last != ".." && !last.contains('*')) =>
            {
                parts.pop();
            }
            // `/..` is `/`: there is nothing above the root to go to.
            ".." if absolute && parts.is_empty() => {}
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    if absolute {
        format!("/{joined}")
    } else {
        joined
    }
}

/// Why `pattern` (already normalised) cannot be a directory or a pattern for one, or `None` when it can.
///
/// It has to name a place: a path from the root (or a drive on Windows), or a pattern that starts with `*`. Anything
/// else is relative to nothing, would match no session, and an empty result is the worst way to say so.
fn dir_problem(pattern: &str) -> Option<String> {
    let drive =
        pattern.as_bytes().get(1) == Some(&b':') && pattern.as_bytes()[0].is_ascii_alphabetic();
    if pattern.is_empty() || pattern.chars().any(char::is_control) {
        Some(
            "`dir` must be a directory, or a pattern such as `/work/*`, with no control characters"
                .to_string(),
        )
    } else if !(pattern.starts_with('/') || pattern.starts_with('*') || drive) {
        Some(format!(
            "`dir` `{pattern}` is not an absolute path; give the full path (`/work/app`) or a pattern starting with `*`"
        ))
    } else {
        None
    }
}

/// Whether `text` matches `pattern`, where `*` stands for any run of characters (`/` included) and every other
/// character is itself. Linear in the text: the usual two-pointer walk that backs up to the last `*`, so a pattern full
/// of stars cannot make a long path slow.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    // Where the last `*` was, and how much of the text it has swallowed so far.
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if p < pattern.len() && pattern[p] == text[t] {
            p += 1;
            t += 1;
        } else if let Some((star_p, star_t)) = star {
            p = star_p + 1;
            t = star_t + 1;
            star = Some((star_p, star_t + 1));
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

/// The cursor `{"mtime_ns", "key"}` means "everything ordered at or before this is done"; `null` is the beginning.
fn parse_cursor(cursor: &str) -> Result<Option<(i64, String)>, SourceError> {
    let invalid = |message: &str| SourceError::CursorInvalid(message.to_string());
    let value: Value =
        serde_json::from_str(cursor).map_err(|_| invalid("the cursor is not JSON"))?;
    if value.is_null() {
        return Ok(None);
    }
    let mtime_ns = value
        .get("mtime_ns")
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid("`mtime_ns` is missing or not an integer"))?;
    let key = value
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("`key` is missing or not a string"))?;
    Ok(Some((mtime_ns, key.to_string())))
}

fn path_of(source: &SourceRef, key: &str) -> PathBuf {
    key.split('/')
        .fold(PathBuf::from(&source.locator), |path, part| path.join(part))
}

/// FNV-1a over the body, with its length: a revision must change when the content does, and nothing more is asked of it.
fn revision_of(body: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in body.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}-{}", body.len())
}

/// The session header entry (`{"type":"session", ...}`), which Pi writes as the first line; `None` when there is none.
fn header_of(body: &str) -> Option<Value> {
    body.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|entry| entry.get("type").and_then(Value::as_str) == Some("session"))
}

/// Whether `text` has the shape of an RFC 3339 timestamp (`2026-07-14T14:27:12.546Z`, or with a `+HH:MM` offset).
///
/// MemCastle fails the whole job on an `occurred-at` it cannot parse, so a header Pi wrote in some other shape is left
/// out of the document rather than taking every session with it.
fn looks_like_rfc3339(text: &str) -> bool {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .is_some_and(|b| b.iter().all(u8::is_ascii_digit))
    };
    let shape = digits(0..4)
        && bytes.get(4) == Some(&b'-')
        && digits(5..7)
        && bytes.get(7) == Some(&b'-')
        && digits(8..10)
        && matches!(bytes.get(10), Some(b'T' | b't' | b' '))
        && digits(11..13)
        && bytes.get(13) == Some(&b':')
        && digits(14..16)
        && bytes.get(16) == Some(&b':')
        && digits(17..19);
    if !shape {
        return false;
    }
    // Optional fraction, then `Z` or an offset.
    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let end = fraction
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(fraction.len());
        if end == 0 {
            return false;
        }
        rest = &fraction[end..];
    }
    match rest.as_bytes() {
        [b'Z' | b'z'] => true,
        [b'+' | b'-', h1, h2, b':', m1, m2] => [h1, h2, m1, m2].iter().all(|b| b.is_ascii_digit()),
        _ => false,
    }
}

/// The text of one Pi message worth filing, or `None` when it has none (reasoning only, a tool result, an unknown role).
fn message_text(role: &str, message: &Value) -> Option<String> {
    match role {
        "user" | "assistant" => {
            let text = match message.get("content")? {
                Value::String(text) => text.trim().to_string(),
                Value::Array(blocks) => blocks
                    .iter()
                    .filter_map(block_text)
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => return None,
            };
            (!text.trim().is_empty()).then_some(text)
        }
        // The command the user ran, not its output.
        "bashExecution" => message
            .get("command")
            .and_then(Value::as_str)
            .map(|command| format!("[bash: {}]", first_line(command))),
        _ => None,
    }
}

/// One content block as text: prose as it is, a tool call as a marker, anything else (reasoning, images) not at all.
fn block_text(block: &Value) -> Option<String> {
    match block.get("type").and_then(Value::as_str)? {
        "text" => {
            let text = block.get("text").and_then(Value::as_str)?.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        "toolCall" => {
            let name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
            Some(format!("[tool call: {name}]"))
        }
        _ => None,
    }
}

/// The first line of `text`, cut to [`TITLE_CHARS`] characters.
fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .trim()
        .chars()
        .take(TITLE_CHARS)
        .collect()
}

impl Guest for Pi {
    fn identify(
        locator: Option<String>,
        options: Vec<(String, String)>,
    ) -> Result<SourceRef, SourceError> {
        let mut normalised = Vec::new();
        let mut account = None;
        for (key, value) in options {
            match key.as_str() {
                // Only narrows what is read, so it is not part of the identity: an earlier `since` next time continues
                // from the same cursor, and `--full` is how to reach back.
                "since" => {
                    parse_since(&value).map_err(SourceError::InvalidInput)?;
                    // Checked now, kept as typed: `discover` parses it again, so a value that passes here cannot fail there.
                    normalised.push((key, value.trim().to_string()));
                }
                // Selects one working directory's sessions out of the same root, so it is part of the identity: each
                // directory has a cursor of its own, and a `dir` run never moves another's.
                //
                // The value is a directory or a pattern in which `*` matches any run of characters, `/` included
                // (`/work/*` is every project under `/work`). It is normalised the way Pi's own `cwd` is stored, so
                // `/work/app`, `/work/app/` and `/work/./app` are one `dir` and one cursor.
                "dir" => {
                    let dir = normalize_dir(value.trim());
                    if let Some(problem) = dir_problem(&dir) {
                        return Err(SourceError::InvalidInput(problem));
                    }
                    account = Some(format!("dir={dir}"));
                    normalised.push((key, dir));
                }
                other => {
                    return Err(SourceError::InvalidInput(format!(
                        "the pi source has no option `{other}`; it accepts `since` and `dir`"
                    )));
                }
            }
        }
        let root = match locator {
            Some(locator) => PathBuf::from(locator),
            None => default_root().ok_or_else(|| {
                SourceError::InvalidInput(
                    "cannot tell where Pi keeps its sessions without a home directory; give the sessions directory as the locator"
                        .to_string(),
                )
            })?,
        };
        // MemCastle hands over the canonical path of a directory it has opened for this source, so there is nothing
        // to canonicalise here; a locator that is not a readable directory is simply refused.
        if !root.is_dir() {
            return Err(SourceError::InvalidInput(format!(
                "{} is not a directory of Pi sessions this source can read",
                root.display()
            )));
        }
        Ok(SourceRef {
            source: NAME.to_string(),
            account,
            locator: root.display().to_string(),
            options: normalised,
        })
    }

    fn default_wing(_source: SourceRef) -> String {
        "pi".to_string()
    }

    fn default_room() -> String {
        "sessions".to_string()
    }

    fn discover(source: SourceRef, cursor: String, limit: u32) -> Result<Discovery, SourceError> {
        let after = parse_cursor(&cursor)?;
        let mut entries = Vec::new();
        collect(Path::new(&source.locator), &mut entries);
        // Oldest first, ties broken by path, so sessions saved in the same instant are neither skipped nor read twice.
        let option = |name: &str| {
            source
                .options
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        if let Some(since) = option("since").and_then(|at| parse_since(at).ok()) {
            let floor = since.saturating_mul(1_000_000);
            entries.retain(|entry| entry.mtime_ns >= floor);
        }
        if let Some(dir) = option("dir") {
            // Read last, on what is left: the first line of each remaining file is all it costs. A session with no
            // readable header has no directory to match, so it is not taken for this one.
            entries.retain(|entry| {
                cwd_of(&path_of(&source, &entry.key))
                    .is_some_and(|cwd| glob_match(dir, &normalize_dir(&cwd)))
            });
        }
        entries.sort_by(|a, b| (a.mtime_ns, &a.key).cmp(&(b.mtime_ns, &b.key)));
        entries.retain(|entry| {
            after.as_ref().is_none_or(|(mtime, key)| {
                (entry.mtime_ns, entry.key.as_str()) > (*mtime, key.as_str())
            })
        });
        let limit = limit as usize;
        let exhausted = entries.len() <= limit;
        entries.truncate(limit);
        Ok(Discovery {
            candidates: entries
                .into_iter()
                .map(|entry| Candidate {
                    cursor_after: json!({ "mtime_ns": entry.mtime_ns, "key": entry.key })
                        .to_string(),
                    handle: entry.key.clone(),
                    external_id: entry.key,
                })
                .collect(),
            exhausted,
        })
    }

    fn read(source: SourceRef, candidate: Candidate) -> Result<Option<RawDocument>, SourceError> {
        let path = path_of(&source, &candidate.handle);
        // A session being written right now can end mid-line or mid-character; the unfinished tail is picked up whole
        // on the next run, once the file has moved on. Any failure is a skip, not an error.
        let Ok(body) = std::fs::read_to_string(&path) else {
            return Ok(None);
        };
        if body.trim().is_empty() {
            return Ok(None);
        }
        let header = header_of(&body);
        let field = |name: &str| {
            header
                .as_ref()
                .and_then(|h| h.get(name))
                .cloned()
                .unwrap_or(Value::Null)
        };
        // The session's own start time is its identity in time; it does not move when the file is copied or restored.
        let occurred_at = field("timestamp")
            .as_str()
            .filter(|at| looks_like_rfc3339(at))
            .map(str::to_string);
        Ok(Some(RawDocument {
            external_id: candidate.external_id,
            revision: revision_of(&body),
            body,
            metadata: json!({
                "path": path.display().to_string(),
                "session_id": field("id"),
                "cwd": field("cwd"),
                "version": field("version"),
            })
            .to_string(),
            occurred_at,
        }))
    }

    fn normalize(raw: RawDocument) -> Result<CanonicalDocument, SourceError> {
        let mut header: Option<(String, Option<String>)> = None;
        let mut messages: Vec<Segment> = Vec::new();
        let mut title: Option<String> = None;

        for line in raw.body.lines() {
            let Ok(entry) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            match entry.get("type").and_then(Value::as_str) {
                Some("session") => {
                    let id = entry
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let cwd = entry.get("cwd").and_then(Value::as_str).map(str::to_string);
                    header = Some((id, cwd));
                }
                Some("message") => {
                    let Some(message) = entry.get("message") else {
                        continue;
                    };
                    let role = message
                        .get("role")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let Some(text) = message_text(role, message) else {
                        continue;
                    };
                    if title.is_none() && role == "user" {
                        title = Some(first_line(&text));
                    }
                    let at = entry
                        .get("timestamp")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    messages.push(Segment {
                        text: format!("### {role} · {at}\n{text}\n\n"),
                    });
                }
                // Model and thinking-level changes, labels, compactions and anything newer: not conversation.
                _ => {}
            }
        }

        // A session with no messages files nothing (MemCastle still remembers its revision).
        let mut segments = Vec::new();
        if !messages.is_empty() {
            let (id, cwd) = header.clone().unwrap_or_default();
            segments.push(Segment {
                text: format!(
                    "# Pi session {id}\nworking directory: {}\n\n",
                    cwd.as_deref().unwrap_or("unknown")
                ),
            });
            segments.extend(messages);
        }

        let room = header
            .as_ref()
            .and_then(|(_, cwd)| cwd.as_deref())
            .and_then(|cwd| Path::new(cwd).file_name())
            .map(|name| name.to_string_lossy().into_owned());
        let name = Path::new(&raw.external_id)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned());
        let metadata: Value = serde_json::from_str(&raw.metadata).unwrap_or(Value::Null);

        Ok(CanonicalDocument {
            title,
            room,
            name,
            kind: SourceKind::Transcript,
            uri: metadata
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string),
            tags: vec!["transcript".to_string(), "pi".to_string()],
            segments,
        })
    }
}

export!(Pi);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TREE: AtomicU64 = AtomicU64::new(0);

    struct SessionTree(PathBuf);

    impl SessionTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "memcastle-pi-test-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn put(&self, name: &str, body: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, body).unwrap();
            path
        }

        fn source(&self, options: Vec<(&str, &str)>) -> SourceRef {
            Pi::identify(
                Some(self.0.display().to_string()),
                options
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
            )
            .unwrap()
        }
    }

    impl Drop for SessionTree {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn candidate(key: &str) -> Candidate {
        Candidate {
            handle: key.into(),
            external_id: key.into(),
            cursor_after: "null".into(),
        }
    }

    fn raw(body: &str, metadata: &str) -> RawDocument {
        RawDocument {
            external_id: "directory/session.jsonl".into(),
            revision: revision_of(body),
            body: body.into(),
            metadata: metadata.into(),
            occurred_at: None,
        }
    }

    const SESSION: &str = include_str!(
        "../fixtures/pi-sessions/tree/--home-me-project--/2026-07-14T14-27-12-546Z_019f6106.jsonl"
    );

    #[test]
    fn a_pi_session_keeps_the_conversation_but_never_its_tool_results_or_reasoning() {
        let document = Pi::normalize(RawDocument {
            external_id: "--home-me-project--/2026-07-14T14-27-12-546Z_019f6106.jsonl".into(),
            revision: revision_of(SESSION),
            body: SESSION.into(),
            metadata: json!({"path": "/sessions/session.jsonl"}).to_string(),
            occurred_at: None,
        })
        .unwrap();
        let text: String = document
            .segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect();
        assert!(text.starts_with("# Pi session 019f6106"), "{text}");
        assert!(text.contains("How do I rotate the signing keys?"));
        assert!(text.contains("[tool call: read]"));
        assert!(!text.contains("SECRET-TOOL-OUTPUT"));
        assert!(!text.contains("PRIVATE-REASONING"));
        assert_eq!(
            document.title.as_deref(),
            Some("How do I rotate the signing keys?")
        );
        assert_eq!(document.room.as_deref(), Some("project"));
        assert_eq!(document.kind, SourceKind::Transcript);
        assert_eq!(document.tags, ["transcript", "pi"]);
        assert_eq!(document.uri.as_deref(), Some("/sessions/session.jsonl"));
    }

    #[test]
    fn dates_and_directory_filters_keep_their_original_scope_rules() {
        assert_eq!(parse_since("2026-09"), parse_since("2026-09-01"));
        assert!(parse_since("not-a-date").is_err());
        assert_eq!(normalize_dir("/work/app/../project/"), "/work/project");
        assert!(glob_match("/work/*", "/work/project"));
        assert!(!glob_match("/work/app", "/work/project"));
    }

    #[test]
    fn identify_validates_options_and_keeps_only_directory_in_the_account() {
        let tree = SessionTree::new();
        let source = tree.source(vec![
            ("since", " 2026-09 "),
            ("dir", " /work/./app/../pi/ "),
        ]);
        assert_eq!(source.source, "pi");
        assert_eq!(source.locator, tree.0.display().to_string());
        assert_eq!(source.account.as_deref(), Some("dir=/work/pi"));
        assert_eq!(
            source.options,
            [
                ("since".into(), "2026-09".into()),
                ("dir".into(), "/work/pi".into())
            ]
        );
        assert_eq!(Pi::default_wing(source.clone()), "pi");
        assert_eq!(Pi::default_room(), "sessions");
        assert_eq!(tree.source(vec![("since", "2025-01")]).account, None);
        assert_eq!(
            tree.source(vec![("dir", "*/repo")]).account.as_deref(),
            Some("dir=*/repo")
        );

        for options in [
            vec![("unknown", "yes")],
            vec![("since", "2025-02-30")],
            vec![("dir", "relative")],
            vec![("dir", "\n")],
        ] {
            assert!(matches!(
                Pi::identify(
                    Some(tree.0.display().to_string()),
                    options
                        .into_iter()
                        .map(|(k, v)| (k.into(), v.into()))
                        .collect()
                ),
                Err(SourceError::InvalidInput(_))
            ));
        }
        assert!(matches!(
            Pi::identify(Some(tree.0.join("absent").display().to_string()), vec![]),
            Err(SourceError::InvalidInput(_))
        ));
    }

    #[test]
    fn default_locator_uses_home_when_its_sessions_directory_exists() {
        let expected = std::env::var("HOME")
            .ok()
            .filter(|home| !home.is_empty())
            .map(|home| PathBuf::from(home).join(".pi/agent/sessions"));
        assert_eq!(default_root(), expected);
        match expected {
            Some(path) if path.is_dir() => {
                assert_eq!(
                    Pi::identify(None, vec![]).unwrap().locator,
                    path.display().to_string()
                );
            }
            _ => assert!(matches!(
                Pi::identify(None, vec![]),
                Err(SourceError::InvalidInput(_))
            )),
        }
    }

    #[test]
    fn discovery_walks_only_sessions_and_pages_by_mtime_and_key() {
        let tree = SessionTree::new();
        let session = "{\"type\":\"session\",\"cwd\":\"/work/pi\"}\n";
        tree.put("a.jsonl", session);
        tree.put("project/b.jsonl", session);
        tree.put(
            "project/c.jsonl",
            "{\"type\":\"session\",\"cwd\":\"/work/other\"}\n",
        );
        tree.put("project/b.txt", session);
        tree.put("project/deep/d.jsonl", session);
        tree.put("other/no-header.jsonl", "invalid json\n");
        #[cfg(unix)]
        std::os::unix::fs::symlink(tree.0.join("a.jsonl"), tree.0.join("link.jsonl")).unwrap();
        let source = tree.source(vec![]);
        let all = Pi::discover(source.clone(), "null".into(), 20).unwrap();
        let keys: Vec<_> = all.candidates.iter().map(|c| c.handle.as_str()).collect();
        assert_eq!(keys.len(), 4);
        assert!(keys.contains(&"a.jsonl"));
        assert!(keys.contains(&"project/b.jsonl"));
        assert!(keys.contains(&"project/c.jsonl"));
        assert!(keys.contains(&"other/no-header.jsonl"));
        assert!(all.exhausted);
        assert_eq!(
            all.candidates
                .iter()
                .map(|c| c.external_id.as_str())
                .collect::<Vec<_>>(),
            keys
        );

        let mut cursor = "null".to_string();
        let mut paged = Vec::new();
        loop {
            let page = Pi::discover(source.clone(), cursor.clone(), 1).unwrap();
            if let Some(last) = page.candidates.last() {
                cursor = last.cursor_after.clone();
                let value: Value = serde_json::from_str(&cursor).unwrap();
                assert_eq!(value["key"], last.handle);
            }
            paged.extend(page.candidates.into_iter().map(|c| c.handle));
            if page.exhausted {
                break;
            }
        }
        assert_eq!(paged, keys);

        let filtered = tree.source(vec![("dir", "/work/pi")]);
        let matching = Pi::discover(filtered.clone(), "null".into(), 10).unwrap();
        assert_eq!(matching.candidates.len(), 2);
        assert!(
            matching
                .candidates
                .iter()
                .all(|c| c.handle == "a.jsonl" || c.handle == "project/b.jsonl")
        );
        assert!(
            Pi::discover(tree.source(vec![("since", "9999-01")]), "null".into(), 10)
                .unwrap()
                .candidates
                .is_empty()
        );
        assert_eq!(
            Pi::discover(source, "null".into(), 0)
                .unwrap()
                .candidates
                .len(),
            0
        );
        assert!(Pi::discover(filtered, "null".into(), 0).is_ok());
    }

    #[test]
    fn discovery_rejects_invalid_cursors_and_tolerates_missing_roots() {
        let tree = SessionTree::new();
        let source = tree.source(vec![]);
        for cursor in [
            "garbage",
            "{}",
            "{\"mtime_ns\":1}",
            "{\"mtime_ns\":\"1\",\"key\":\"a\"}",
            "{\"mtime_ns\":1,\"key\":2}",
        ] {
            assert!(matches!(
                Pi::discover(source.clone(), cursor.into(), 1),
                Err(SourceError::CursorInvalid(_))
            ));
        }
        assert!(
            Pi::discover(source.clone(), "null".into(), 1)
                .unwrap()
                .exhausted
        );
        let missing = SourceRef {
            locator: tree.0.join("missing").display().to_string(),
            ..source
        };
        assert!(
            Pi::discover(missing, "null".into(), 1)
                .unwrap()
                .candidates
                .is_empty()
        );
    }

    #[test]
    fn read_uses_session_header_and_content_revision_and_skips_bad_files() {
        let tree = SessionTree::new();
        let source = tree.source(vec![]);
        let body = "garbage\n{\"type\":\"session\",\"id\":\"id-1\",\"cwd\":\"/work/pi\",\"version\":3,\"timestamp\":\"2026-07-14T14:27:12.546Z\"}\n";
        let path = tree.put("project/session.jsonl", body);
        let document = Pi::read(source.clone(), candidate("project/session.jsonl"))
            .unwrap()
            .unwrap();
        assert_eq!(document.external_id, "project/session.jsonl");
        assert_eq!(document.body, body);
        assert_eq!(document.revision, revision_of(body));
        assert_eq!(
            document.occurred_at.as_deref(),
            Some("2026-07-14T14:27:12.546Z")
        );
        let metadata: Value = serde_json::from_str(&document.metadata).unwrap();
        assert_eq!(metadata["path"], path.display().to_string());
        assert_eq!(metadata["session_id"], "id-1");
        assert_eq!(metadata["cwd"], "/work/pi");
        assert_eq!(metadata["version"], 3);

        std::fs::write(&path, format!("{body}more\n")).unwrap();
        let changed = Pi::read(source.clone(), candidate("project/session.jsonl"))
            .unwrap()
            .unwrap();
        assert_ne!(changed.revision, document.revision);
        tree.put("empty.jsonl", " \n");
        assert!(
            Pi::read(source.clone(), candidate("empty.jsonl"))
                .unwrap()
                .is_none()
        );
        assert!(
            Pi::read(source.clone(), candidate("absent.jsonl"))
                .unwrap()
                .is_none()
        );
        tree.put(
            "bad.jsonl",
            "{\"type\":\"session\",\"timestamp\":\"not-a-date\"}\n",
        );
        let bad = Pi::read(source.clone(), candidate("bad.jsonl"))
            .unwrap()
            .unwrap();
        assert_eq!(bad.occurred_at, None);
        assert_eq!(
            serde_json::from_str::<Value>(&bad.metadata).unwrap()["session_id"],
            Value::Null
        );
        tree.put("invalid.jsonl", "not json\n");
        assert_eq!(
            Pi::read(source, candidate("invalid.jsonl"))
                .unwrap()
                .unwrap()
                .occurred_at,
            None
        );
    }

    #[test]
    fn normalize_skips_noise_and_handles_missing_headers_and_metadata() {
        let body = concat!(
            "not json\n",
            "{\"type\":\"label\"}\n",
            "{\"type\":\"message\"}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"toolResult\",\"content\":\"secret\"}}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":\"  \"}}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":42}}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"  Hello\\n again  \"}}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"bashExecution\",\"command\":\"git status\\nsecond line\"}}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"thinking\",\"text\":\"private\"},{\"type\":\"text\",\"text\":\" \"},{\"type\":\"text\",\"text\":\" Answer \"},{\"type\":\"toolCall\"}]}}\n"
        );
        let document = Pi::normalize(raw(body, "invalid metadata")).unwrap();
        assert_eq!(document.title.as_deref(), Some("Hello"));
        assert_eq!(document.room, None);
        assert_eq!(document.name.as_deref(), Some("session"));
        assert_eq!(document.uri, None);
        assert_eq!(document.segments.len(), 4);
        assert_eq!(
            document.segments[0].text,
            "# Pi session \nworking directory: unknown\n\n"
        );
        assert!(document.segments[1].text.contains("Hello\n again"));
        assert!(document.segments[2].text.contains("[bash: git status]"));
        assert!(
            document.segments[3]
                .text
                .contains("Answer\n[tool call: tool]")
        );
        assert!(
            !document
                .segments
                .iter()
                .any(|s| s.text.contains("secret") || s.text.contains("private"))
        );

        let empty = Pi::normalize(raw("{\"type\":\"session\",\"id\":\"empty\"}\n", "{}")).unwrap();
        assert!(empty.segments.is_empty());
        assert_eq!(empty.title, None);
        let header = Pi::normalize(raw("{\"type\":\"session\",\"id\":\"latest\",\"cwd\":\"/work/notes\"}\n{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":\"Done\"}}\n", "{\"path\":\"/tmp/pi.jsonl\"}")).unwrap();
        assert_eq!(header.room.as_deref(), Some("notes"));
        assert_eq!(header.title, None);
        assert_eq!(header.uri.as_deref(), Some("/tmp/pi.jsonl"));
        assert!(header.segments[0].text.contains("Pi session latest"));
    }

    #[test]
    fn timestamps_dates_paths_and_globs_cover_boundaries() {
        assert_eq!(
            parse_since("2024-02-29"),
            parse_since("2024-02-29T00:00:00Z")
        );
        assert_eq!(
            parse_since("2026-09-14T12:30:20.123456+02:30"),
            parse_since("2026-09-14T10:00:20.123Z")
        );
        assert_eq!(
            parse_since("2026-09-14T10:00:20.5-02:00"),
            parse_since("2026-09-14T12:00:20.500Z")
        );
        assert_eq!(
            parse_since("2026-09-14t10:00:20z"),
            parse_since("2026-09-14T10:00:20Z")
        );
        for bad in [
            "1900-02-29",
            "2026-00",
            "2026-13",
            "2026-04-31",
            "2026-01-00",
            "2026-01-01-01",
            "2026-01-01T00:00:00",
            "2026-01-01T24:00:00Z",
            "2026-01-01T00:60:00Z",
            "2026-01-01T00:00:60Z",
            "2026-01-01T00:00:00.Z",
            "2026-01-01T00:00:00.xZ",
            "2026-01-01T00:00:00+xx:00",
            "2026-01-01T00:00:00+0100",
            "2026-01-01T00:00:00Zextra",
        ] {
            assert!(parse_since(bad).is_err(), "{bad}");
        }
        assert_eq!(normalize_dir("/../../work//./app"), "/work/app");
        assert_eq!(normalize_dir("../work/../pi"), "../pi");
        assert_eq!(normalize_dir("/work/*/../pi"), "/work/*/../pi");
        assert!(dir_problem("C:/work").is_none());
        assert!(dir_problem("/work/\n").is_some());
        assert!(glob_match("/work/**/pi", "/work/a/b/pi"));
        assert!(glob_match("*", ""));
        assert!(glob_match("/work/*", "/work/"));
        assert!(!glob_match("/work/*/pi", "/work/other"));
        assert!(glob_match("/é*", "/école"));
        assert_eq!(first_line(&"é".repeat(90)).chars().count(), TITLE_CHARS);
        assert_eq!(first_line("  first  \nsecond"), "first");
        for (text, expected) in [
            ("2026-07-14T14:27:12Z", true),
            ("2026-07-14T14:27:12.546+02:00", true),
            ("2026-07-14 14:27:12z", true),
            ("2026-07-14T14:27:12.", false),
            ("2026-07-14T14:27:12+0:00", false),
            ("not a date", false),
        ] {
            assert_eq!(looks_like_rfc3339(text), expected, "{text}");
        }
    }
}
