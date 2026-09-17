// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};

use crate::Installation;

/// Process names Bitwig runs under. The engine and plugin hosts are separate
/// processes and hold the installation open after the main window closes, so
/// all of them have to be clear before the installation can be modified.
const PROCESS_MARKERS: &[&str] = &[
    "bitwigstudio",
    "bitwig studio",
    "bitwig audio engine",
    "bitwig plug-in host",
    "bitwigplugin",
];

/// Whether anything is currently holding a Bitwig installation open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunState {
    Clear,
    /// Process names found, for reporting which one to quit.
    Running(Vec<String>),
}

impl RunState {
    pub fn is_clear(&self) -> bool {
        matches!(self, RunState::Clear)
    }
}

/// Scan for processes holding `install` open.
///
/// Candidates are found by process name, because the audio engine and the plugin
/// hosts live in nested bundles whose names are all that is stable about them.
/// They are then narrowed to this installation by executable path: a second copy
/// of Bitwig running elsewhere does not stop this one being modified, and on
/// Linux running from a copy the caller never resolved is normal.
///
/// A process whose path cannot be read counts as holding the installation open.
/// Being told to quit a Bitwig that was not in the way costs a moment; modifying
/// an archive under a live JVM costs the session.
pub fn running_state(install: &Installation) -> RunState {
    let refresh = RefreshKind::nothing()
        .with_processes(ProcessRefreshKind::nothing().with_exe(UpdateKind::Always));
    let system = System::new_with_specifics(refresh);

    let mut found: Vec<String> = system
        .processes()
        .values()
        .filter(|process| {
            let name = process.name().to_string_lossy().to_ascii_lowercase();
            PROCESS_MARKERS.iter().any(|marker| name.contains(marker))
        })
        .filter(|process| process.exe().is_none_or(|exe| exe.starts_with(install.root())))
        .map(|process| process.name().to_string_lossy().into_owned())
        .collect();

    found.sort();
    found.dedup();

    if found.is_empty() { RunState::Clear } else { RunState::Running(found) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// That the scan narrows to one installation is proved where a second one
    /// exists to narrow against: `orng-tools`' preparation tests run against a
    /// copy while the real Bitwig may well be open.
    #[test]
    fn scanning_does_not_panic() {
        let Ok(install) = Installation::discover() else {
            eprintln!("no Bitwig Studio installed, skipping");
            return;
        };
        let _ = running_state(&install);
    }
}
