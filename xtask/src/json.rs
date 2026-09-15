//! The smallest JSON this package can get away with.
//!
//! It exists for exactly one conversation: the request the describer
//! posts to a hosted vision model, and the answer that comes back. That
//! is a few hundred bytes out and one string in, and the string is buried
//! two levels down a shape somebody else chose — so writing JSON by
//! `format!` and reading it back by `find("\"content\"")` is the version
//! of this that works until a description contains a brace.
//!
//! A hundred and fifty lines rather than a crate, for the reason the
//! SHA-256 and the tar reconstruction beside it are: this package has no
//! dependencies, and the whole of what is wanted here is escaping,
//! unescaping, and walking two keys and an index. What it deliberately
//! does not have is anything about numbers a chat API does not send —
//! there is no big-integer path and no attempt to keep the text of a
//! number, because nothing here reads one back out.

use std::fmt::Write as _;

/// A JSON value, as much of one as an answer from a chat API is.
#[derive(Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    List(Vec<Self>),
    /// Members in the order they were written. A list rather than a map
    /// because the only thing anything here does with an object is ask it
    /// for one key, and duplicate keys — which JSON permits and nobody
    /// sends — then keep the first, which is a decision rather than an
    /// accident.
    Map(Vec<(String, Self)>),
}

impl Json {
    /// One member of an object, or `None` for anything else.
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Map(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// One element of an array, or `None` for anything else.
    pub fn at(&self, index: usize) -> Option<&Self> {
        match self {
            Self::List(items) => items.get(index),
            _ => None,
        }
    }

    /// The string this is, or `None` if it is not one. A number where a
    /// string was wanted is a shape nobody here can act on, so it reads
    /// as an absence rather than as a rendering of the number.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// **A whole JSON document, or a complaint saying where reading stopped.**
///
/// The offset is in the message on purpose: the document this reads is
/// somebody else's error page as often as it is an answer, and "not JSON"
/// with nothing else in it sends a person looking at the wrong half of
/// the pipeline.
pub fn parse(text: &str) -> Result<Json, String> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let value = read(bytes, &mut at)?;
    space(bytes, &mut at);
    if at < bytes.len() {
        return Err(format!("something follows the value, {at} bytes in"));
    }
    Ok(value)
}

fn read(bytes: &[u8], at: &mut usize) -> Result<Json, String> {
    space(bytes, at);
    match bytes.get(*at) {
        None => Err("the text ends where a value should be".to_owned()),
        Some(b'{') => object(bytes, at),
        Some(b'[') => array(bytes, at),
        Some(b'"') => text(bytes, at).map(Json::Text),
        Some(b't') => word(bytes, at, "true", Json::Bool(true)),
        Some(b'f') => word(bytes, at, "false", Json::Bool(false)),
        Some(b'n') => word(bytes, at, "null", Json::Null),
        Some(_) => number(bytes, at),
    }
}

fn object(bytes: &[u8], at: &mut usize) -> Result<Json, String> {
    *at += 1; // the `{`
    let mut members = Vec::new();
    space(bytes, at);
    if bytes.get(*at) == Some(&b'}') {
        *at += 1;
        return Ok(Json::Map(members));
    }
    loop {
        space(bytes, at);
        let key = text(bytes, at)?;
        space(bytes, at);
        if bytes.get(*at) != Some(&b':') {
            return Err(format!("no `:` after `{key}`, {at} bytes in"));
        }
        *at += 1;
        members.push((key, read(bytes, at)?));
        space(bytes, at);
        match bytes.get(*at) {
            Some(b',') => *at += 1,
            Some(b'}') => {
                *at += 1;
                return Ok(Json::Map(members));
            }
            _ => return Err(format!("no `,` or `}}` in the object, {at} bytes in")),
        }
    }
}

fn array(bytes: &[u8], at: &mut usize) -> Result<Json, String> {
    *at += 1; // the `[`
    let mut items = Vec::new();
    space(bytes, at);
    if bytes.get(*at) == Some(&b']') {
        *at += 1;
        return Ok(Json::List(items));
    }
    loop {
        items.push(read(bytes, at)?);
        space(bytes, at);
        match bytes.get(*at) {
            Some(b',') => *at += 1,
            Some(b']') => {
                *at += 1;
                return Ok(Json::List(items));
            }
            _ => return Err(format!("no `,` or `]` in the array, {at} bytes in")),
        }
    }
}

fn word(bytes: &[u8], at: &mut usize, spelling: &str, value: Json) -> Result<Json, String> {
    if bytes[*at..].starts_with(spelling.as_bytes()) {
        *at += spelling.len();
        return Ok(value);
    }
    Err(format!("`{spelling}` is misspelled, {at} bytes in"))
}

fn number(bytes: &[u8], at: &mut usize) -> Result<Json, String> {
    let from = *at;
    while bytes
        .get(*at)
        .is_some_and(|byte| matches!(byte, b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E'))
    {
        *at += 1;
    }
    std::str::from_utf8(&bytes[from..*at])
        .ok()
        .and_then(|text| text.parse().ok())
        .map(Json::Number)
        .ok_or_else(|| format!("not a value, {from} bytes in"))
}

/// One string, with its escapes undone. Everything about JSON that is not
/// obvious is in here, which is why the reader above is as short as it is.
fn text(bytes: &[u8], at: &mut usize) -> Result<String, String> {
    if bytes.get(*at) != Some(&b'"') {
        return Err(format!("no string where one was expected, {at} bytes in"));
    }
    *at += 1;
    let mut out = String::new();
    loop {
        let byte = *bytes
            .get(*at)
            .ok_or_else(|| "a string that never closes".to_owned())?;
        *at += 1;
        match byte {
            b'"' => return Ok(out),
            b'\\' => {
                let escape = *bytes
                    .get(*at)
                    .ok_or_else(|| "an escape that never finishes".to_owned())?;
                *at += 1;
                match escape {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => out.push(escaped_char(bytes, at)?),
                    other => return Err(format!("`\\{}` is not an escape", other as char)),
                }
            }
            // Everything else is UTF-8 already, so the bytes are carried
            // through as they lie and the string is built at the end.
            _ => {
                let from = *at - 1;
                while bytes
                    .get(*at)
                    .is_some_and(|next| !matches!(next, b'"' | b'\\'))
                {
                    *at += 1;
                }
                out.push_str(
                    std::str::from_utf8(&bytes[from..*at])
                        .map_err(|_| format!("a string that is not UTF-8, {from} bytes in"))?,
                );
            }
        }
    }
}

/// A `\u` escape, and the surrogate pair a character outside the basic
/// plane is written as. An emoji in a description is not hypothetical.
fn escaped_char(bytes: &[u8], at: &mut usize) -> Result<char, String> {
    let first = hex(bytes, at)?;
    if !(0xd800..0xdc00).contains(&first) {
        return char::from_u32(u32::from(first)).ok_or_else(|| "not a character".to_owned());
    }
    if bytes.get(*at) != Some(&b'\\') || bytes.get(*at + 1) != Some(&b'u') {
        return Err("half of a surrogate pair".to_owned());
    }
    *at += 2;
    let second = hex(bytes, at)?;
    let code = 0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00);
    char::from_u32(code).ok_or_else(|| "not a character".to_owned())
}

fn hex(bytes: &[u8], at: &mut usize) -> Result<u16, String> {
    let digits = bytes
        .get(*at..*at + 4)
        .ok_or_else(|| "a `\\u` with fewer than four digits".to_owned())?;
    *at += 4;
    let text = std::str::from_utf8(digits).map_err(|_| "a `\\u` that is not hex".to_owned())?;
    u16::from_str_radix(text, 16).map_err(|_| format!("`{text}` is not four hex digits"))
}

fn space(bytes: &[u8], at: &mut usize) {
    while bytes
        .get(*at)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
    {
        *at += 1;
    }
}

/// One string as JSON writes it, quotes and all.
///
/// Control characters go out as `\u00XX` rather than being dropped,
/// because the thing being escaped here is a prompt containing somebody's
/// file names and a measurement, and a silently-mangled prompt is a
/// description of the wrong asset.
pub fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", other as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The answer this reader exists for is one it can read.** The
    /// shape is somebody else's — a list of choices, each with a message,
    /// each with the content — and the string wanted is two keys and an
    /// index down it, wearing every escape a sentence can carry.
    #[test]
    fn the_shape_a_chat_api_answers_in_is_walked_to_the_sentence() {
        let answer = r#"{
          "id": "gen-1",
          "choices": [
            {"index": 0, "message": {"role": "assistant",
             "content": "A squat crate, 0.7 m across, with \"01\" stencilled on it.\nWorn."}}
          ],
          "usage": {"total_tokens": 91}
        }"#;
        let said = parse(answer)
            .expect("the answer parses")
            .get("choices")
            .and_then(|choices| choices.at(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Json::text)
            .expect("a description in it")
            .to_owned();
        assert_eq!(
            said,
            "A squat crate, 0.7 m across, with \"01\" stencilled on it.\nWorn."
        );
    }

    /// **A refusal is a shape too, and it is the one that arrives when
    /// the key is wrong.** Reading it is how the run says "401" instead
    /// of "no description".
    #[test]
    fn the_shape_a_refusal_arrives_in_is_read_as_well() {
        let refusal = r#"{"error": {"message": "No auth credentials found", "code": 401}}"#;
        let parsed = parse(refusal).expect("a refusal is JSON too");
        assert_eq!(
            parsed
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Json::text),
            Some("No auth credentials found")
        );
        assert_eq!(
            parsed.get("error").and_then(|error| error.get("code")),
            Some(&Json::Number(401.0))
        );
    }

