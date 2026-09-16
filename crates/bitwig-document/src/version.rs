// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bitwig release numbers.
//!
//! A document is version-sensitive: a release older than the one that wrote
//! it may refuse to load it. So "which Bitwig" is a document concern, and
//! anything describing what can open a document needs to compare releases.

/// A Bitwig release number: `6.1`, `6.0.11`. Not semver -- the component count
/// varies -- so it is kept as components and ordered by them.
///
/// Ordering is the point: content declares a minimum Bitwig version, and asking
/// whether an installation satisfies it must not become string comparison,
/// where `6.10` sorts before `6.9`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BitwigVersion(Vec<u32>);

impl BitwigVersion {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if !text.contains('.') {
            return None;
        }
        text.split('.')
            .map(|part| part.parse().ok())
            .collect::<Option<Vec<u32>>>()
            .filter(|parts| !parts.is_empty())
            .map(BitwigVersion)
    }

    /// Whether this release satisfies a declared minimum.
    ///
    /// Missing components count as zero, so `6.1` satisfies `6.1.0` and not
    /// `6.1.1`.
    pub fn satisfies(&self, minimum: &BitwigVersion) -> bool {
        let width = self.0.len().max(minimum.0.len());
        let pad = |v: &[u32]| -> Vec<u32> {
            v.iter().copied().chain(std::iter::repeat(0)).take(width).collect()
        };
        pad(&self.0) >= pad(&minimum.0)
    }
}

impl std::fmt::Display for BitwigVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self.0.iter().map(u32::to_string).collect();
        f.write_str(&parts.join("."))
    }
}

#[cfg(test)]
mod tests {
    use super::BitwigVersion;

    fn v(text: &str) -> BitwigVersion {
        BitwigVersion::parse(text).unwrap()
    }

    #[test]
    fn orders_by_component_not_by_text() {
        assert!(v("6.10") > v("6.9"));
        assert!(v("6.0.11") > v("6.0.6"));
        assert!(v("6.1") > v("6.0.11"));
    }

    #[test]
    fn missing_components_count_as_zero() {
        assert!(v("6.1").satisfies(&v("6.1.0")));
        assert!(!v("6.1").satisfies(&v("6.1.1")));
        assert!(v("6.1.1").satisfies(&v("6.1")));
    }

    #[test]
    fn rejects_what_is_not_a_release_number() {
        assert!(BitwigVersion::parse("6").is_none());
        assert!(BitwigVersion::parse("6.1-beta").is_none());
        assert!(BitwigVersion::parse("").is_none());
    }
}
