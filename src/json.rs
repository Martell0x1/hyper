//! JSON support for the `json` builtin module: `loads`, `dumps`, `load`, `dump`.
//!
//! Parsing walks the source bytes once and builds Hyper values directly, so no
//! intermediate document tree is allocated. Object keys are emitted in sorted
//! order because Hyper dicts are hash maps and would otherwise print differently
//! on every run.

use std::collections::HashMap;

use crate::environment::HyperValue;

pub fn parse(source: &str) -> Result<HyperValue, String> {
    let mut parser = Parser {
        bytes: source.as_bytes(),
        pos: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.pos < parser.bytes.len() {
        return Err(format!(
            "unexpected trailing content at byte {}",
            parser.pos
        ));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "expected '{}' at byte {}",
                byte as char, self.pos
            ))
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(())
        } else {
            Err(format!("invalid literal at byte {}", self.pos))
        }
    }

    fn parse_value(&mut self) -> Result<HyperValue, String> {
        match self.peek() {
            None => Err("unexpected end of input".to_string()),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(HyperValue::String),
            Some(b't') => {
                self.literal("true")?;
                Ok(HyperValue::Boolean(true))
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(HyperValue::Boolean(false))
            }
            Some(b'n') => {
                self.literal("null")?;
                Ok(HyperValue::None)
            }
            Some(_) => self.parse_number(),
        }
    }

    fn parse_object(&mut self) -> Result<HyperValue, String> {
        self.expect(b'{')?;
        let mut entries = HashMap::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(new_dict(entries));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.parse_value()?;
            entries.insert(key, value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(new_dict(entries));
                }
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.pos)),
            }
        }
    }

    fn parse_array(&mut self) -> Result<HyperValue, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(HyperValue::List(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.parse_value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(HyperValue::List(items));
                }
                _ => return Err(format!("expected ',' or ']' at byte {}", self.pos)),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let start = self.pos;
        // Fast path: copy the whole run when there is nothing to unescape.
        while let Some(b) = self.peek() {
            match b {
                b'"' => {
                    let raw = &self.bytes[start..self.pos];
                    self.pos += 1;
                    return String::from_utf8(raw.to_vec())
                        .map_err(|_| "invalid UTF-8 in string".to_string());
                }
                b'\\' => break,
                _ => self.pos += 1,
            }
        }

        let mut out = String::from_utf8(self.bytes[start..self.pos].to_vec())
            .map_err(|_| "invalid UTF-8 in string".to_string())?;
        loop {
            match self.peek() {
                None => return Err("unterminated string".to_string()),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let escape = self.peek().ok_or("unterminated escape".to_string())?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.parse_unicode_escape()?),
                        other => {
                            return Err(format!("invalid escape '\\{}'", other as char));
                        }
                    }
                }
                Some(_) => {
                    let chunk_start = self.pos;
                    while let Some(b) = self.peek() {
                        if b == b'"' || b == b'\\' {
                            break;
                        }
                        self.pos += 1;
                    }
                    let chunk = std::str::from_utf8(&self.bytes[chunk_start..self.pos])
                        .map_err(|_| "invalid UTF-8 in string".to_string())?;
                    out.push_str(chunk);
                }
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let high = self.parse_hex4()?;
        // Surrogate pair: the low half arrives as a second \u escape.
        if (0xD800..0xDC00).contains(&high) {
            if self.peek() == Some(b'\\') && self.bytes.get(self.pos + 1) == Some(&b'u') {
                self.pos += 2;
                let low = self.parse_hex4()?;
                if (0xDC00..0xE000).contains(&low) {
                    let combined =
                        0x10000 + (((high - 0xD800) as u32) << 10) + (low - 0xDC00) as u32;
                    return char::from_u32(combined)
                        .ok_or_else(|| "invalid surrogate pair".to_string());
                }
                return Err("invalid low surrogate".to_string());
            }
            return Err("lone high surrogate".to_string());
        }
        char::from_u32(high as u32).ok_or_else(|| "invalid unicode escape".to_string())
    }

    fn parse_hex4(&mut self) -> Result<u16, String> {
        if self.pos + 4 > self.bytes.len() {
            return Err("truncated unicode escape".to_string());
        }
        let digits = std::str::from_utf8(&self.bytes[self.pos..self.pos + 4])
            .map_err(|_| "invalid unicode escape".to_string())?;
        let value =
            u16::from_str_radix(digits, 16).map_err(|_| "invalid unicode escape".to_string())?;
        self.pos += 4;
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<HyperValue, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') || self.peek() == Some(b'+') {
            self.pos += 1;
        }
        let mut is_float = false;
        while let Some(b) = self.peek() {
            match b {
                b'0'..=b'9' => self.pos += 1,
                b'.' | b'e' | b'E' => {
                    is_float = true;
                    self.pos += 1;
                }
                b'-' | b'+' => self.pos += 1,
                _ => break,
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "invalid number".to_string())?;
        if text.is_empty() {
            return Err(format!("expected a value at byte {}", start));
        }
        if !is_float {
            if let Ok(n) = text.parse::<i64>() {
                return Ok(HyperValue::I64(n));
            }
        }
        text.parse::<f64>()
            .map(HyperValue::F64)
            .map_err(|_| format!("invalid number '{}'", text))
    }
}

