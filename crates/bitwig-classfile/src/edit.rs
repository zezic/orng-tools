// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Structural class editing, backed by krakatau2.
//!
//! Writing a class back goes through disassemble/assemble rather than a direct
//! serializer: it recomputes the pieces an edit invalidates (stack map frames,
//! branch offsets) instead of asking the caller to keep them consistent.

use krakatau2::lib::classfile::attrs::AttrBody;
use krakatau2::lib::classfile::cpool::Const;
use krakatau2::lib::{
    AssemblerOptions, DisassemblerOptions, ParserOptions, assemble as assemble_all, classfile,
};

use crate::{Error, Result};

// Types this module's own signatures are written in. A caller cannot use
// `edit_method_code`, `find_window` or `parse` without them, so they are
// re-exported here rather than making every caller depend on krakatau2.
pub use krakatau2::lib::classfile::code::{Bytecode, Instr, Pos};
pub use krakatau2::lib::classfile::parse::Class;

/// Parse, hand the class to `edit`, and serialize the result.
///
/// The parsed class borrows `bytes`, so it never escapes this call. Strings a
/// caller intends to push into the constant pool must be created before the
/// call so that they outlive the closure.
pub fn edit_class<E>(bytes: &[u8], edit: E) -> Result<Vec<u8>>
where
    E: FnOnce(&mut Class<'_>) -> Result<()>,
{
    let mut class = parse(bytes)?;
    edit(&mut class)?;
    reassemble(&class)
}

pub fn parse(bytes: &[u8]) -> Result<Class<'_>> {
    classfile::parse(bytes, ParserOptions { no_short_code_attr: true })
        .map_err(|e| Error::ClassParse(format!("{e:?}")))
}

pub fn reassemble(class: &Class<'_>) -> Result<Vec<u8>> {
    let mut disassembled = Vec::new();
    krakatau2::lib::disassemble::disassemble(
        &mut disassembled,
        class,
        DisassemblerOptions { roundtrip: true },
    )
    .map_err(|e| Error::Assembly(e.to_string()))?;

    let source =
        std::str::from_utf8(&disassembled).map_err(|e| Error::Assembly(e.to_string()))?;
    assemble(source)
}

/// Assemble a single class from Krakatau assembly source.
///
/// How a class with no counterpart in the archive is built: its source is text
/// in this repository, so producing it needs no JDK and the bytes that reach an
/// installation stay reviewable.
pub fn assemble(source: &str) -> Result<Vec<u8>> {
    let assembled =
        assemble_all(source, AssemblerOptions {}).map_err(|e| Error::Assembly(format!("{e:?}")))?;
    let [(_name, data)] = <[_; 1]>::try_from(assembled)
        .map_err(|v| Error::Assembly(format!("expected one class, got {}", v.len())))?;
    Ok(data)
}

/// The name of a method, as recorded in the constant pool.
pub fn method_name<'a>(class: &'a Class<'_>, index: usize) -> Option<&'a str> {
    let method = class.methods.get(index)?;
    class.cp.utf8(method.name).and_then(|b| std::str::from_utf8(b).ok())
}

/// Run `edit` over the bytecode of the named method.
///
/// Returns whether the method was found, so callers can fail loudly instead of
/// silently producing an unpatched class.
#[must_use = "a false return means the method was not found and nothing was edited"]
pub fn edit_method_code(
    class: &mut Class<'_>,
    name: &str,
    edit: impl FnOnce(&mut Bytecode),
) -> bool {
    let Some(index) = (0..class.methods.len()).find(|i| method_name(class, *i) == Some(name))
    else {
        return false;
    };
    let Some(attr) = class.methods[index].attrs.first_mut() else {
        return false;
    };
    let AttrBody::Code((code, _)) = &mut attr.body else {
        return false;
    };
    edit(&mut code.bytecode);
    true
}

