//! The textual serialization.
//!
//! Bitwig also writes documents as relaxed JSON: unquoted keys at the structural
//! level, quoted keys inside `data`. Identity work needs only the string fields
//! of the metadata object, so this scans for them rather than parsing a tree.

use std::collections::BTreeMap;

/// A quoted string value and the span of its contents, excluding the quotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub value: String,
    pub offset: usize,
    pub len: usize,
}

pub type Fields = BTreeMap<String, Field>;

/// Collect every `"key" : "value"` pair in `section`.
///
/// Structural keys in this format are unquoted (`class :`, `data :`) while keys
/// inside `data` are quoted, so a pair cannot be recognised from a key alone.
/// Instead every quoted run is collected and consecutive runs separated by
/// nothing but whitespace and a single colon are taken as a pair.
///
/// Offsets are relative to the start of `section`.
pub fn scan(section: &[u8]) -> Fields {
    let runs = quoted_runs(section);
    let mut fields = Fields::new();
    let mut i = 0;

    while i + 1 < runs.len() {
        let (key, value) = (&runs[i], &runs[i + 1]);
        // `end` is the closing quote and `start - 1` the opening one, so the
        // separator is what lies strictly between the two quoted runs.
        if is_pair_separator(&section[key.end + 1..value.start - 1]) {
            fields.insert(
                decode(section, key),
                Field {
                    value: decode(section, value),
                    offset: value.start,
                    len: value.end - value.start,
                },
            );
            i += 2;
        } else {
            i += 1;
        }
    }
    fields
}

/// Byte span of one quoted run's contents, excluding the quotes.
struct Run {
    start: usize,
    end: usize,
}

fn quoted_runs(data: &[u8]) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] != b'"' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < data.len() && data[j] != b'"' {
            j += if data[j] == b'\\' { 2 } else { 1 };
        }
        if j >= data.len() {
            break;
        }
        runs.push(Run { start, end: j });
        i = j + 1;
    }
    runs
}

fn decode(data: &[u8], run: &Run) -> String {
    String::from_utf8_lossy(&data[run.start..run.end]).into_owned()
}

/// Whitespace around exactly one colon, and nothing else.
fn is_pair_separator(between: &[u8]) -> bool {
    let mut colons = 0;
    for &b in between {
        match b {
            b':' => colons += 1,
            b if b.is_ascii_whitespace() => {}
            _ => return false,
        }
    }
    colons == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_string_fields_and_skips_others() {
        let src = br#"
        class : "meta",
        data :
        {
            "creator" : "Example Studio",
            "revision_no" : 52795,
            "device_name" : "DISPERSER",
            "has_audio_input" : false
        }"#;
        let fields = scan(src);
        assert_eq!(fields["creator"].value, "Example Studio");
        assert_eq!(fields["device_name"].value, "DISPERSER");
        assert!(!fields.contains_key("revision_no"));
        // The recorded span must address exactly the value's bytes.
        let f = &fields["device_name"];
        assert_eq!(&src[f.offset..f.offset + f.len], b"DISPERSER");
    }
}
