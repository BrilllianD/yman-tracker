//! A hand-rolled JSON writer.
//!
//! There is no `serde_json` in the dependency list and no reason to add one:
//! yman emits a handful of fixed shapes and never parses JSON back. Values are
//! rendered as strings and assembled in the order the caller writes them, so
//! the key order in the source is the key order on the wire.

/// Escapes a string and wraps it in quotes.
pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `null` when there is nothing, a quoted string otherwise.
pub fn opt_string(v: Option<&str>) -> String {
    match v {
        Some(s) => string(s),
        None => "null".to_string(),
    }
}

/// An array of strings, each escaped.
pub fn strings(items: &[String]) -> String {
    array(items.iter().map(|s| string(s)))
}

/// An array of values that are already rendered.
pub fn array(items: impl IntoIterator<Item = String>) -> String {
    let mut out = String::from("[");
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&item);
    }
    out.push(']');
    out
}

/// An object built key by key. Insertion order is emission order; nothing is
/// sorted and nothing is deduplicated.
pub struct Object {
    buf: String,
}

impl Object {
    pub fn new() -> Object {
        Object {
            buf: String::from("{"),
        }
    }

    /// A string value.
    pub fn str(self, key: &str, value: &str) -> Object {
        self.raw(key, string(value))
    }

    /// A string value that may be absent, emitted as `null`.
    pub fn opt(self, key: &str, value: Option<&str>) -> Object {
        self.raw(key, opt_string(value))
    }

    /// A value the caller has already rendered: a number, a bool, an array or
    /// a nested object.
    pub fn raw(mut self, key: &str, value: impl AsRef<str>) -> Object {
        if self.buf.len() > 1 {
            self.buf.push(',');
        }
        self.buf.push_str(&string(key));
        self.buf.push(':');
        self.buf.push_str(value.as_ref());
        self
    }

    pub fn finish(mut self) -> String {
        self.buf.push('}');
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_the_characters_json_requires() {
        assert_eq!(string("a\"b"), r#""a\"b""#);
        assert_eq!(string("a\\b"), r#""a\\b""#);
        assert_eq!(string("a\nb"), r#""a\nb""#);
        assert_eq!(string("a\rb"), r#""a\rb""#);
        assert_eq!(string("a\tb"), r#""a\tb""#);
        assert_eq!(string("a\u{1}b"), "\"a\\u0001b\"");
    }

    #[test]
    fn non_ascii_passes_through_unescaped() {
        assert_eq!(string("почини вход"), "\"почини вход\"");
        assert_eq!(string("naïve — café"), "\"naïve — café\"");
    }

    #[test]
    fn empty_collections_are_empty_arrays() {
        assert_eq!(strings(&[]), "[]");
        assert_eq!(array(Vec::new()), "[]");
        assert_eq!(Object::new().finish(), "{}");
    }

    #[test]
    fn arrays_join_with_commas() {
        assert_eq!(strings(&["a".into(), "b".into()]), r#"["a","b"]"#);
        assert_eq!(array(["1".to_string(), "2".to_string()]), "[1,2]");
    }

    #[test]
    fn keys_keep_insertion_order() {
        let out = Object::new()
            .str("id", "5.1")
            .raw("priority", "5")
            .opt("assignee", None)
            .opt("author", Some("Ivan"))
            .raw("tags", strings(&["ui".into()]))
            .finish();
        assert_eq!(
            out,
            r#"{"id":"5.1","priority":5,"assignee":null,"author":"Ivan","tags":["ui"]}"#
        );
    }
}
