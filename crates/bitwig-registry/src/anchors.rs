//! Resolving Bitwig's internals without knowing a single obfuscated name.
//!
//! Every anchor here is a user-facing string, an unobfuscated enum constant, a
//! JDK type inside a descriptor, or a bytecode shape. Class, method and field
//! names change on every release; none of those do.
//!
//! Resolution fails closed. An anchor that cannot be found, or that matches
//! ambiguously, is reported by name rather than guessed at, so an unrecognised
//! build is refused instead of mispatched.

use std::collections::BTreeMap;
use std::ops::ControlFlow;

use bitwig_classfile::edit::{self, MemberRef};
use bitwig_classfile::{Jar, pool};
use bitwig_document::{BitwigVersion, Kind};
use krakatau2::lib::classfile::code::Instr;
use krakatau2::lib::classfile::parse::Class;

use crate::{Anchor, Error, Result};

/// UI label carried by the class holding the tamper guard. A user-facing string
/// is not obfuscated and rarely changes.
const GUARD_ANCHOR: &str = "Apply Device Remote Control Changes To All Devices";

/// How many registered library paths a class must carry to be the registry.
/// Bitwig 6.1 has 428; anything else in the archive has none.
const MIN_LIBRARY_PATHS: usize = 100;

/// Everything the patcher and the injected helper need to know about one build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Which Bitwig this is. `None` when the version string is not where it
    /// usually sits, which is reportable but does not block a patch.
    pub build: Option<BuildId>,
    pub registry: RegistryBinding,
    pub entitlement: EntitlementBinding,
    /// Archive entry of the class carrying the tamper guard.
    pub guard_entry: String,
}

/// A Bitwig release, as the archive states it: a version plus the commit it was
/// built from. Identifies an installation across updates and names a backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildId {
    pub version: BitwigVersion,
    pub revision: String,
}

impl BuildId {
    /// Parse `"<version> - <40 hex>"`, the shape Bitwig stores its build under.
    ///
    /// The format is the anchor, not any class name: the string is a constant in
    /// whichever class happens to carry it that release.
    fn parse(text: &str) -> Option<Self> {
        let (version, revision) = text.split_once('-')?;
        let revision = revision.trim();
        let committed = revision.len() == 40 && revision.chars().all(|c| c.is_ascii_hexdigit());
        committed
            .then(|| BitwigVersion::parse(version))
            .flatten()
            .map(|version| BuildId { version, revision: revision.to_owned() })
    }

    /// Short form for a backup file name or a status line.
    pub fn short_revision(&self) -> &str {
        &self.revision[..8.min(self.revision.len())]
    }
}

impl std::fmt::Display for BuildId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.version, self.short_revision())
    }
}

/// The class that registers every native device, modulator and Grid module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryBinding {
    pub entry: String,
    pub class: String,
    /// Private static `(UUID, String, <kind enum>, String, boolean)`.
    pub register_method: String,
    pub register_descriptor: String,
    pub kind_enum: String,
    /// Static field of the kind enum holding each constant.
    pub kind_fields: BTreeMap<Kind, String>,
}

/// The license entitlement check. A UUID absent from its grant maps is treated
/// as unavailable on any edition that does not set the "everything allowed" flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitlementBinding {
    pub entry: String,
    pub class: String,
    /// All three grant maps, one per kind. Each entry grants one identity.
    ///
    /// Which map belongs to which kind is deliberately not resolved. The
    /// dispatcher that would tell us is three hops away and fragile, and
    /// seeding every map is equivalent in effect: a UUID identifies exactly one
    /// document, so a device's UUID is never asked about as a modulator.
    pub grant_fields: Vec<String>,
    /// Row type stored in the grant maps; has a `(UUID)` constructor.
    pub row_class: String,
}

impl Binding {
    /// Resolve every anchor in a single sweep of the archive.
    pub fn resolve(jar: &Jar) -> Result<Self> {
        let mut registry_class = None;
        let mut entitlement_class = None;
        let mut guard_entry = None;
        let mut build = None;

        jar.visit_classes(|name, bytes| {
            if registry_class.is_none() && looks_like_registry(bytes) {
                registry_class = Some((name.to_owned(), bytes.to_vec()));
            }
            if entitlement_class.is_none() && looks_like_entitlement(bytes) {
                entitlement_class = Some((name.to_owned(), bytes.to_vec()));
            }
            if guard_entry.is_none() && pool::contains(bytes, GUARD_ANCHOR) {
                guard_entry = Some(name.to_owned());
            }
            if build.is_none() {
                build = find_build_id(bytes);
            }
            // The build string is cheap to keep looking for but not worth
            // sweeping the whole archive on its own.
            if registry_class.is_some() && entitlement_class.is_some() && guard_entry.is_some() {
                return ControlFlow::Break(());
            }
            ControlFlow::Continue(())
        })?;

        let (registry_entry, registry_bytes) =
            registry_class.ok_or(Error::Unresolved(Anchor::RegistryClass))?;
        let (entitlement_entry, entitlement_bytes) =
            entitlement_class.ok_or(Error::Unresolved(Anchor::EntitlementClass))?;
        let guard_entry = guard_entry.ok_or(Error::Unresolved(Anchor::GuardClass))?;

        let mut registry = resolve_registry(&registry_entry, &registry_bytes)?;
        let enum_bytes = jar.entry(&format!("{}.class", registry.kind_enum))?;
        registry.kind_fields = resolve_kind_fields(&registry.kind_enum, &enum_bytes)?;

        let entitlement = resolve_entitlement(&entitlement_entry, &entitlement_bytes)?;

        Ok(Binding { build, registry, entitlement, guard_entry })
    }
}

