/// Parse raw SSE lines into structured data events.
/// Handles `data:` prefix and `[DONE]` sentinel.
pub struct SseParser;

pub enum SseLine<'a> {
    Data(&'a str),
    Event(&'a str),
    Done,
    Comment,
    Empty,
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SseParser {
    pub fn new() -> Self {
        Self
    }

    /// Feed a raw line from the SSE stream.
    pub fn feed<'a>(&'a mut self, raw_line: &'a str) -> Option<SseLine<'a>> {
        let line = raw_line.trim();

        if line.is_empty() {
            return Some(SseLine::Empty);
        }

        if line.starts_with(':') {
            return Some(SseLine::Comment);
        }

        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                return Some(SseLine::Done);
            }
            return Some(SseLine::Data(data));
        }

        if let Some(event) = line.strip_prefix("event: ") {
            return Some(SseLine::Event(event));
        }

        if line == "[DONE]" {
            return Some(SseLine::Done);
        }

        None
    }
}
