//! Progress guard for read-only tool calls within one agent run.
//!
//! The loop detector in `loop_.rs` compares calls by their exact arguments and
//! only reacts after dozens of repeats. That misses a model that rotates
//! through a handful of files, re-reading each one although nothing changed,
//! until the context is full. This guard tracks reads per path and reports:
//!
//! - a re-read of content that has not changed (warn at the 3rd, block at the 6th);
//! - a long run of read-only calls with no write and no plan update (25, 50);
//! - a turn in which most of the recent reads were such re-reads (pause).
//!
//! Reading files that were not read before is never blocked. The guard also
//! keeps the list of files read so far, which compaction pins into the summary
//! so the model does not have to rediscover what it already looked at.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};

use serde_json::Value;

/// The re-read of unchanged content that draws a warning.
pub(crate) const REREAD_WARN_AT: usize = 3;
/// The re-read of unchanged content that is refused.
pub(crate) const REREAD_BLOCK_AT: usize = 6;
/// Consecutive read-only calls after which the user request is restated.
pub(crate) const READ_ONLY_STREAK_REMINDERS: [usize; 2] = [25, 50];
/// How many of the latest reads are considered when deciding to pause.
pub(crate) const RECENT_READ_WINDOW: usize = 40;

const PATH_KEYS: [&str; 3] = ["path", "file_path", "file"];
const MAX_RANGES_PER_PATH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadVerdict {
    Allow,
    /// Run the call, and tell the model it is repeating itself.
    Warn(String),
    /// Do not run the call; the text becomes its result.
    Block(String),
}

/// Why the turn should stop and hand back to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadPause {
    /// Unchanged re-reads among the latest reads.
    pub rereads: usize,
    /// Number of latest reads considered.
    pub window: usize,
    /// Files re-read most often, for the explanation.
    pub paths: Vec<String>,
}

#[derive(Debug, Default)]
struct RequestReads {
    last_content: u64,
    /// Times in a row this exact request returned what it returned before.
    rereads: usize,
}

#[derive(Debug, Default)]
struct PathReads {
    display: String,
    reads: usize,
    ranges: Vec<String>,
    requests: HashMap<u64, RequestReads>,
}

#[derive(Debug)]
struct RecentRead {
    key: String,
    unchanged: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ReadGuard {
    paths: HashMap<String, PathReads>,
    /// Path keys in first-read order.
    order: Vec<String>,
    recent: VecDeque<RecentRead>,
    read_only_streak: usize,
    /// Streak reminders already issued in the current streak.
    reminded: usize,
}

/// The file a read-only call targets, when its input names one.
pub(crate) fn read_target(input: &Value) -> Option<String> {
    PATH_KEYS.iter().find_map(|key| {
        input
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string)
    })
}

fn path_key(path: &str) -> String {
    let unified = path.trim().replace('\\', "/");
    let unified = unified.strip_prefix("./").unwrap_or(&unified);
    if cfg!(windows) {
        unified.to_lowercase()
    } else {
        unified.to_string()
    }
}

fn request_hash(name: &str, input: &Value) -> u64 {
    let mut normalized = input.clone();
    if let Some(obj) = normalized.as_object_mut() {
        obj.remove("_trace_id");
    }
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    normalized.to_string().hash(&mut hasher);
    hasher.finish()
}

fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Which part of the file a call asked for, in a form short enough to list.
fn range_label(input: &Value) -> String {
    let offset = input.get("offset").and_then(Value::as_u64);
    let limit = input.get("limit").and_then(Value::as_u64);
    if offset.is_some() || limit.is_some() {
        let start = offset.unwrap_or(1).max(1);
        return match limit {
            Some(limit) if limit > 0 => format!("L{}-{}", start, start + limit - 1),
            _ => format!("L{}+", start),
        };
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(obj) = input.as_object() {
        for (key, value) in obj {
            if key == "_trace_id" || PATH_KEYS.contains(&key.as_str()) {
                continue;
            }
            let text = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => continue,
            };
            let text: String = text.chars().take(40).collect();
            parts.push(format!("{}={}", key, text));
        }
    }
    if parts.is_empty() {
        "全文".to_string()
    } else {
        parts.join(", ")
    }
}

/// Plan and todo tools record progress but cannot change a file on disk.
fn is_plan_update(name: &str) -> bool {
    name.starts_with("plan_") || name.contains("todo")
}

