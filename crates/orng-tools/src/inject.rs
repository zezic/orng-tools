// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! The class preparation adds, and the two calls that reach it.
//!
//! This is what makes a prepared installation read the entry list instead of
//! having the list baked into it. The helper is a fixed class: its bytecode is
//! the same for every Bitwig release, and only the names in its constant pool
//! are rewritten to whatever the build in front of us calls things. Those names
//! never leave the archive they describe, so nothing here writes an obfuscated
//! name down.
//!
//! Both calls go at the very end of something Bitwig already runs:
//!
//! - the content registry's class initialiser, after its own several hundred
//!   registrations, so ours are added to a table that is already complete;
//! - the entitlement object's constructor, after its grant maps exist.
//!
//! Appending rather than inserting is deliberate. Both methods are straight-line
//! and end in a single `return`, so the edit cannot change any existing branch
//! target, disturb a stack map frame, or run before the state it depends on.

use std::collections::HashMap;

use bitwig_classfile::edit::{self, Class, Code, Instr, Pos};
use bitwig_classfile::pool;
use bitwig_registry::{EntitlementBinding, RegistryBinding};

use crate::{Binding, Error, Result};

/// The helper, as assembly. See `java/OrngRegistry.java` for what it does.
const HELPER_SOURCE: &str = include_str!("../java/OrngRegistry.j");

/// The helper's own name, which is ours and so is never retargeted.
pub(crate) const HELPER_CLASS: &str = "OrngRegistry";
pub(crate) const HELPER_ENTRY: &str = "OrngRegistry.class";

/// What the helper prints when it cannot apply the entry list.
///
/// It catches everything rather than throwing, because it runs inside a class
/// initialiser and a constructor that Bitwig cannot start without. That makes
/// its output the only evidence it failed, which is why verification reads it.
/// Kept in step with `java/OrngRegistry.java` by a test.
pub(crate) const HELPER_FAILURE: &str = "orng-registry: could not apply the entry list: ";

const INSTALL: &str = "install";
const INSTALL_DESCRIPTOR: &str = "()V";
const GRANT: &str = "grant";
const GRANT_DESCRIPTOR: &str = "(Ljava/util/HashMap;Ljava/util/HashMap;Ljava/util/HashMap;)V";
const GRANT_MAP_DESCRIPTOR: &str = "Ljava/util/HashMap;";

/// Names the helper is compiled against, each standing for something this build
/// calls by an obfuscated name. Every one must be found exactly once; see
/// [`helper_class`].
const PLACEHOLDER_REGISTRY: &str = "orng/placeholder/Registry";
const PLACEHOLDER_KIND: &str = "orng/placeholder/Kind";
const PLACEHOLDER_GRANT: &str = "orng/placeholder/Grant";
const PLACEHOLDER_REGISTER: &str = "register";
const PLACEHOLDER_REGISTER_DESCRIPTOR: &str =
    "(Ljava/util/UUID;Ljava/lang/String;Lorng/placeholder/Kind;Ljava/lang/String;Z)V";

/// Assemble the helper and point it at this build.
///
/// Every placeholder must be replaced exactly once. A placeholder that is missing
/// means the assembly no longer has the shape this code assumes, and one found
/// twice means a replacement would land somewhere it was not meant to; either
/// way the helper would call something that is not there, and the only safe
/// answer is to refuse.
pub(crate) fn helper_class(binding: &Binding) -> Result<Vec<u8>> {
    let replacements = HashMap::from([
        (PLACEHOLDER_REGISTRY, binding.registry.class.clone()),
        (PLACEHOLDER_KIND, binding.registry.kind_enum.clone()),
        (PLACEHOLDER_GRANT, binding.entitlement.row_class.clone()),
        (PLACEHOLDER_REGISTER, binding.registry.register_method.clone()),
        (PLACEHOLDER_REGISTER_DESCRIPTOR, binding.registry.register_descriptor.clone()),
    ]);

    let (retargeted, hits) = pool::replace_strings(&edit::assemble(HELPER_SOURCE)?, &replacements)?;
    for placeholder in replacements.keys() {
        match hits.get(*placeholder) {
            Some(1) => {}
            found => {
                return Err(Error::HelperPlaceholder {
                    placeholder,
                    found: found.copied().unwrap_or(0),
                });
            }
        }
    }
    Ok(retargeted)
}

/// Add the call at the end of the content registry's class initialiser.
pub(crate) fn call_from_registry(class: &[u8], registry: &RegistryBinding) -> Result<Vec<u8>> {
    let mut parsed = edit::parse(class)?;
    let call = edit::add_method_ref(&mut parsed, HELPER_CLASS, INSTALL, INSTALL_DESCRIPTOR)?;

    append_at_end_of(&mut parsed, "<clinit>", &registry.class, |code| {
        // Takes no arguments and returns nothing, so it needs no stack at all.
        appended(code, vec![Instr::Invokestatic(call)], 0)
    })?;
    edit::reassemble(&parsed).map_err(Error::from)
}

