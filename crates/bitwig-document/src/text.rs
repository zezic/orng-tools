// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The textual serialization.
//!
//! Bitwig also writes documents as relaxed JSON: unquoted keys at the structural
//! level, quoted keys inside `data`. Identity work needs only the string fields
//! of a section's own object, so this scans for them rather than parsing a
//! tree.

use std::collections::BTreeMap;

/// A quoted string value and the span of its contents, excluding the quotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub value: String,
    pub offset: usize,
    pub len: usize,
}

pub type Fields = BTreeMap<String, Field>;

/// Braces and brackets open around a field of the section's own object: the
/// object's, and then its `data`'s. Anything deeper belongs to an object
/// nested in it, which can carry fields of the same name.
const OWN_FIELD_DEPTH: usize = 2;

/// Collect the `"key" : "value"` pairs of the object `section` starts with.
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
        if key.depth == OWN_FIELD_DEPTH
            && is_pair_separator(&section[key.end + 1..value.start - 1])
        {
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
    /// Braces and brackets open around the run.
    depth: usize,
}

/// The quoted runs of the first object in `data`, up to where it closes.
/// Whatever follows it - padding, or the resources archive after a body - is
/// not the object's.
fn quoted_runs(data: &[u8]) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b'{' | b'[' => depth += 1,
            // The first object closing ends it, and a close with nothing
            // open is past it as well.
            b'}' | b']' if depth <= 1 => break,
            b'}' | b']' => depth -= 1,
            _ => {}
        }
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
        runs.push(Run { start, end: j, depth });
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
        let src = br#"{
        class : "meta",
        data :
        {
            "creator" : "Example Studio",
            "revision_no" : 52795,
            "device_name" : "DISPERSER",
            "has_audio_input" : false
        }
        }"#;
        let fields = scan(src);
        assert_eq!(fields["creator"].value, "Example Studio");
        assert_eq!(fields["device_name"].value, "DISPERSER");
        assert!(!fields.contains_key("revision_no"));
        // The recorded span must address exactly the value's bytes.
        let f = &fields["device_name"];
        assert_eq!(&src[f.offset..f.offset + f.len], b"DISPERSER");
    }

    /// A device inside a container's chain has an identity of its own, under
    /// the same key as the container's.
    #[test]
    fn reads_only_the_objects_own_fields() {
        let src = br#"{
        class : "float_core.device_contents(151)",
        data :
        {
            "device_name(386)" : "OUTER",
            "child_components(173)" :
            [
                {
                    class : "float_core.device_contents(151)",
                    data :
                    {
                        "device_name(386)" : "INNER"
                    }
                }
            ]
        }
        }
        "not_a_field" : "after the object""#;
        let fields = scan(src);
        assert_eq!(fields["device_name(386)"].value, "OUTER");
        assert_eq!(fields.len(), 1);
    }
}
