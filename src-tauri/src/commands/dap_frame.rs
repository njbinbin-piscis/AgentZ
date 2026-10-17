//! Bounded incremental DAP framing; protocol errors close the owned transport.
const MAX_HEADER: usize = 16 * 1024;
const MAX_BODY: usize = 1024 * 1024;
const MAX_CHUNK: usize = 8192;

#[derive(Default)]
pub(super) struct FrameParser {
    buffer: Vec<u8>,
}
impl FrameParser {
    pub(super) fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>, String> {
        if chunk.len() > MAX_CHUNK
            || self
                .buffer
                .len()
                .checked_add(chunk.len())
                .map_or(true, |n| n > MAX_HEADER + MAX_BODY + MAX_CHUNK)
        {
            return Err("DAP frame buffer limit exceeded".into());
        }
        self.buffer.extend_from_slice(chunk);
        let mut messages = Vec::new();
        loop {
            let Some(end) = self.buffer.windows(4).position(|w| w == b"\r\n\r\n") else {
                if self.buffer.len() > MAX_HEADER {
                    return Err("DAP header limit exceeded".into());
                }
                return Ok(messages);
            };
            if end > MAX_HEADER {
                return Err("DAP header limit exceeded".into());
            }
            let header =
                std::str::from_utf8(&self.buffer[..end]).map_err(|_| "DAP header is not UTF-8")?;
            let mut length = None;
            for line in header.split("\r\n") {
                let (key, value) = line.split_once(':').ok_or("Malformed DAP header")?;
                if key.eq_ignore_ascii_case("Content-Length") {
                    if length.is_some() {
                        return Err("Duplicate DAP Content-Length".into());
                    }
                    let value = value.trim();
                    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                        return Err("Invalid DAP Content-Length".into());
                    }
                    let parsed = value
                        .parse::<usize>()
                        .map_err(|_| "Invalid DAP Content-Length")?;
                    if parsed == 0 || parsed > MAX_BODY {
                        return Err("DAP body limit exceeded".into());
                    }
                    length = Some(parsed);
                }
            }
            let length = length.ok_or("Missing DAP Content-Length")?;
            let start = end.checked_add(4).ok_or("DAP frame length overflow")?;
            let finish = start
                .checked_add(length)
                .ok_or("DAP frame length overflow")?;
            if self.buffer.len() < finish {
                return Ok(messages);
            }
            let text = std::str::from_utf8(&self.buffer[start..finish])
                .map_err(|_| "DAP body is not UTF-8")?
                .to_owned();
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|_| "DAP body is not JSON")?;
            if !value.is_object() {
                return Err("DAP body must be an object".into());
            }
            self.buffer.drain(..finish);
            messages.push(text);
        }
    }
    pub(super) fn finish(&self) -> Result<(), String> {
        if self.buffer.is_empty() {
            Ok(())
        } else {
            Err("DAP stream ended with an incomplete frame".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_and_multiple_frames() {
        let mut p = FrameParser::default();
        assert!(p.push(b"Content-Len").unwrap().is_empty());
        assert_eq!(
            p.push(b"gth: 2\r\n\r\n{}Content-Length: 2\r\n\r\n{}")
                .unwrap(),
            ["{}", "{}"]
        );
        assert!(p.finish().is_ok());
    }
    #[test]
    fn rejects_invalid_lengths_and_bodies() {
        for header in [
            "Content-Length: -1",
            "Content-Length: 0",
            "Content-Length: 1048577",
            "Content-Length: 18446744073709551616",
            "Other: 2",
            "Content-Length: 2\r\nContent-Length: 2",
            "broken",
        ] {
            assert!(
                FrameParser::default()
                    .push(format!("{header}\r\n\r\n").as_bytes())
                    .is_err(),
                "{header}"
            );
        }
        assert!(FrameParser::default()
            .push(b"Content-Length: 2\r\n\r\n[]")
            .is_err());
        assert!(FrameParser::default()
            .push(b"Content-Length: 2\r\n\r\nxx")
            .is_err());
    }
    #[test]
    fn rejects_unterminated_header_and_truncated_body() {
        let mut p = FrameParser::default();
        for _ in 0..2 {
            assert!(p.push(&[b'x'; MAX_CHUNK]).is_ok());
        }
        assert!(p.push(b"x").is_err());
        let mut p = FrameParser::default();
        p.push(b"Content-Length: 20\r\n\r\n{}").unwrap();
        assert!(p.finish().is_err());
    }
}