/// Scan a class's pool for the build string.
fn find_build_id(bytes: &[u8]) -> Option<BuildId> {
    // Every candidate holds a digit, a dot and a dash; almost no class does.
    if !bytes.windows(3).any(|w| w[1] == b'.' && w[0].is_ascii_digit()) {
        return None;
    }
    pool::strings(bytes)
        .ok()?
        .into_iter()
        .filter_map(|s| std::str::from_utf8(s).ok())
        .find_map(BuildId::parse)
}

/// The registry is the only class carrying a wall of `devices/x.bwdevice`-shaped
/// strings. Content, not identifiers, so obfuscation cannot touch it.
fn looks_like_registry(bytes: &[u8]) -> bool {
    if !bytes.windows(9).any(|w| w == b".bwdevice") {
        return false;
    }
    let Ok(strings) = pool::strings(bytes) else { return false };
    strings
        .iter()
        .filter(|s| std::str::from_utf8(s).is_ok_and(is_library_path))
        .count()
        >= MIN_LIBRARY_PATHS
}

fn is_library_path(text: &str) -> bool {
    Kind::ALL.iter().any(|kind| {
        text.starts_with(kind.library_subdir())
            && text[kind.library_subdir().len()..].starts_with('/')
            && text.ends_with(kind.extension())
    })
}

/// Cheap pre-filter for the entitlement class: it must mention the UUID-keyed
/// predicate descriptor and the map type backing its grant maps.
fn looks_like_entitlement(bytes: &[u8]) -> bool {
    pool::contains(bytes, ENTITLEMENT_DESCRIPTOR)
        && pool::contains(bytes, "Ljava/util/HashMap;")
        && edit::parse(bytes).is_ok_and(|class| grant_fields(&class).len() == GRANT_MAP_COUNT)
}

const ENTITLEMENT_DESCRIPTOR: &str = "(Ljava/util/UUID;)Z";
/// One grant map per kind.
const GRANT_MAP_COUNT: usize = 3;

fn resolve_registry(entry: &str, bytes: &[u8]) -> Result<RegistryBinding> {
    let class = edit::parse(bytes).map_err(Error::Classfile)?;
    let class_name = edit::this_class(&class)
        .ok_or(Error::Unresolved(Anchor::RegistryClass))?
        .to_owned();

    // Private static (UUID, String, <enum>, String, boolean) -> void. The JDK
    // types either side of the enum make the shape unique.
    const PREFIX: &str = "(Ljava/util/UUID;Ljava/lang/String;L";
    const SUFFIX: &str = ";Ljava/lang/String;Z)V";

    let matches: Vec<_> = (0..class.methods.len())
        .filter_map(|i| edit::method_signature(&class, i))
        .filter(|(_, desc)| desc.starts_with(PREFIX) && desc.ends_with(SUFFIX))
        .collect();

    let [(name, descriptor)] = matches.as_slice() else {
        return Err(Error::Ambiguous { anchor: Anchor::RegisterMethod, found: matches.len() });
    };
    let kind_enum = descriptor[PREFIX.len()..descriptor.len() - SUFFIX.len()].to_owned();

    Ok(RegistryBinding {
        entry: entry.to_owned(),
        class: class_name,
        register_method: (*name).to_owned(),
        register_descriptor: (*descriptor).to_owned(),
        kind_enum,
        kind_fields: BTreeMap::new(),
    })
}