impl ReadGuard {
    /// Decide, before a call runs, whether it repeats a read that keeps
    /// returning the same content.
    pub(crate) fn check(&mut self, name: &str, input: &Value, read_only: bool) -> ReadVerdict {
        if !read_only {
            return ReadVerdict::Allow;
        }
        let Some(path) = read_target(input) else {
            return ReadVerdict::Allow;
        };
        let key = path_key(&path);
        let Some(entry) = self.paths.get(&key) else {
            return ReadVerdict::Allow;
        };
        let Some(request) = entry.requests.get(&request_hash(name, input)) else {
            // A part of the file that was not requested before.
            return ReadVerdict::Allow;
        };
        let next = request.rereads + 1;
        if next >= REREAD_BLOCK_AT {
            let message = format!(
                "工具 '{}' 被阻止：{} 本轮已读取 {} 次，内容自上次读取后没有变化。不要再读取它，请基于已读到的内容继续任务；如果确实缺少某一部分，请读取尚未读过的行范围。",
                name, entry.display, entry.reads
            );
            self.push_recent(key, true);
            ReadVerdict::Block(message)
        } else if next >= REREAD_WARN_AT {
            ReadVerdict::Warn(format!(
                "工具 '{}' 正在第 {} 次重读 {}，内容自上次读取后没有变化（本轮已读取 {} 次）。请直接使用已读到的内容继续任务，继续重读将被阻止。",
                name, next, entry.display, entry.reads
            ))
        } else {
            ReadVerdict::Allow
        }
    }

    /// Record a call that ran.
    pub(crate) fn record(
        &mut self,
        name: &str,
        input: &Value,
        read_only: bool,
        content: &str,
        is_error: bool,
    ) {
        if !read_only {
            // A write or a plan update is progress.
            self.read_only_streak = 0;
            self.reminded = 0;
            if !is_error && !is_plan_update(name) {
                // It may also have changed files, so earlier reads no longer
                // predict what the next read returns.
                self.forget_rereads();
            }
            return;
        }
        self.read_only_streak += 1;
        if is_error {
            return;
        }
        let Some(path) = read_target(input) else {
            return;
        };
        let key = path_key(&path);
        if !self.paths.contains_key(&key) {
            self.order.push(key.clone());
        }
        let entry = self.paths.entry(key.clone()).or_default();
        if entry.display.is_empty() {
            entry.display = path;
        }
        entry.reads += 1;
        let label = range_label(input);
        if !entry.ranges.contains(&label) && entry.ranges.len() < MAX_RANGES_PER_PATH {
            entry.ranges.push(label);
        }
        let hash = content_hash(content);
        let request = entry.requests.entry(request_hash(name, input));
        let unchanged = match request {
            std::collections::hash_map::Entry::Occupied(mut seen) => {
                let seen = seen.get_mut();
                let unchanged = seen.last_content == hash;
                seen.rereads = if unchanged { seen.rereads + 1 } else { 0 };
                seen.last_content = hash;
                unchanged
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(RequestReads {
                    last_content: hash,
                    rereads: 0,
                });
                false
            }
        };
        self.push_recent(key, unchanged);
    }

    /// The compacted context may no longer hold what was read, so a read that
    /// follows is not a pointless repeat. The pause rule still sees it.
    pub(crate) fn on_context_compacted(&mut self) {
        self.forget_rereads();
    }

    /// `Some(n)` once, when the run of read-only calls reaches a reminder mark.
    pub(crate) fn take_streak_reminder(&mut self) -> Option<usize> {
        let mark = *READ_ONLY_STREAK_REMINDERS.get(self.reminded)?;
        if self.read_only_streak < mark {
            return None;
        }
        self.reminded += 1;
        Some(mark)
    }

    /// Set when more than half of the latest reads returned content the model
    /// had already been given.
    pub(crate) fn pause_reason(&self) -> Option<ReadPause> {
        let rereads = self.recent.iter().filter(|read| read.unchanged).count();
        if rereads * 2 <= RECENT_READ_WINDOW {
            return None;
        }
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for read in self.recent.iter().filter(|read| read.unchanged) {
            match counts.iter_mut().find(|(key, _)| *key == read.key) {
                Some((_, count)) => *count += 1,
                None => counts.push((read.key.as_str(), 1)),
            }
        }
        counts.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        let paths = counts
            .into_iter()
            .take(5)
            .map(|(key, _)| {
                self.paths
                    .get(key)
                    .map(|entry| entry.display.clone())
                    .unwrap_or_else(|| key.to_string())
            })
            .collect();
        Some(ReadPause {
            rereads,
            window: self.recent.len(),
            paths,
        })
    }