/// Run `edit` over the bytecode of every method that has any.
pub fn edit_all_code(class: &mut Class<'_>, mut edit: impl FnMut(&mut Bytecode)) {
    for method in &mut class.methods {
        let Some(attr) = method.attrs.first_mut() else { continue };
        if let AttrBody::Code((code, _)) = &mut attr.body {
            edit(&mut code.bytecode);
        }
    }
}

/// Position of the first window of `len` instructions satisfying `matches`.
pub fn find_window(
    bytecode: &Bytecode,
    len: usize,
    matches: impl Fn(&[(krakatau2::lib::classfile::code::Pos, Instr)]) -> bool,
) -> Option<usize> {
    bytecode.0.windows(len).position(matches)
}

/// A field or method reference resolved out of the constant pool.
///
/// Resolving references by shape rather than by name is what lets anchors
/// survive an obfuscation reshuffle: the owner's name changes between builds,
/// the descriptor's JDK types do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberRef<'a> {
    pub owner: &'a str,
    pub name: &'a str,
    pub descriptor: &'a str,
}

fn utf8<'a>(class: &'a Class<'_>, index: u16) -> Option<&'a str> {
    class.cp.utf8(index).and_then(|b| std::str::from_utf8(b).ok())
}

/// The internal name behind a `Class` pool entry.
pub fn class_name<'a>(class: &'a Class<'_>, index: u16) -> Option<&'a str> {
    match class.cp.0.get(index as usize)? {
        Const::Class(name) => utf8(class, *name),
        _ => None,
    }
}

/// The text behind a `String` pool entry, as loaded by `ldc`.
pub fn string_const<'a>(class: &'a Class<'_>, index: u16) -> Option<&'a str> {
    match class.cp.0.get(index as usize)? {
        Const::Str(text) => utf8(class, *text),
        _ => None,
    }
}

/// This class's own internal name.
pub fn this_class<'a>(class: &'a Class<'_>) -> Option<&'a str> {
    class_name(class, class.this)
}

fn member_ref<'a>(class: &'a Class<'_>, owner: u16, nat: u16) -> Option<MemberRef<'a>> {
    let Const::NameAndType(name, descriptor) = class.cp.0.get(nat as usize)? else {
        return None;
    };
    Some(MemberRef {
        owner: class_name(class, owner)?,
        name: utf8(class, *name)?,
        descriptor: utf8(class, *descriptor)?,
    })
}

pub fn field_ref<'a>(class: &'a Class<'_>, index: u16) -> Option<MemberRef<'a>> {
    match class.cp.0.get(index as usize)? {
        Const::Field(owner, nat) => member_ref(class, *owner, *nat),
        _ => None,
    }
}

pub fn method_ref<'a>(class: &'a Class<'_>, index: u16) -> Option<MemberRef<'a>> {
    match class.cp.0.get(index as usize)? {
        Const::Method(owner, nat) | Const::InterfaceMethod(owner, nat) => {
            member_ref(class, *owner, *nat)
        }
        _ => None,
    }
}

/// Declared name and descriptor of the method at `index`.
pub fn method_signature<'a>(class: &'a Class<'_>, index: usize) -> Option<(&'a str, &'a str)> {
    let method = class.methods.get(index)?;
    Some((utf8(class, method.name)?, utf8(class, method.desc)?))
}

/// Bytecode of the method at `index`, if it has a `Code` attribute.
pub fn method_code<'b>(class: &'b Class<'_>, index: usize) -> Option<&'b Bytecode> {
    let attr = class.methods.get(index)?.attrs.first()?;
    match &attr.body {
        AttrBody::Code((code, _)) => Some(&code.bytecode),
        _ => None,
    }
}

/// Indices of every method matching `descriptor`.
pub fn methods_with_descriptor(class: &Class<'_>, descriptor: &str) -> Vec<usize> {
    (0..class.methods.len())
        .filter(|i| method_signature(class, *i).is_some_and(|(_, d)| d == descriptor))
        .collect()
}

/// Access flags of the method at `index`.
pub fn method_access(class: &Class<'_>, index: usize) -> Option<u16> {
    class.methods.get(index).map(|m| m.access)
}
