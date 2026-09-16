use sysinfo::{ProcessRefreshKind, RefreshKind, System};

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

/// Scan for running Bitwig processes.
///
/// Matches on process name rather than executable path: the audio engine and
/// plugin hosts live in nested bundles, and on Linux they may be started from a
/// copy of the install the caller never resolved.
pub fn running_state() -> RunState {
    let refresh = RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing());
    let system = System::new_with_specifics(refresh);

    let mut found: Vec<String> = system
        .processes()
        .values()
        .filter_map(|process| {
            let name = process.name().to_string_lossy().to_ascii_lowercase();
            PROCESS_MARKERS
                .iter()
                .any(|marker| name.contains(marker))
                .then(|| process.name().to_string_lossy().into_owned())
        })
        .collect();

    found.sort();
    found.dedup();

    if found.is_empty() { RunState::Clear } else { RunState::Running(found) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanning_does_not_panic() {
        let _ = running_state();
    }
}
