//! Canonical JSON matching `sign.mjs` (RFC 8785 / RFC 8785-style JCS).
//!
//! `sign.mjs` sorts object keys in UTF-16 code unit order and serializes
//! strings and numbers the way `JSON.stringify` does. Signatures and the
//! hash chain are computed over these bytes, so this encoder has to agree
//! with that script exactly.

use serde_json::Value;
use std::cmp::Ordering;

pub fn canon(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => json_string(s),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(canon).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| cmp_utf16(a, b));
            let parts: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{}:{}", json_string(k), canon(&map[k])))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

fn cmp_utf16(a: &str, b: &str) -> Ordering {
    let au: Vec<u16> = a.encode_utf16().collect();
    let bu: Vec<u16> = b.encode_utf16().collect();
    au.cmp(&bu)
}

/// `JSON.stringify` string encoding, including ES2019 U+2028 / U+2029 escapes.
pub fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorts_keys_and_skips_nothing_null() {
        let v = json!({"b": 1, "a": null, "c": {"z": true, "y": [1, "x"]}});
        assert_eq!(canon(&v), r#"{"a":null,"b":1,"c":{"y":[1,"x"],"z":true}}"#);
    }

    #[test]
    fn chinese_and_controls() {
        let v = json!({"comment": "抢占天元。", "n": "a\nb"});
        assert_eq!(canon(&v), r#"{"comment":"抢占天元。","n":"a\nb"}"#);
    }
}
