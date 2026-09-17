// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Browser descriptions and search keywords.
//!
//! These are not in `bitwig.jar`. Bitwig reads them from properties bundles in
//! the installation's `localization` directory, keyed by the entry's display
//! name. Writing them needs no bytecode work and does not disturb the tamper
//! seal, so a registered entry is searchable from the moment it exists.
//!
//! Bitwig's own lookup returns an empty keyword array for a missing key, which
//! is why an unregistered name is silently unfindable rather than broken.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bitwig_document::descriptions_key;

use crate::{Installation, Kind, Registration, Result, fs};

/// Marks the block this app owns, so its lines can be rewritten without
/// touching Bitwig's own entries.
const BLOCK_START: &str = "#--- orng-registry entries below, managed automatically";

/// Update one kind's bundle so that it describes exactly `registrations`.
///
/// Bitwig's own lines are preserved verbatim. Everything after the marker is
/// replaced, which makes the write idempotent and a removal complete.
pub fn write_bundle(
    install: &Installation,
    kind: Kind,
    registrations: &[&Registration],
) -> Result<PathBuf> {
    let path = install.localization_dir().join(kind.descriptions_bundle());
    let existing = fs::read_to_string_if_exists(&path)?.unwrap_or_default();

    let mut out = existing
        .split_once(BLOCK_START)
        .map_or(existing.as_str(), |(head, _)| head)
        .trim_end()
        .to_owned();

    if !registrations.is_empty() {
        out.push_str("\n\n");
        out.push_str(BLOCK_START);
        out.push('\n');
        for line in bundle_lines(kind, registrations) {
            out.push_str(&line);
            out.push('\n');
        }
    } else {
        out.push('\n');
    }

    fs::write(&path, out)?;
    Ok(path)
}

/// The `key=value` lines describing `registrations`, sorted for a stable file.
fn bundle_lines(kind: Kind, registrations: &[&Registration]) -> Vec<String> {
    let mut pairs = BTreeMap::new();
    for entry in registrations {
        pairs.insert(
            descriptions_key(kind, &entry.name, "desc"),
            escape(&entry.description),
        );
        pairs.insert(
            descriptions_key(kind, &entry.name, "keywords"),
            escape(&entry.keywords.join(" ")),
        );
    }
    pairs.into_iter().map(|(k, v)| format!("{k}={v}")).collect()
}

/// Properties values are single-line; anything else would truncate the entry.
fn escape(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LibraryPath, Provenance};
    use uuid::Uuid;

    fn registration(name: &str) -> Registration {
        Registration {
            uuid: Uuid::new_v4(),
            kind: Kind::Device,
            name: name.into(),
            library_path: LibraryPath::new(format!("devices/My Devices/{name}.bwdevice")).unwrap(),
            description: "Allpass phase-rotator".into(),
            keywords: vec!["disperser".into(), "allpass".into()],
            provenance: Provenance::Local,
        }
    }

    #[test]
    fn writes_keys_in_bitwigs_own_shape() {
        let entry = registration("GLUE COMP");
        let lines = bundle_lines(Kind::Device, &[&entry]);
        assert_eq!(
            lines,
            [
                "device.glue_comp.desc=Allpass phase-rotator",
                "device.glue_comp.keywords=disperser allpass",
            ]
        );
    }

    #[test]
    fn multiline_descriptions_are_flattened() {
        let mut entry = registration("A");
        entry.description = "first\nsecond".into();
        let lines = bundle_lines(Kind::Device, &[&entry]);
        assert!(lines[0].ends_with("first second"), "{lines:?}");
    }
}
