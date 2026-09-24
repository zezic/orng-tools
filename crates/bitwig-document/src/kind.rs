// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;

use crate::{Error, Result};

/// The longest file name all three platforms take, in the bytes macOS and
/// Linux count. Windows counts UTF-16 units, of which there are never more.
const MAX_FILE_NAME: usize = 255;

/// The three kinds of content Bitwig keeps identities for.
///
/// Fixed by Bitwig: its own category enum has exactly these three constants,
/// and their names are not obfuscated. A document's kind is a property of the
/// document, never a user choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Device,
    Modulator,
    Module,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Device, Kind::Modulator, Kind::Module];

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "bwdevice" => Some(Kind::Device),
            "bwmodulator" => Some(Kind::Modulator),
            "bwmodule" => Some(Kind::Module),
            _ => None,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
    }

    pub fn extension(self) -> &'static str {
        match self {
            Kind::Device => "bwdevice",
            Kind::Modulator => "bwmodulator",
            Kind::Module => "bwmodule",
        }
    }

    /// The file a document of this kind called `name` is kept in:
    /// `<name>.<extension>`.
    ///
    /// Refused where the name cannot be a file on one of the three platforms,
    /// which is a statement about the name rather than about the document: a
    /// separator, a character Windows refuses, a name Windows reserves for a
    /// device whatever the extension after it, or more than 255 bytes.
    pub fn file_name(self, name: &str) -> Result<String> {
        const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
        let upper = name.to_ascii_uppercase();
        let numbered = ["COM", "LPT"].iter().any(|port| {
            upper.strip_prefix(port).is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
        });
        let file_name = format!("{name}.{}", self.extension());
        let placeable = !name.is_empty()
            && !name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
            && !RESERVED.contains(&upper.as_str())
            && !numbered
            && file_name.len() <= MAX_FILE_NAME;
        placeable.then_some(file_name).ok_or_else(|| Error::UnplaceableName(name.to_owned()))
    }

    /// Constant name in Bitwig's category enum. Not obfuscated, so this is a
    /// reliable anchor when resolving the enum's fields.
    pub fn enum_constant(self) -> &'static str {
        match self {
            Kind::Device => "DEVICE",
            Kind::Modulator => "MODULATOR",
            Kind::Module => "MODULE",
        }
    }

    /// Subdirectory of the installation's `Library` that registry paths of this
    /// kind resolve against.
    pub fn library_subdir(self) -> &'static str {
        match self {
            Kind::Device => "devices",
            Kind::Modulator => "modulators",
            Kind::Module => "modules",
        }
    }

    /// Folder in the user library that Bitwig's own "Save..." writes into, and
    /// that the installation is linked to.
    pub fn user_folder(self) -> &'static str {
        match self {
            Kind::Device => "My Devices",
            Kind::Modulator => "My Modulators",
            Kind::Module => "My Modules",
        }
    }

    /// Properties file under the installation's `localization` directory that
    /// holds browser descriptions and search keywords for this kind.
    pub fn descriptions_bundle(self) -> &'static str {
        match self {
            Kind::Device => "Device-descriptions-resources.properties",
            Kind::Modulator => "Modulator-descriptions-resources.properties",
            Kind::Module => "Module-descriptions-resources.properties",
        }
    }

    /// Key prefix within [`Self::descriptions_bundle`]. Full key is
    /// `<prefix>.<name lowercased, spaces to underscores>.<desc|keywords>`.
    pub fn descriptions_key_prefix(self) -> &'static str {
        match self {
            Kind::Device => "device",
            Kind::Modulator => "modulator",
            Kind::Module => "module",
        }
    }

    /// Human label for UI copy.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Device => "Device",
            Kind::Modulator => "Modulator",
            Kind::Module => "Grid module",
        }
    }
}

/// Build the descriptions key for a display name, matching Bitwig's own
/// derivation: spaces to underscores, lowercased.
pub fn descriptions_key(kind: Kind, display_name: &str, suffix: &str) -> String {
    format!(
        "{}.{}.{}",
        kind.descriptions_key_prefix(),
        display_name.replace(' ', "_").to_lowercase(),
        suffix
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_matches_bitwigs_derivation() {
        // Bitwig ships `device.polysynth.keywords`; the Grid module bundle uses
        // `module.gate_in.desc`.
        assert_eq!(descriptions_key(Kind::Device, "Polysynth", "keywords"), "device.polysynth.keywords");
        assert_eq!(descriptions_key(Kind::Module, "Gate In", "desc"), "module.gate_in.desc");
    }

    /// A name is refused where some platform would refuse the file, and only
    /// there: a reserved device name is only reserved whole.
    #[test]
    fn a_name_is_a_file_only_where_every_platform_can_hold_it() {
        for name in ["A/B", "A\\B", "WHAT?", "A:B", "CON", "nul", "com1", "LPT9", "", "A\tB"] {
            let refused = Kind::Device.file_name(name);
            assert!(matches!(refused, Err(Error::UnplaceableName(_))), "{name:?}: {refused:?}");
        }
        assert!(Kind::Device.file_name(&"X".repeat(247)).is_err(), "past 255 bytes");
        for name in ["Wait...", "..", "CONSOLE", "COM10", "\u{d8} Bend", &"X".repeat(246)] {
            let file_name = Kind::Device.file_name(name);
            assert!(file_name.is_ok(), "{name:?}: {file_name:?}");
        }
        assert_eq!(Kind::Modulator.file_name("SHAPER").unwrap(), "SHAPER.bwmodulator");
    }

    #[test]
    fn kinds_round_trip_through_extensions() {
        for kind in Kind::ALL {
            assert_eq!(Kind::from_extension(kind.extension()), Some(kind));
        }
    }
}
