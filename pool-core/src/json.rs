//! A small, exact JSON reader and writer.
//!
//! The engine deliberately does not depend on a third-party JSON crate: the
//! floating point parsers of the common crates are not always correctly rounded
//! (they build the value from a truncated significand and a power of ten, which can
//! be one unit in the last place off), and a physics engine that promises
//! bit-for-bit reproducibility cannot afford that.
//!
//! This module parses numbers with the standard library's correctly rounded
//! `str::parse::<f64>` and writes them with the standard library's shortest
//! round-tripping formatter, so a value survives a write/read cycle exactly.

use core::fmt;

/// A JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    /// An object; the entries keep their order so that a write/read cycle is
    /// byte-identical.
    Object(Vec<(String, Value)>),
}

/// A JSON parse error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    /// What went wrong.
    pub message: String,
    /// The byte offset at which it went wrong.
    pub offset: usize,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at byte {})", self.message, self.offset)
    }
}

impl Value {
    /// The value as a number, if it is one.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The value as a string, if it is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The value as a boolean, if it is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The value as an array, if it is one.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items.as_slice()),
            _ => None,
        }
    }

    /// The value as an object, if it is one.
    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(entries) => Some(entries.as_slice()),
            _ => None,
        }
    }

    /// Looks a key up in an object.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// Looks a key up in an object, or fails with a message.
    pub fn field(&self, key: &str) -> Result<&Value, String> {
        self.get(key)
            .ok_or_else(|| format!("the JSON object has no field '{key}'"))
    }

    /// Writes the value as JSON.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(true) => out.push_str("true"),
            Value::Bool(false) => out.push_str("false"),
            Value::Number(n) => {
                if n.is_finite() {
                    // `format!` writes the shortest representation that round-trips.
                    out.push_str(&format!("{n}"));
                } else {
                    out.push_str("null");
                }
            }
            Value::String(s) => write_string(s, out),
            Value::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Value::Object(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(key, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parses a JSON document.
pub fn parse(text: &str) -> Result<Value, JsonError> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        position: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.position != parser.bytes.len() {
        return Err(parser.error("trailing characters after the JSON value"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Parser<'a> {
    fn error(&self, message: &str) -> JsonError {
        JsonError {
            message: message.to_string(),
            offset: self.position,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
                self.position += 1;
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), JsonError> {
        if self.peek() == Some(c) {
            self.position += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", c as char)))
        }
    }

    fn parse_value(&mut self) -> Result<Value, JsonError> {
        match self.peek() {
            None => Err(self.error("unexpected end of input")),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Value::String(self.parse_string()?)),
            Some(b't') => {
                self.parse_literal("true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.parse_literal("false")?;
                Ok(Value::Bool(false))
            }
            Some(b'n') => {
                self.parse_literal("null")?;
                Ok(Value::Null)
            }
            Some(_) => self.parse_number(),
        }
    }

    fn parse_literal(&mut self, literal: &str) -> Result<(), JsonError> {
        if self.bytes[self.position..].starts_with(literal.as_bytes()) {
            self.position += literal.len();
            Ok(())
        } else {
            Err(self.error(&format!("expected '{literal}'")))
        }
    }

    fn parse_object(&mut self) -> Result<Value, JsonError> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(Value::Object(entries));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.parse_value()?;
            entries.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                }
                Some(b'}') => {
                    self.position += 1;
                    break;
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
        Ok(Value::Object(entries))
    }

    fn parse_array(&mut self) -> Result<Value, JsonError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.parse_value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                }
                Some(b']') => {
                    self.position += 1;
                    break;
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
        Ok(Value::Array(items))
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let c = match self.peek() {
                Some(c) => c,
                None => return Err(self.error("unterminated string")),
            };
            self.position += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let escape = match self.peek() {
                        Some(c) => c,
                        None => return Err(self.error("unterminated escape")),
                    };
                    self.position += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let code = self.parse_hex4()?;
                            // A surrogate pair, if there is one.
                            if (0xD800..0xDC00).contains(&code) {
                                if self.peek() == Some(b'\\') {
                                    self.position += 1;
                                    self.expect(b'u')?;
                                    let low = self.parse_hex4()?;
                                    let combined =
                                        0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                                    match char::from_u32(combined) {
                                        Some(ch) => out.push(ch),
                                        None => return Err(self.error("invalid surrogate pair")),
                                    }
                                } else {
                                    return Err(self.error("lone high surrogate"));
                                }
                            } else {
                                match char::from_u32(code) {
                                    Some(ch) => out.push(ch),
                                    None => return Err(self.error("invalid escape")),
                                }
                            }
                        }
                        _ => return Err(self.error("unknown escape")),
                    }
                }
                _ => {
                    // Copy the UTF-8 sequence verbatim.
                    let start = self.position - 1;
                    let len = utf8_len(c);
                    if start + len > self.bytes.len() {
                        return Err(self.error("truncated UTF-8 sequence"));
                    }
                    self.position = start + len;
                    match core::str::from_utf8(&self.bytes[start..self.position]) {
                        Ok(s) => out.push_str(s),
                        Err(_) => return Err(self.error("invalid UTF-8")),
                    }
                }
            }
        }
        Ok(out)
    }

    fn parse_hex4(&mut self) -> Result<u32, JsonError> {
        if self.position + 4 > self.bytes.len() {
            return Err(self.error("truncated unicode escape"));
        }
        let text = core::str::from_utf8(&self.bytes[self.position..self.position + 4])
            .map_err(|_| self.error("invalid unicode escape"))?;
        let value =
            u32::from_str_radix(text, 16).map_err(|_| self.error("invalid unicode escape"))?;
        self.position += 4;
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Value, JsonError> {
        let start = self.position;
        if self.peek() == Some(b'-') || self.peek() == Some(b'+') {
            self.position += 1;
        }
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' | b'.' | b'e' | b'E' => self.position += 1,
                b'+' | b'-' => self.position += 1,
                _ => break,
            }
        }
        let text = core::str::from_utf8(&self.bytes[start..self.position])
            .map_err(|_| self.error("invalid number"))?;
        // The standard library's parser is correctly rounded.
        match text.parse::<f64>() {
            Ok(value) => Ok(Value::Number(value)),
            Err(_) => Err(self.error("invalid number")),
        }
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else if first >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_round_trip_exactly() {
        let value = 1.75_f64 * 0.0254;
        let json = format!("{{\"x\":{value}}}");
        let parsed = parse(&json).expect("parses");
        let back = parsed
            .field("x")
            .expect("the field")
            .as_f64()
            .expect("a number");
        assert_eq!(back.to_bits(), value.to_bits());
    }

    #[test]
    fn the_fixture_numbers_are_read_exactly() {
        let json = "{\"expected\": 0.14562399883500798}";
        let parsed = parse(json).expect("parses");
        let back = parsed
            .field("expected")
            .expect("the field")
            .as_f64()
            .expect("a number");
        assert_eq!(back.to_bits(), 0.14562399883500798_f64.to_bits());
    }

    #[test]
    fn objects_and_arrays_round_trip() {
        let text = r#"{"a":[1,2,{"b":"c\n"}],"d":true,"e":null,"f":-1500}"#;
        let value = parse(text).expect("parses");
        assert_eq!(value.to_json(), text);
        // An exponent is read as the same number.
        let value = parse(r#"{"f":-1.5e3}"#).expect("parses");
        assert_eq!(value.field("f").expect("the field").as_f64(), Some(-1500.0));
    }

    #[test]
    fn a_broken_document_is_reported() {
        assert!(parse("{").is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("nul").is_err());
    }
}
