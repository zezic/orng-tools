//! Reading what a build already has registered.
//!
//! The registry's class initialiser runs in two phases: it parses every UUID
//! into a static field, then calls the registration method once per entry with
//! that field, a display name, a category constant and a library path. Both
//! phases are walked here to recover the whole table.

use std::collections::HashMap;

use bitwig_classfile::edit;
use bitwig_document::Kind;
use krakatau2::lib::classfile::code::Instr;
use krakatau2::lib::classfile::parse::Class;
use uuid::Uuid;

use crate::{Anchor, Error, RegistryBinding, Result};

/// One row of the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub uuid: Uuid,
    pub name: String,
    pub kind: Kind,
    /// Relative to the installation's `Library`, e.g. `devices/Amp.bwdevice`.
    pub library_path: String,
}

/// Read every entry from the registry class.
pub fn read_entries(class_bytes: &[u8], binding: &RegistryBinding) -> Result<Vec<Entry>> {
    let class = edit::parse(class_bytes)?;
    let clinit = (0..class.methods.len())
        .find(|i| edit::method_signature(&class, *i).is_some_and(|(n, _)| n == "<clinit>"))
        .and_then(|i| edit::method_code(&class, i))
        .ok_or(Error::Unresolved(Anchor::RegistryClass))?;

    let uuids = parse_uuid_fields(&class, clinit);
    let kinds: HashMap<&str, Kind> = binding
        .kind_fields
        .iter()
        .map(|(kind, field)| (field.as_str(), *kind))
        .collect();

    let mut entries = Vec::new();
    for (at, (_, instr)) in clinit.0.iter().enumerate() {
        let Instr::Invokestatic(index) = instr else { continue };
        if !is_register_call(&class, *index, binding) || at < 5 {
            continue;
        }
        // getstatic <uuid>; ldc <name>; getstatic <kind>; ldc <path>; iconst_?
        let Instr::Getstatic(uuid_field) = clinit.0[at - 5].1 else { continue };
        let Instr::Getstatic(kind_field) = clinit.0[at - 3].1 else { continue };

        let uuid = edit::field_ref(&class, uuid_field).and_then(|f| uuids.get(f.name));
        let kind = edit::field_ref(&class, kind_field).and_then(|f| kinds.get(f.name));
        let name = load_string(&class, &clinit.0[at - 4].1);
        let path = load_string(&class, &clinit.0[at - 2].1);

        if let (Some(uuid), Some(kind), Some(name), Some(library_path)) = (uuid, kind, name, path) {
            entries.push(Entry { uuid: *uuid, name, kind: *kind, library_path });
        }
    }

    if entries.is_empty() {
        return Err(Error::Ambiguous { anchor: Anchor::RegistryClass, found: 0 });
    }
    Ok(entries)
}

/// Phase one: `ldc "<uuid>"; invokestatic UUID.fromString; putstatic <field>`.
fn parse_uuid_fields<'a>(
    class: &'a Class<'_>,
    clinit: &krakatau2::lib::classfile::code::Bytecode,
) -> HashMap<&'a str, Uuid> {
    let mut fields = HashMap::new();
    let mut pending: Option<Uuid> = None;
    for (_, instr) in &clinit.0 {
        match instr {
            Instr::Ldc(_) | Instr::LdcW(_) => {
                pending = load_string(class, instr).and_then(|s| Uuid::parse_str(&s).ok());
            }
            Instr::Putstatic(index) => {
                if let (Some(uuid), Some(field)) = (pending.take(), edit::field_ref(class, *index)) {
                    fields.insert(field.name, uuid);
                }
            }
            _ => {}
        }
    }
    fields
}

fn is_register_call(class: &Class<'_>, index: u16, binding: &RegistryBinding) -> bool {
    edit::method_ref(class, index).is_some_and(|m| {
        m.owner == binding.class
            && m.name == binding.register_method
            && m.descriptor == binding.register_descriptor
    })
}

fn load_string(class: &Class<'_>, instr: &Instr) -> Option<String> {
    let index = match instr {
        Instr::Ldc(i) => *i as u16,
        Instr::LdcW(i) => *i,
        _ => return None,
    };
    edit::string_const(class, index).map(str::to_owned)
}
