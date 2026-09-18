// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3, as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.

//! Reading and preparing Bitwig's internal content registry.
//!
//! Two things live here: resolving where the registry, the entitlement grants
//! and the tamper guard are in a given `bitwig.jar` ([`anchors`]), and reading
//! what is already registered ([`entries`]).
//!
//! Nothing in this crate stores an obfuscated name beyond the life of the jar it
//! was read from.

pub mod anchors;
pub mod entries;
pub mod guard;
mod section_key;

pub use anchors::{Binding, BuildId, EntitlementBinding, RegistryBinding};
pub use bitwig_document::BitwigVersion;
pub use entries::{Entry, read_entries};
pub use guard::GuardState;
pub use section_key::section_key;

/// Something resolution looks for. Naming the failure is the point: a build that
/// changed shape has to be reported, never guessed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    RegistryClass,
    RegisterMethod,
    KindEnum,
    EntitlementClass,
    EntitlementGrants,
    EntitlementRowClass,
    GuardClass,
    GuardSite,
}

impl Anchor {
    /// Plain-language description, for a diagnostics report.
    pub fn describe(self) -> &'static str {
        match self {
            Anchor::RegistryClass => "the class listing every native device",
            Anchor::RegisterMethod => "the registration method",
            Anchor::KindEnum => "the device/modulator/module category enum",
            Anchor::EntitlementClass => "the license entitlement check",
            Anchor::EntitlementGrants => "the entitlement grant maps",
            Anchor::EntitlementRowClass => "the entitlement grant row type",
            Anchor::GuardClass => "the class holding the tamper guard",
            Anchor::GuardSite => "the tamper guard itself",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not locate {} in this build", .0.describe())]
    Unresolved(Anchor),
    #[error("expected one match for {}, found {found}", .anchor.describe())]
    Ambiguous { anchor: Anchor, found: usize },
    #[error(transparent)]
    Classfile(#[from] bitwig_classfile::Error),
    #[error("io error on {path}: {source}")]
    Io { path: String, source: std::io::Error },
    /// Never names a candidate and never carries one. What went wrong is the
    /// count and the reason; the material is not part of either.
    #[error("no section key in this build: {why} ({tried} candidate(s) tried)")]
    NoSectionKey { tried: usize, why: &'static str },
}

pub type Result<T> = std::result::Result<T, Error>;