    /// **What goes out comes back the same.** The prompt carries file
    /// names, quotes and newlines, and every one of them is a character
    /// JSON means something by.
    #[test]
    fn a_prompt_survives_being_quoted_and_read_back() {
        for original in [
            "SM_Prop_Crate_01",
            "a \"quoted\" name",
            "two\nlines\tand a tab",
            "a backslash \\ and a slash /",
            "an emoji \u{1f6f0} and an accent é",
            "\u{1}a control character",
        ] {
            let round = parse(&quoted(original)).expect("its own quoting");
            assert_eq!(round.text(), Some(original), "`{original}` did not survive");
        }
    }

    /// **A surrogate pair is one character.** Models answer with emoji,
    /// and a half-read pair is a panic or a replacement character in the
    /// middle of somebody's catalogue.
    #[test]
    fn an_escaped_pair_outside_the_basic_plane_reads_as_one_character() {
        assert_eq!(
            parse("\"\\ud83d\\ude80 to orbit\"").expect("a pair").text(),
            Some("\u{1f680} to orbit")
        );
    }

    /// **Malformed text is a complaint, not a panic.** What this reads is
    /// whatever a server sent, which on a bad day is an HTML error page.
    #[test]
    fn something_that_is_not_json_is_a_sentence_about_where_it_stopped() {
        for broken in [
            "<html><body>502 Bad Gateway</body></html>",
            "{\"choices\": [",
            "{\"a\" 1}",
            "",
            "{\"a\": \"unclosed}",
        ] {
            let complaint = parse(broken).expect_err("this is not JSON");
            assert!(!complaint.is_empty(), "`{broken}` complained about nothing");
        }
    }
}