fn new_dict(entries: HashMap<String, HyperValue>) -> HyperValue {
    HyperValue::Dict {
        key_type: "string".to_string(),
        val_type: "any".to_string(),
        entries,
    }
}

/// Serialize a Hyper value. `indent` of 0 keeps the output on one line.
pub fn stringify(value: &HyperValue, indent: usize) -> Result<String, String> {
    let mut out = String::new();
    write_value(value, indent, 0, &mut out)?;
    Ok(out)
}

fn write_value(
    value: &HyperValue,
    indent: usize,
    depth: usize,
    out: &mut String,
) -> Result<(), String> {
    match value {
        HyperValue::None => out.push_str("null"),
        HyperValue::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        HyperValue::String(s) => write_string(s, out),
        HyperValue::F32(n) => write_float(*n as f64, out)?,
        HyperValue::F64(n) => write_float(*n, out)?,
        HyperValue::List(items) => write_array(items, indent, depth, out)?,
        HyperValue::Array { elements, .. } => write_array(elements, indent, depth, out)?,
        HyperValue::Dict { entries, .. } => {
            let mut keys: Vec<&String> = entries.keys().collect();
            keys.sort();
            let pairs: Vec<(&str, &HyperValue)> = keys
                .into_iter()
                .map(|k| (k.as_str(), entries.get(k).unwrap()))
                .collect();
            write_object(&pairs, indent, depth, out)?;
        }
        HyperValue::Instance {
            fields,
            field_indices,
            ..
        } => {
            let borrowed = fields.borrow();
            let mut names: Vec<&String> = field_indices.keys().collect();
            names.sort();
            let pairs: Vec<(&str, &HyperValue)> = names
                .into_iter()
                .map(|name| (name.as_str(), &borrowed[field_indices[name]]))
                .collect();
            write_object(&pairs, indent, depth, out)?;
        }
        other => match other.to_int() {
            Some(n) => out.push_str(&n.to_string()),
            None => {
                return Err(format!("cannot serialize {} to JSON", type_label(other)));
            }
        },
    }
    Ok(())
}

fn write_array(
    items: &[HyperValue],
    indent: usize,
    depth: usize,
    out: &mut String,
) -> Result<(), String> {
    if items.is_empty() {
        out.push_str("[]");
        return Ok(());
    }
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_break(indent, depth + 1, out);
        write_value(item, indent, depth + 1, out)?;
    }
    write_break(indent, depth, out);
    out.push(']');
    Ok(())
}

fn write_object(
    pairs: &[(&str, &HyperValue)],
    indent: usize,
    depth: usize,
    out: &mut String,
) -> Result<(), String> {
    if pairs.is_empty() {
        out.push_str("{}");
        return Ok(());
    }
    out.push('{');
    for (i, (key, value)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_break(indent, depth + 1, out);
        write_string(key, out);
        out.push(':');
        if indent > 0 {
            out.push(' ');
        }
        write_value(value, indent, depth + 1, out)?;
    }
    write_break(indent, depth, out);
    out.push('}');
    Ok(())
}

