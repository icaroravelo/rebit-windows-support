//! Strict, bounded sequence compilation on the control thread.
use std::fmt;

pub const MAX_FRAMES: usize = 65_534;
#[derive(Clone, Debug, Default)]
pub struct Sequence {
    pub values: Box<[i16]>,
    pub loop_start: Option<usize>,
    pub release_start: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError(pub String);
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ParseError {}
fn err(message: &str) -> ParseError {
    ParseError(message.into())
}
impl Sequence {
    pub fn parse(text: &str, min: i16, max: i16) -> Result<Self, ParseError> {
        if min > max {
            return Err(err("Invalid value range"));
        }
        if text.len() > MAX_FRAMES * 16 {
            return Err(err("Sequence text is too long"));
        }
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        // Non-ASCII text is rejected by integer parsing.
        let s: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        let mut parts = s.split('|');
        let main = parts.next().unwrap_or("");
        let release = parts.next();
        if parts.next().is_some() {
            return Err(err("Only one release separator is allowed"));
        }
        let mut result = Self::default();
        let mut values = Vec::new();
        if let Some(open) = main.find('[') {
            let close = main
                .find(']')
                .ok_or_else(|| err("Unmatched loop brackets"))?;
            if close <= open
                || close + 1 != main.len()
                || main[open + 1..close].contains(['[', ']'])
            {
                return Err(err(
                    "The loop must be the final section of the main sequence",
                ));
            }
            append(&main[..open], min, max, &mut values)?;
            result.loop_start = Some(values.len());
            append(&main[open + 1..close], min, max, &mut values)?;
            if result.loop_start == Some(values.len()) {
                return Err(err("Loop cannot be empty"));
            }
        } else {
            if main.contains(']') {
                return Err(err("Unmatched loop brackets"));
            }
            append(main, min, max, &mut values)?;
        }
        if values.is_empty() {
            return Err(err("Main sequence cannot be empty"));
        }
        if let Some(tail) = release {
            if tail.is_empty() {
                return Err(err("Release section cannot be empty"));
            }
            result.release_start = Some(values.len());
            append(tail, min, max, &mut values)?;
        }
        result.values = values.into_boxed_slice();
        Ok(result)
    }
}
fn integer(s: &str) -> Result<i32, ParseError> {
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(err("Expected an integer"));
    }
    s.parse().map_err(|_| err("Integer is too large"))
}
fn append(section: &str, min: i16, max: i16, out: &mut Vec<i16>) -> Result<(), ParseError> {
    if section.is_empty() {
        return Ok(());
    }
    for token in section.split(',') {
        let (a, b, count) = if let Some((start, rest)) = token.split_once("to") {
            let (end, frames) = rest
                .split_once("in")
                .ok_or_else(|| err("Use start to end in frames"))?;
            let n = integer(frames.strip_suffix('f').unwrap_or(frames))?;
            if n < 2 {
                return Err(err("A ramp needs at least two frames"));
            }
            (integer(start)?, integer(end)?, n)
        } else if let Some((value, frames)) = token.split_once('x') {
            let n = integer(frames.strip_suffix('f').unwrap_or(frames))?;
            if n < 1 {
                return Err(err("Repetitions must be positive"));
            }
            let a = integer(value)?;
            (a, a, n)
        } else {
            let a = integer(token)?;
            (a, a, 1)
        };
        if a < min as i32 || a > max as i32 || b < min as i32 || b > max as i32 {
            return Err(ParseError(format!(
                "Values must be between {min} and {max}"
            )));
        }
        if count as usize > MAX_FRAMES - out.len() {
            return Err(err("Sequence exceeds 65,534 frames"));
        }
        for i in 0..count {
            let value = if count == 1 {
                a as f64
            } else {
                a as f64 + (b - a) as f64 * i as f64 / (count - 1) as f64
            };
            out.push(value.round() as i16);
        }
    }
    Ok(())
}
#[derive(Clone, Default)]
pub struct SequenceBank(pub [Sequence; 3]);
#[derive(Clone, Copy, Debug, Default)]
pub struct Cursor {
    index: usize,
    released: bool,
    finished: bool,
}
impl Cursor {
    pub fn value(&self, seq: &Sequence) -> Option<i16> {
        if self.finished {
            None
        } else {
            seq.values.get(self.index).copied()
        }
    }
    pub fn position(&self, seq: &Sequence) -> Option<usize> {
        self.value(seq).map(|_| self.index)
    }
    /// Retain main/release progress when a control-thread edit replaces a bank.
    pub fn rebind(&mut self, old: &Sequence, new: &Sequence) {
        if self.released {
            if let Some(start) = new.release_start {
                let offset = old
                    .release_start
                    .map_or(0, |r| self.index.saturating_sub(r));
                self.index = start.saturating_add(offset);
                self.finished = self.finished || self.index >= new.values.len();
            } else {
                self.finished = true;
            }
        } else {
            let end = new.release_start.unwrap_or(new.values.len());
            self.index = self.index.min(end.saturating_sub(1));
            self.finished = false;
        }
    }
    pub fn release(&mut self, seq: &Sequence) {
        if self.released {
            return;
        }
        self.released = true;
        if let Some(start) = seq.release_start {
            self.index = start;
        } else {
            self.finished = true;
        }
    }
    pub fn advance(&mut self, seq: &Sequence) {
        if self.finished || seq.values.is_empty() {
            return;
        }
        let end = if self.released {
            seq.values.len()
        } else {
            seq.release_start.unwrap_or(seq.values.len())
        };
        if self.index + 1 < end {
            self.index += 1;
        } else if self.released {
            self.finished = true;
        } else if let Some(start) = seq.loop_start {
            self.index = start;
        }
    }
}