/// Add the call at the end of the entitlement object's constructor.
///
/// The grant maps are read here and passed as arguments rather than read inside
/// the helper, which keeps every obfuscated field name in the class that already
/// knows it and leaves the helper's own signature in JDK types.
pub(crate) fn call_from_entitlement<'a>(
    class: &'a [u8],
    entitlement: &'a EntitlementBinding,
) -> Result<Vec<u8>> {
    let mut parsed = edit::parse(class)?;
    let call = edit::add_method_ref(&mut parsed, HELPER_CLASS, GRANT, GRANT_DESCRIPTOR)?;

    let mut loads = Vec::with_capacity(entitlement.grant_fields.len() * 2 + 1);
    for field in &entitlement.grant_fields {
        let reference =
            edit::add_field_ref(&mut parsed, &entitlement.class, field, GRANT_MAP_DESCRIPTOR)?;
        loads.push(Instr::Aload0);
        loads.push(Instr::Getfield(reference));
    }
    loads.push(Instr::Invokestatic(call));

    // The three maps sit on the stack together at the call.
    let stack = u16::try_from(entitlement.grant_fields.len()).expect("three grant maps");
    append_at_end_of(&mut parsed, "<init>", &entitlement.class, |code| {
        appended(code, loads, stack)
    })?;
    edit::reassemble(&parsed).map_err(Error::from)
}

/// Run `append` over the named method, naming the class if it is not there.
fn append_at_end_of(
    class: &mut Class<'_>,
    method: &str,
    owner: &str,
    append: impl FnOnce(&mut Code<'_>) -> Result<()>,
) -> Result<()> {
    let mut result = Ok(());
    let found = edit::edit_method(class, method, |code| result = append(code));
    if !found {
        return Err(Error::NoSuchMethod { class: owner.to_owned(), method: method.to_owned() });
    }
    result
}

/// Put `instructions` at the end of a method that ends in `return`.
///
/// The trailing `return` is dropped and a fresh one closes the method, so the
/// added code runs after everything the method already did.
///
/// A position is a byte offset, and every one of them becomes a label when the
/// class is written back. Two of those labels are spoken for. The trailing
/// `return`'s own offset is referenced by the line number table, so the first
/// added instruction takes it over rather than leaving it dangling. One past it
/// is the end-of-code marker. Everything from two past it is free, and the
/// assembler resolves labels back to real offsets regardless of the gap.
fn appended(code: &mut Code<'_>, instructions: Vec<Instr>, stack: u16) -> Result<()> {
    let Some((Pos(last), Instr::Return)) = code.bytecode.0.last() else {
        return Err(Error::NotStraightLine);
    };
    let mut offsets = std::iter::once(*last).chain(*last + 2..);
    code.bytecode.0.pop();

    for instruction in instructions.into_iter().chain([Instr::Return]) {
        let offset = offsets.next().expect("the offsets past the end do not run out");
        code.bytecode.0.push((Pos(offset), instruction));
    }

    // The added code starts from an empty stack, so what it needs is its own
    // peak and not an addition to the method's.
    code.stack = code.stack.max(stack);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helper_assembles_before_it_is_retargeted() {
        let bytes = edit::assemble(HELPER_SOURCE).unwrap();
        assert_eq!(&bytes[..4], b"\xca\xfe\xba\xbe");
    }

    /// The placeholders are only useful if they are actually in there. A typo in
    /// either this file or the assembly would otherwise surface as a refusal to
    /// prepare, on a machine with Bitwig installed, with no hint which end moved.
    #[test]
    fn every_placeholder_is_in_the_assembly_exactly_once() {
        let bytes = edit::assemble(HELPER_SOURCE).unwrap();
        let strings = pool::strings(&bytes).unwrap();
        for placeholder in [
            PLACEHOLDER_REGISTRY,
            PLACEHOLDER_KIND,
            PLACEHOLDER_GRANT,
            PLACEHOLDER_REGISTER,
            PLACEHOLDER_REGISTER_DESCRIPTOR,
        ] {
            let found = strings.iter().filter(|s| **s == placeholder.as_bytes()).count();
            assert_eq!(found, 1, "{placeholder} appears {found} times");
        }
    }

    /// Verification decides whether a patch is good by reading for this string.
    /// If the Java were reworded and this were not, a helper that failed on every
    /// launch would verify clean.
    #[test]
    fn the_failure_message_is_the_one_the_helper_prints() {
        let bytes = edit::assemble(HELPER_SOURCE).unwrap();
        let strings = pool::strings(&bytes).unwrap();
        // The message is the constant half of a string concatenation, so what is
        // in the pool is this followed by the marker standing for the throwable.
        assert!(
            strings.iter().any(|s| s.starts_with(HELPER_FAILURE.as_bytes())),
            "the helper does not print {HELPER_FAILURE:?}"
        );
    }

    /// The methods the helper is called from are named by name, so the helper
    /// itself must declare exactly the two the call sites reach for.
    #[test]
    fn the_helper_declares_what_the_call_sites_name() {
        let bytes = edit::assemble(HELPER_SOURCE).unwrap();
        let class = edit::parse(&bytes).unwrap();
        for (name, descriptor) in
            [(INSTALL, INSTALL_DESCRIPTOR), (GRANT, GRANT_DESCRIPTOR)]
        {
            assert!(
                (0..class.methods.len())
                    .filter_map(|i| edit::method_signature(&class, i))
                    .any(|signature| signature == (name, descriptor)),
                "{name}{descriptor} is not in the helper"
            );
        }
        assert_eq!(edit::this_class(&class), Some(HELPER_CLASS));
    }
}
