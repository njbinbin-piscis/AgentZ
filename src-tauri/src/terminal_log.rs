pub(crate) const TERMINAL_BUFFER_MAX_LINES: usize = 5000;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 64 * 1024;

/// Rolling line buffer of PTY stdout/stderr for agent `terminal_read`.
#[derive(Default)]
pub struct TerminalOutputLog {
    lines: std::collections::VecDeque<String>,
    partial: String,
    bytes: usize,
}

impl TerminalOutputLog {
    pub fn append(&mut self, data: &str) {
        for segment in data.split_inclusive('\n') {
            // Keep the most recent UTF-8 tail even for progress output using \r.
            let segment = utf8_tail(segment, MAX_LINE_BYTES);
            let keep = MAX_LINE_BYTES.saturating_sub(segment.len());
            let tail = utf8_tail(&self.partial, keep);
            if tail.len() != self.partial.len() {
                self.partial = tail.to_owned();
            }
            self.partial.push_str(segment);
            if segment.ends_with('\n') {
                let line = std::mem::take(&mut self.partial);
                self.bytes += line.len();
                self.lines.push_back(line);
            }
            while self.lines.len() > TERMINAL_BUFFER_MAX_LINES
                || self.bytes + self.partial.len() > MAX_BYTES
            {
                if let Some(line) = self.lines.pop_front() {
                    self.bytes -= line.len();
                } else {
                    break;
                }
            }
        }
    }

    pub fn tail(&self, lines: usize) -> String {
        let n = lines.min(self.lines.len());
        self.lines
            .iter()
            .skip(self.lines.len().saturating_sub(n))
            .cloned()
            .collect()
    }

    pub fn grep_in_tail(&self, pattern: &str, search_lines: usize) -> String {
        let n = search_lines.min(self.lines.len());
        self.lines
            .iter()
            .skip(self.lines.len().saturating_sub(n))
            .filter(|l| l.contains(pattern))
            .cloned()
            .collect()
    }
}

fn utf8_tail(text: &str, max_bytes: usize) -> &str {
    let mut start = text.len().saturating_sub(max_bytes);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuous_output_and_long_lines_are_bounded() {
        let mut log = TerminalOutputLog::default();
        let data = "中".repeat(10_000);
        for _ in 0..1000 {
            log.append(&data);
        }
        assert!(log.partial.len() <= MAX_LINE_BYTES);
        for _ in 0..1000 {
            log.append(&format!("{data}\n"));
        }
        assert!(log.bytes + log.partial.len() <= MAX_BYTES);
        assert!(log.lines.len() <= TERMINAL_BUFFER_MAX_LINES);
        assert!(log.tail(1).ends_with('\n'));
    }
    #[test]
    fn retains_recent_lines_and_grep() {
        let mut log = TerminalOutputLog::default();
        for i in 0..6000 {
            log.append(&format!("line {i}\n"));
        }
        assert_eq!(log.lines.len(), 5000);
        assert_eq!(log.tail(2), "line 5998\nline 5999\n");
        assert_eq!(log.grep_in_tail("5998", 2), "line 5998\n");
    }
}
