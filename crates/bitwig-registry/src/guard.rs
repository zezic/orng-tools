// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Bitwig's tamper guard.
//!
//! Bitwig verifies its own archive at runtime. Any modification breaks that
//! seal, and the guard then degrades audio in every project. Preparing an
//! installation therefore always includes disarming it; they are one operation,
//! not two options.
//!
//! The guard compiles to `<load>; sipush 5000; if_icmple ok`. Replacing the load
//! with `iconst_0` makes the comparison always take the normal path.

use bitwig_classfile::edit::{self, Bytecode, Instr};

use crate::{Anchor, Error, Result};

/// Threshold the guard compares against.
const GUARD_LIMIT: i16 = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardState {
    /// The live check is present.
    Armed,
    /// Already neutralised; disarming again would be a no-op.
    Disarmed,
    /// Recognised neither way. Never guessed at, never patched.
    Unknown,
}

/// Report the guard's state without modifying anything.
pub fn inspect(class_bytes: &[u8]) -> Result<GuardState> {
    let class = edit::parse(class_bytes)?;
    let mut state = None;
    for index in 0..class.methods.len() {
        let Some(code) = edit::method_code(&class, index) else { continue };
        let Some(at) = find_guard(code) else { continue };
        state = Some(match code.0[at].1 {
            Instr::Iconst0 => GuardState::Disarmed,
            Instr::Iload(_) | Instr::Iload0 | Instr::Iload1 | Instr::Iload2 | Instr::Iload3 => {
                GuardState::Armed
            }
            _ => GuardState::Unknown,
        });
        break;
    }
    state.ok_or(Error::Unresolved(Anchor::GuardSite))
}

/// Disarm the guard, returning the rewritten class.
///
/// Refuses an unrecognised guard rather than editing blind.
pub fn disarm(class_bytes: &[u8]) -> Result<Vec<u8>> {
    match inspect(class_bytes)? {
        GuardState::Disarmed => return Ok(class_bytes.to_vec()),
        GuardState::Unknown => return Err(Error::Unresolved(Anchor::GuardSite)),
        GuardState::Armed => {}
    }

    let mut patched = false;
    let bytes = edit::edit_class(class_bytes, |class| {
        edit::edit_all_code(class, |code| {
            if patched {
                return;
            }
            if let Some(at) = find_guard(code) {
                code.0[at].1 = Instr::Iconst0;
                patched = true;
            }
        });
        Ok(())
    })?;

    if !patched {
        return Err(Error::Unresolved(Anchor::GuardSite));
    }
    Ok(bytes)
}

/// Index of the value load feeding the guard comparison.
fn find_guard(code: &Bytecode) -> Option<usize> {
    edit::find_window(code, 3, |w| {
        matches!(
            (&w[1].1, &w[2].1),
            (Instr::Sipush(GUARD_LIMIT), Instr::IfIcmple(_))
        )
    })
}