    /// Files read in this run, oldest first, limited to the latest `max`.
    /// Each line: path, the ranges requested, and how often it was read.
    pub(crate) fn files_read(&self, max: usize) -> Vec<String> {
        let skip = self.order.len().saturating_sub(max);
        self.order
            .iter()
            .skip(skip)
            .filter_map(|key| self.paths.get(key))
            .map(|entry| {
                format!(
                    "{}（{}；{} 次）",
                    entry.display,
                    entry.ranges.join("、"),
                    entry.reads
                )
            })
            .collect()
    }

    fn forget_rereads(&mut self) {
        for entry in self.paths.values_mut() {
            for request in entry.requests.values_mut() {
                request.rereads = 0;
            }
        }
    }

    fn push_recent(&mut self, key: String, unchanged: bool) {
        self.recent.push_back(RecentRead { key, unchanged });
        while self.recent.len() > RECENT_READ_WINDOW {
            self.recent.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// One model request for a file: check, then run it unless it was blocked.
    fn read(guard: &mut ReadGuard, path: &str, content: &str) -> ReadVerdict {
        let input = json!({ "path": path });
        let verdict = guard.check("file_read", &input, true);
        if !matches!(verdict, ReadVerdict::Block(_)) {
            guard.record("file_read", &input, true, content, false);
        }
        verdict
    }

    #[test]
    fn rotating_unchanged_files_warn_at_the_third_reread_block_at_the_sixth_and_pause() {
        let files = ["docs/a.md", "docs/b.md", "docs/c.md", "docs/d.md"];
        let mut guard = ReadGuard::default();
        let mut first_warning = None;
        let mut first_block = None;
        let mut paused_at = None;
        for call in 1..=60usize {
            let file = files[(call - 1) % files.len()];
            let verdict = read(&mut guard, file, "same content");
            // Read number for this file: 1 is the first read, n + 1 the n-th re-read.
            let nth_read = (call - 1) / files.len() + 1;
            match verdict {
                ReadVerdict::Warn(ref message) => {
                    assert!(message.contains("file_read") && message.contains(file), "{message}");
                    first_warning.get_or_insert(nth_read - 1);
                }
                ReadVerdict::Block(ref message) => {
                    assert!(message.contains("file_read") && message.contains(file), "{message}");
                    first_block.get_or_insert(nth_read - 1);
                }
                ReadVerdict::Allow => {
                    assert!(nth_read - 1 < REREAD_WARN_AT, "re-read {} was not flagged", nth_read - 1);
                }
            }
            if guard.pause_reason().is_some() {
                paused_at = Some(call);
                break;
            }
        }
        assert_eq!(first_warning, Some(3), "warning belongs to the 3rd re-read");
        assert_eq!(first_block, Some(6), "the 6th re-read is refused");
        let paused_at = paused_at.expect("the turn must pause");
        assert!(paused_at <= 60, "paused only at call {paused_at}");
        let pause = guard.pause_reason().unwrap();
        assert!(pause.rereads * 2 > RECENT_READ_WINDOW);
        assert_eq!(pause.paths.len(), 4);
    }

    #[test]
    fn sixty_distinct_files_are_never_blocked_and_only_draw_reminders() {
        let mut guard = ReadGuard::default();
        let mut reminders = Vec::new();
        for call in 1..=60 {
            let verdict = read(&mut guard, &format!("src/file_{call}.rs"), &format!("body {call}"));
            assert_eq!(verdict, ReadVerdict::Allow, "call {call}");
            assert!(guard.pause_reason().is_none(), "call {call}");
            if let Some(mark) = guard.take_streak_reminder() {
                reminders.push((call, mark));
            }
        }
        assert_eq!(reminders, vec![(25, 25), (50, 50)]);
    }

    #[test]
    fn a_changed_file_is_not_a_reread() {
        let mut guard = ReadGuard::default();
        for round in 0..10 {
            let verdict = read(&mut guard, "log.txt", &format!("line {round}"));
            assert_eq!(verdict, ReadVerdict::Allow, "round {round}");
        }
        assert!(guard.pause_reason().is_none());
    }

    #[test]
    fn a_new_range_of_a_file_already_read_is_allowed() {
        let mut guard = ReadGuard::default();
        for _ in 0..6 {
            read(&mut guard, "big.md", "head");
        }
        let next_part = json!({ "path": "big.md", "offset": 201, "limit": 200 });
        assert_eq!(guard.check("file_read", &next_part, true), ReadVerdict::Allow);
        // The part that keeps being re-read is still refused.
        assert!(matches!(read(&mut guard, "big.md", "head"), ReadVerdict::Block(_)));
    }

    #[test]
    fn a_write_clears_reread_counts_and_the_streak_but_a_plan_update_keeps_the_counts() {
        let mut guard = ReadGuard::default();
        for _ in 0..3 {
            read(&mut guard, "a.md", "same");
        }
        assert!(matches!(read(&mut guard, "a.md", "same"), ReadVerdict::Warn(_)));

        guard.record("plan_todo", &json!({ "items": [] }), false, "ok", false);
        assert!(
            matches!(read(&mut guard, "a.md", "same"), ReadVerdict::Warn(_)),
            "a plan update cannot have changed the file"
        );

        guard.record("file_write", &json!({ "path": "b.md" }), false, "written", false);
        assert_eq!(read(&mut guard, "a.md", "same"), ReadVerdict::Allow);

        // Streak: 30 reads, a plan update, then the count starts again.
        let mut guard = ReadGuard::default();
        for i in 0..30 {
            read(&mut guard, &format!("f{i}"), "x");
        }
        assert_eq!(guard.take_streak_reminder(), Some(25));
        assert_eq!(guard.take_streak_reminder(), None);
        guard.record("plan_todo", &json!({}), false, "ok", false);
        for i in 30..54 {
            read(&mut guard, &format!("f{i}"), "x");
        }
        assert_eq!(guard.take_streak_reminder(), None, "24 reads since the plan update");
        read(&mut guard, "f54", "x");
        assert_eq!(guard.take_streak_reminder(), Some(25));
    }

    #[test]
    fn compaction_allows_reading_again_but_does_not_hide_a_loop() {
        let mut guard = ReadGuard::default();
        for _ in 0..6 {
            read(&mut guard, "a.md", "same");
        }
        assert!(matches!(read(&mut guard, "a.md", "same"), ReadVerdict::Block(_)));
        guard.on_context_compacted();
        assert_eq!(read(&mut guard, "a.md", "same"), ReadVerdict::Allow);
        // 5 unchanged re-reads, 1 blocked attempt and 1 more re-read are still on record.
        let unchanged = guard.recent.iter().filter(|read| read.unchanged).count();
        assert_eq!(unchanged, 7);
    }

    #[test]
    fn failed_reads_and_calls_without_a_path_are_not_tracked_as_files() {
        let mut guard = ReadGuard::default();
        for _ in 0..10 {
            let input = json!({ "path": "missing.md" });
            assert_eq!(guard.check("file_read", &input, true), ReadVerdict::Allow);
            guard.record("file_read", &input, true, "not found", true);
            let search = json!({ "query": "same query" });
            assert_eq!(guard.check("web_search", &search, true), ReadVerdict::Allow);
            guard.record("web_search", &search, true, "same hits", false);
        }
        assert!(guard.files_read(10).is_empty());
        assert!(guard.pause_reason().is_none());
        // They still count as read-only calls without progress.
        assert_eq!(guard.take_streak_reminder(), None);
        for _ in 0..3 {
            guard.record("web_search", &json!({ "query": "q" }), true, "hits", false);
        }
        assert_eq!(guard.take_streak_reminder(), None);
        for _ in 0..2 {
            guard.record("web_search", &json!({ "query": "q" }), true, "hits", false);
        }
        assert_eq!(guard.take_streak_reminder(), Some(25));
    }

    #[test]
    fn files_read_lists_path_ranges_and_count() {
        let mut guard = ReadGuard::default();
        guard.record("file_read", &json!({ "path": "docs\\plan.md" }), true, "all", false);
        guard.record(
            "file_read",
            &json!({ "path": "docs/plan.md", "offset": 1, "limit": 200 }),
            true,
            "head",
            false,
        );
        guard.record(
            "file_read",
            &json!({ "path": "./src/main.rs", "offset": 50 }),
            true,
            "tail",
            false,
        );
        assert_eq!(
            guard.files_read(10),
            vec![
                "docs\\plan.md（全文、L1-200；2 次）".to_string(),
                "./src/main.rs（L50+；1 次）".to_string(),
            ]
        );
        assert_eq!(guard.files_read(1), vec!["./src/main.rs（L50+；1 次）".to_string()]);
    }
}