fn write_break(indent: usize, depth: usize, out: &mut String) {
    if indent == 0 {
        return;
    }
    out.push('\n');
    for _ in 0..indent * depth {
        out.push(' ');
    }
}

fn write_float(value: f64, out: &mut String) -> Result<(), String> {
    if !value.is_finite() {
        return Err("cannot serialize NaN or infinity to JSON".to_string());
    }
    if value == value.trunc() && value.abs() < 1e15 {
        out.push_str(&format!("{:.1}", value));
    } else {
        out.push_str(&value.to_string());
    }
    Ok(())
}

fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn type_label(value: &HyperValue) -> &'static str {
    match value {
        HyperValue::Function { .. } | HyperValue::NativeFunction(_) => "a function",
        HyperValue::Module { .. } => "a module",
        HyperValue::File { .. } => "a file",
        HyperValue::MmapFile { .. } => "a mapped file",
        HyperValue::StructDef { .. } => "a struct definition",
        HyperValue::TraitDef { .. } => "a trait",
        _ => "this value",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict_of(value: &HyperValue) -> &HashMap<String, HyperValue> {
        match value {
            HyperValue::Dict { entries, .. } => entries,
            other => panic!("expected a dict, got {:?}", other),
        }
    }

    #[test]
    fn parses_scalars() {
        assert_eq!(parse("null").unwrap(), HyperValue::None);
        assert_eq!(parse("true").unwrap(), HyperValue::Boolean(true));
        assert_eq!(parse("-12").unwrap(), HyperValue::I64(-12));
        assert_eq!(parse("2.5").unwrap(), HyperValue::F64(2.5));
        assert_eq!(
            parse("\"hi\"").unwrap(),
            HyperValue::String("hi".to_string())
        );
    }

    #[test]
    fn parses_nested_document() {
        let value = parse("{\"name\": \"Hyper\", \"tags\": [1, 2], \"ok\": true}").unwrap();
        let entries = dict_of(&value);
        assert_eq!(entries["name"], HyperValue::String("Hyper".to_string()));
        assert_eq!(entries["ok"], HyperValue::Boolean(true));
        assert_eq!(
            entries["tags"],
            HyperValue::List(vec![HyperValue::I64(1), HyperValue::I64(2)])
        );
    }

    #[test]
    fn parses_escapes_and_unicode() {
        let value = parse("\"a\\nb\\u0041\\ud83d\\ude00\"").unwrap();
        assert_eq!(value, HyperValue::String("a\nbA\u{1F600}".to_string()));
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(parse("{\"a\": }").is_err());
        assert!(parse("[1, 2").is_err());
        assert!(parse("nul").is_err());
        assert!(parse("1 2").is_err());
    }

    #[test]
    fn stringify_is_deterministic() {
        let value = parse("{\"b\": 1, \"a\": [true, null]}").unwrap();
        assert_eq!(
            stringify(&value, 0).unwrap(),
            "{\"a\":[true,null],\"b\":1}"
        );
    }

    #[test]
    fn stringify_indents_when_asked() {
        let value = parse("{\"a\": [1]}").unwrap();
        assert_eq!(
            stringify(&value, 2).unwrap(),
            "{\n  \"a\": [\n    1\n  ]\n}"
        );
    }

    #[test]
    fn floats_keep_a_decimal_point() {
        assert_eq!(stringify(&HyperValue::F64(1.0), 0).unwrap(), "1.0");
        assert_eq!(stringify(&HyperValue::F64(0.25), 0).unwrap(), "0.25");
    }

    #[test]
    fn round_trip_preserves_document() {
        let source = "{\"a\":[1,2.5,\"x\"],\"b\":{\"c\":null}}";
        let parsed = parse(source).unwrap();
        assert_eq!(stringify(&parsed, 0).unwrap(), source);
    }

    #[test]
    fn functions_cannot_be_serialized() {
        let value = HyperValue::NativeFunction("open".to_string());
        assert!(stringify(&value, 0).is_err());
    }
}