/// Map each kind to the enum's static field holding it.
///
/// An enum's constant names survive obfuscation because `Enum.valueOf` needs
/// them, so `<clinit>`'s `ldc "DEVICE" ... putstatic` pairs are a stable anchor.
fn resolve_kind_fields(enum_class: &str, bytes: &[u8]) -> Result<BTreeMap<Kind, String>> {
    let class = edit::parse(bytes).map_err(Error::Classfile)?;
    let descriptor = format!("L{enum_class};");
    let clinit = (0..class.methods.len())
        .find(|i| edit::method_signature(&class, *i).is_some_and(|(n, _)| n == "<clinit>"))
        .and_then(|i| edit::method_code(&class, i))
        .ok_or(Error::Unresolved(Anchor::KindEnum))?;

    let mut fields = BTreeMap::new();
    let mut pending: Option<&str> = None;
    for (_, instr) in &clinit.0 {
        match instr {
            Instr::Ldc(i) => pending = edit::string_const(&class, *i as u16),
            Instr::LdcW(i) => pending = edit::string_const(&class, *i),
            Instr::Putstatic(i) => {
                let (Some(text), Some(field)) = (pending.take(), edit::field_ref(&class, *i))
                else {
                    continue;
                };
                if field.descriptor != descriptor {
                    continue;
                }
                if let Some(kind) = Kind::ALL.iter().find(|k| k.enum_constant() == text) {
                    fields.insert(*kind, field.name.to_owned());
                }
            }
            _ => {}
        }
    }

    if fields.len() != Kind::ALL.len() {
        return Err(Error::Ambiguous { anchor: Anchor::KindEnum, found: fields.len() });
    }
    Ok(fields)
}

fn resolve_entitlement(entry: &str, bytes: &[u8]) -> Result<EntitlementBinding> {
    let class = edit::parse(bytes).map_err(Error::Classfile)?;
    let class_name = edit::this_class(&class)
        .ok_or(Error::Unresolved(Anchor::EntitlementClass))?
        .to_owned();

    let grant_fields = grant_fields(&class);
    if grant_fields.len() != GRANT_MAP_COUNT {
        return Err(Error::Ambiguous {
            anchor: Anchor::EntitlementGrants,
            found: grant_fields.len(),
        });
    }

    let row_class = resolve_row_class(&class, &class_name)
        .ok_or(Error::Unresolved(Anchor::EntitlementRowClass))?;

    Ok(EntitlementBinding {
        entry: entry.to_owned(),
        class: class_name,
        grant_fields,
        row_class,
    })
}

/// Grant map fields, read off the shape of each entitlement predicate:
///
/// ```text
/// aload_0; getfield <flag:Z>; ifne ok
/// aload_0; getfield <grants:HashMap>; aload_1; invokevirtual HashMap.get
/// ```
fn grant_fields(class: &Class<'_>) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    for index in edit::methods_with_descriptor(class, ENTITLEMENT_DESCRIPTOR) {
        let Some(code) = edit::method_code(class, index) else { continue };
        let found = edit::find_window(code, 6, |w| {
            matches!(
                (&w[0].1, &w[1].1, &w[2].1, &w[3].1, &w[4].1, &w[5].1),
                (
                    Instr::Getfield(flag),
                    Instr::Ifne(_),
                    Instr::Aload0,
                    Instr::Getfield(map),
                    Instr::Aload1,
                    Instr::Invokevirtual(get),
                ) if is_boolean_field(class, *flag)
                    && is_map_field(class, *map)
                    && is_map_get(class, *get)
            )
        });
        let Some(at) = found else { continue };
        let Instr::Getfield(map) = code.0[at + 3].1 else { continue };
        if let Some(field) = edit::field_ref(class, map) {
            let name = field.name.to_owned();
            if !fields.contains(&name) {
                fields.push(name);
            }
        }
    }
    fields
}

fn is_boolean_field(class: &Class<'_>, index: u16) -> bool {
    edit::field_ref(class, index).is_some_and(|f| f.descriptor == "Z")
}

fn is_map_field(class: &Class<'_>, index: u16) -> bool {
    edit::field_ref(class, index).is_some_and(|f| f.descriptor == "Ljava/util/HashMap;")
}

fn is_map_get(class: &Class<'_>, index: u16) -> bool {
    edit::method_ref(class, index).is_some_and(|m| {
        m == MemberRef {
            owner: "java/util/HashMap",
            name: "get",
            descriptor: "(Ljava/lang/Object;)Ljava/lang/Object;",
        }
    })
}

/// The grant row type, found where the constructor seeds a grant map with one
/// built-in identity: `new <row>(uuid)`.
fn resolve_row_class(class: &Class<'_>, own_name: &str) -> Option<String> {
    (0..class.methods.len())
        .filter(|i| edit::method_signature(class, *i).is_some_and(|(n, _)| n == "<init>"))
        .filter_map(|i| edit::method_code(class, i))
        .flat_map(|code| code.0.iter())
        .find_map(|(_, instr)| match instr {
            Instr::Invokespecial(index) => edit::method_ref(class, *index)
                .filter(|m| m.descriptor == "(Ljava/util/UUID;)V" && m.owner != own_name)
                .map(|m| m.owner.to_owned()),
            _ => None,
        })
}
