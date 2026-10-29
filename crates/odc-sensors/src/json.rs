//! A strict reader and a writer for the small JSON files OpenDeepCool keeps, such as
//! `gpu-rank.json` (RFC 8259). A key given twice, text after the value or nesting deeper than a
//! few levels refuses the whole file, so a forged or broken file is never half-read (AGENTS.md
//! rule 5). The end-to-end checks read `sensors -j` with it too: a number keeps its text, so no
//! reading passes through floating point.

/// A JSON value. A number keeps its text; an object keeps its members in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

const MAX_DEPTH: usize = 16;

/// The one JSON value of `text`.
pub fn parse(text: &str) -> Result<Json, String> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = parser.value(0)?;
    parser.skip_space();
    if parser.at == parser.bytes.len() {
        Ok(value)
    } else {
        Err(parser.error("text after the value"))
    }
}

/// `text` as a JSON string: quotes, backslashes and control characters escaped.
pub(crate) fn quote(text: &str) -> String {
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        format!("{what} at byte {}", self.at)
    }

    fn skip_space(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.bytes.get(self.at) == Some(&byte);
        if found {
            self.at += 1;
        }
        found
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.eat(byte) {
            Ok(())
        } else {
            Err(self.error(&format!("no {:?}", char::from(byte))))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.error("nesting too deep"));
        }
        self.skip_space();
        match self.bytes.get(self.at) {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("an unexpected character")),
            None => Err(self.error("the end of the text")),
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.error("an unknown word"))
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut members: Vec<(String, Json)> = Vec::new();
        self.skip_space();
        if self.eat(b'}') {
            return Ok(Json::Object(members));
        }
        loop {
            self.skip_space();
            let key = self.string()?;
            if members.iter().any(|(k, _)| *k == key) {
                return Err(self.error(&format!("the key {key:?} twice")));
            }
            self.skip_space();
            self.expect(b':')?;
            let value = self.value(depth + 1)?;
            members.push((key, value));
            self.skip_space();
            if self.eat(b'}') {
                return Ok(Json::Object(members));
            }
            self.expect(b',')?;
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_space();
        if self.eat(b']') {
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_space();
            if self.eat(b']') {
                return Ok(Json::Array(items));
            }
            self.expect(b',')?;
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while matches!(self.bytes.get(self.at), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        self.at - start
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        self.eat(b'-');
        let first = self.at;
        match self.digits() {
            0 => return Err(self.error("a number without digits")),
            n if n > 1 && self.bytes[first] == b'0' => {
                return Err(self.error("a number with a leading zero"));
            }
            _ => {}
        }
        if self.eat(b'.') && self.digits() == 0 {
            return Err(self.error("a fraction without digits"));
        }
        if self.eat(b'e') || self.eat(b'E') {
            let _ = self.eat(b'+') || self.eat(b'-');
            if self.digits() == 0 {
                return Err(self.error("an exponent without digits"));
            }
        }
        let text = String::from_utf8_lossy(&self.bytes[start..self.at]).into_owned();
        Ok(Json::Number(text))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.at;
            while let Some(&b) = self.bytes.get(self.at) {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.at += 1;
            }
            // The text is UTF-8 and the run stops only at ASCII bytes, so it's whole characters.
            let run = std::str::from_utf8(&self.bytes[start..self.at])
                .map_err(|_| self.error("text that isn't UTF-8"))?;
            out.push_str(run);
            match self.bytes.get(self.at) {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    out.push(self.escape()?);
                }
                Some(_) => return Err(self.error("a control character in a string")),
                None => return Err(self.error("an unfinished string")),
            }
        }
    }

    fn escape(&mut self) -> Result<char, String> {
        let Some(&b) = self.bytes.get(self.at) else {
            return Err(self.error("an unfinished escape"));
        };
        self.at += 1;
        let c = match b {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => {
                let first = self.hex4()?;
                let code = if (0xD800..0xDC00).contains(&first) {
                    // A high surrogate needs a low one after it.
                    if !(self.eat(b'\\') && self.eat(b'u')) {
                        return Err(self.error("a lone surrogate"));
                    }
                    let second = self.hex4()?;
                    if !(0xDC00..0xE000).contains(&second) {
                        return Err(self.error("a lone surrogate"));
                    }
                    0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                } else {
                    first
                };
                char::from_u32(code).ok_or_else(|| self.error("an invalid \\u escape"))?
            }
            _ => return Err(self.error("an unknown escape")),
        };
        Ok(c)
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .filter(|d| d.iter().all(u8::is_ascii_hexdigit))
            .ok_or_else(|| self.error("a malformed \\u escape"))?;
        let text = std::str::from_utf8(digits).map_err(|_| self.error("a malformed \\u escape"))?;
        let n = u32::from_str_radix(text, 16).map_err(|_| self.error("a malformed \\u escape"))?;
        self.at += 4;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Json {
        Json::String(text.to_owned())
    }

    #[test]
    fn values_read_as_written() {
        assert_eq!(
            parse(r#" {"a": [1, -2.5e3, true, false, null], "b": "x"} "#),
            Ok(Json::Object(vec![
                (
                    "a".into(),
                    Json::Array(vec![
                        Json::Number("1".into()),
                        Json::Number("-2.5e3".into()),
                        Json::Bool(true),
                        Json::Bool(false),
                        Json::Null,
                    ])
                ),
                ("b".into(), s("x")),
            ]))
        );
        assert_eq!(parse("[]"), Ok(Json::Array(Vec::new())));
        assert_eq!(parse("{}"), Ok(Json::Object(Vec::new())));
    }

    #[test]
    fn escapes_read_back_and_quote_writes_what_parse_reads() {
        assert_eq!(
            parse(r#""a\"b\\c\/d\n\t\u00e9\ud83d\ude00""#),
            Ok(s("a\"b\\c/d\n\té😀"))
        );
        for text in [
            "plain",
            "quote \" and \\",
            "line\nbreak\r\t",
            "\u{1}\u{1f}",
            "é 😀 ü",
        ] {
            assert_eq!(parse(&quote(text)), Ok(s(text)), "{text:?}");
        }
    }

    #[test]
    fn broken_or_forged_text_is_refused() {
        let nested = format!("{}{}", "[".repeat(40), "]".repeat(40));
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\":1,}",
            "{\"a\":1 \"b\":2}",
            "{\"a\":1,\"a\":2}",
            "{\"a\":1} x",
            "\"unfinished",
            "\"tab\tinside\"",
            "\"\\x\"",
            "\"\\u12\"",
            "\"\\ud800\"",
            "\"\\udc00\"",
            "01",
            "-",
            "1.",
            "1e",
            "tru",
            "'single'",
            nested.as_str(),
        ] {
            assert!(parse(bad).is_err(), "{bad:?} should be refused");
        }
    }
}
