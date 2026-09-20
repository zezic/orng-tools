// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Work that has to happen somewhere this process cannot reach.
//!
//! On Windows an installation lives under `Program Files`, which an ordinary
//! account may not write. **A running process cannot gain rights**: Windows
//! decides what a token holds when the process is created and there is no call
//! that adds the administrators group to one afterwards. Asking for more
//! therefore means asking for *another process*, and `ShellExecuteEx` with the
//! `runas` verb is the call that does it - the consent dialog the user sees is
//! raised by the system, not by this application, which is the whole point of it.
//!
//! So the work is described rather than done, handed to a child that holds the
//! rights, and watched from here.
//!
//! # What crosses, and why it is a recipe
//!
//! [`Job`] carries the *calls to make*, not a serialised [`Update`]. The child
//! replays them through the same constructors the window would have used, so
//! every assertion in `Update::add` and `Update::revise` runs on the far side of
//! the process boundary as well as the near one. A serialised `Update` would
//! arrive already built and prove nothing about itself.
//!
//! Rows cross in the entry list's own format, which already exists and is
//! already tested, rather than in a second per-row representation that could
//! drift from it. Only the handful of small values the list does not carry -
//! which work this is, where it writes, what becomes of a removed file - have a
//! wire form of their own, and those are in this module.
//!
//! # What is deliberately not taken from the job
//!
//! **The installation root is on the command line, never in the job.** The
//! preparation runs the installation's own bundled JVM to verify its patch, so a
//! child that took its root from a file would run `java.exe` from wherever that
//! file pointed, elevated. A command line is set by the parent at
//! `ShellExecuteEx` and read by the child out of its own process; nothing
//! between them can rewrite it.
//!
//! # What is deliberately not discovered by the child
//!
//! **The user's home is on the command line too, and for the opposite reason.**
//! Windows does not necessarily elevate as the same account: a user who is not
//! an administrator answers the consent dialog with an administrator's
//! credentials, and the child then runs as *that* account, with that account's
//! `USERPROFILE`. A child that called [`orng_tools::OrngHome::discover`] would
//! write the entry list and the backup under the administrator's profile, where
//! the JVM Bitwig starts - running as the user - never looks. So the home the
//! window resolved is carried across, exactly as the installation is.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;

use orng_tools::{
    Destination, Document, Installation, Kind, Manifest, OrngHome, Registration, Strategy,
    TheDocument, Update, UserLibrary, Uuid,
};
use serde::{Deserialize, Serialize};

use crate::work::Work;

/// What a child is asked to do.
///
/// Three presses reach inside an installation and only two of them are the same
/// operation, so this is where they meet: a child is started the same way, told
/// the same way and watched the same way whichever it was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Task {
    /// Change what is registered, preparing the installation first if it is not
    /// prepared yet.
    Apply(Job),
    /// Put a pristine copy back over the installation.
    ///
    /// The directory it was taken into, which is the only name a backup has -
    /// the build it is of comes back out of that name, and a directory not
    /// named for a build is not offered as a backup at all. The child holds it
    /// against the copies it can see for itself rather than restoring whatever
    /// path it was handed.
    Restore { from: PathBuf },
}

/// What one run has to do, in a form that can be carried to another process.
///
/// Built at the press whether or not anything is elevated, and turned into an
/// [`Update`] by [`Job::update`] in exactly one place - so the run that happens
/// here and the run that happens in a child are the same run, described once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// Whether the installation is prepared first, or only the entries change.
    pub work: Work,
    /// Where documents go when they are not placed inside the installation.
    ///
    /// Beside the installation root rather than inside the job on the wire:
    /// see the module's note on what the child does not take from a job.
    pub library: PathBuf,
    #[serde(with = "crate::settings::placement")]
    pub placement: Strategy,
    /// The entry list as it stood before any of this.
    #[serde(with = "list")]
    pub base: Manifest,
    /// The rows this run writes.
    ///
    /// Whether a row is an addition or a revision is said by whether a document
    /// travels under its identity, which is the same thing that decides it at
    /// the press: a row with bytes behind it is content, and a row without is
    /// words about content already registered.
    #[serde(with = "list")]
    pub written: Manifest,
    /// The documents to place, by the identity of the row that describes them.
    pub documents: Vec<Placing>,
    /// Identities to forget, and what becomes of each one's file.
    pub removed: Vec<Removal>,
}

/// A [`Manifest`] as the entry list spells it.
///
/// The same choice `settings::placement` makes and for the same reason: the
/// format belongs to this application, and `orng-tools` has no other cause to
/// depend on serde. Parsing on the way in is also where a malformed list is
/// refused - before a job built from it can be acted on.
mod list {
    use orng_tools::Manifest;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(value: &Manifest, to: S) -> Result<S::Ok, S::Error> {
        value.to_tsv().serialize(to)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<Manifest, D::Error> {
        let text = String::deserialize(from)?;
        Manifest::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// A document to place, and the identity it is placed under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placing {
    pub uuid: Uuid,
    /// What the bytes are to be read as.
    #[serde(with = "kind")]
    pub kind: Kind,
    /// The document itself.
    ///
    /// The bytes cross rather than a path to them. A path would be a file the
    /// elevated child reads on the say-so of something running unelevated, and
    /// the bytes are twenty to thirty kilobytes.
    pub bytes: Vec<u8>,
}

/// An identity to forget, and what becomes of the file behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Removal {
    pub uuid: Uuid,
    #[serde(with = "outcome")]
    pub document: TheDocument,
}

/// A [`Kind`] as the extension that names it, which is the one spelling of one
/// that already converts both ways.
///
/// The same choice [`list`] and `settings::placement` make: the type stays a
/// type on both sides of the wire, and a kind that is not one is refused as the
/// message is read rather than carried as a `String` that has to be checked
/// again wherever it is used.
mod kind {
    use orng_tools::Kind;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(value: &Kind, to: S) -> Result<S::Ok, S::Error> {
        value.extension().serialize(to)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<Kind, D::Error> {
        let text = String::deserialize(from)?;
        Kind::from_extension(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("{text} is not a kind of document")))
    }
}

/// What becomes of a removed entry's file, as the word the setting is named by.
///
/// A [`TheDocument`] and not a `bool`, for the reason that type exists at all:
/// the two answers differ by whether work the user cannot get back survives,
/// and `delete_document: true` says that in a form no call site has to be right
/// about twice.
mod outcome {
    use orng_tools::TheDocument;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(value: &TheDocument, to: S) -> Result<S::Ok, S::Error> {
        match value {
            TheDocument::Kept => "kept",
            TheDocument::Deleted => "deleted",
        }
        .serialize(to)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<TheDocument, D::Error> {
        match String::deserialize(from)?.as_str() {
            "kept" => Ok(TheDocument::Kept),
            "deleted" => Ok(TheDocument::Deleted),
            other => Err(serde::de::Error::custom(format!(
                "{other} is not what becomes of a document"
            ))),
        }
    }
}

impl Job {
    /// An empty job against a destination and the list as it stands.
    ///
    /// What it writes is added by the caller, in the same calls [`Update`]
    /// takes, so a press reads the same whether it ends up running here or in a
    /// child holding rights this process does not.
    pub fn against(work: Work, to: &Destination, base: &Manifest) -> Job {
        Job {
            work,
            library: to.library.root().to_path_buf(),
            placement: to.placement,
            base: base.clone(),
            written: Manifest::default(),
            documents: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// Register `document` under `registration`, and place it.
    ///
    /// The identity is the registration's, because that is what the row is found
    /// by on the other side; the kind is the document's, because that is what
    /// its bytes are read as. Where the two disagree, `Update::add` is what
    /// says so, and it says so in both processes.
    pub fn add(&mut self, registration: Registration, document: &Document) {
        self.documents.push(Placing {
            uuid: registration.uuid,
            kind: document.kind(),
            bytes: document.bytes().to_vec(),
        });
        self.written.insert(registration);
    }

    /// Change what the list says about an identity already in it.
    pub fn revise(&mut self, registration: Registration) {
        self.written.insert(registration);
    }

    /// Take an identity out of the list, and say what becomes of its document.
    pub fn remove(&mut self, uuid: Uuid, document: TheDocument) {
        self.removed.push(Removal { uuid, document });
    }

    /// Whether this would change anything, which is what decides if it is worth
    /// a press - and, on Windows, worth a consent dialog.
    pub fn is_empty(&self) -> bool {
        self.written.is_empty() && self.removed.is_empty()
    }

    /// The update this describes, built through the same calls the window makes.
    ///
    /// The one place a [`Job`] becomes an [`Update`], which is what keeps the
    /// local run and the elevated run from being two different runs.
    pub fn update(&self) -> Result<Update, String> {
        let documents: BTreeMap<Uuid, &Placing> =
            self.documents.iter().map(|placing| (placing.uuid, placing)).collect();

        let mut update = Update::to(self.base.clone());
        for row in self.written.entries() {
            match documents.get(&row.uuid) {
                Some(placing) => update.add(row.clone(), placing.document()?),
                None => update.revise(row.clone()),
            }
        }
        for removal in &self.removed {
            update.remove(removal.uuid, removal.document);
        }
        Ok(update)
    }

    /// Where this writes, resolved against the installation and the home the
    /// caller names.
    ///
    /// Both are parameters and not fields for the reasons the module states: in
    /// a child the installation comes off the command line because nothing a
    /// job says may decide which JVM runs, and the home comes off it because a
    /// child may be running as an account whose own home is the wrong one.
    pub fn destination(&self, install: Installation, home: OrngHome) -> Destination {
        Destination::under(install, UserLibrary::at(&self.library), home, self.placement)
    }
}

impl Placing {
    /// The document these bytes are, refused if they are not one.
    ///
    /// Parsed rather than trusted. The bytes arrive from another process and end
    /// up written into somebody's library under an identity the same message
    /// chose; reading them as the kind they claim to be is what says the two
    /// agree before anything is placed. The kind itself was refused earlier, as
    /// the message was read - see [`kind`].
    fn document(&self) -> Result<Document, String> {
        Document::parse(self.kind, self.bytes.clone())
            .map_err(|e| format!("the document for {}: {e}", self.uuid))
    }
}

/// What the child says as it goes.
///
/// A wire form of its own rather than [`crate::work`]'s `Progress`, because the
/// two answer to different things: that one is what the progress dialog draws
/// and may be refactored whenever the dialog changes, and this one is a format
/// two processes have to agree on. Keeping them the same type would make every
/// change to the drawing a change to the protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Report {
    /// Which steps this plan will actually run.
    Planned(Vec<WireStep>),
    /// This step has started, so the one before it has finished.
    Began(WireStep),
    /// The installation is prepared and the entries are being written.
    Registering,
    /// Nothing more is coming: the entry list as it now stands, or why it
    /// stopped.
    Finished(Result<String, String>),
}

/// The preparation's steps, as they cross between processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireStep {
    Backup,
    Patch,
    Verify,
    Activate,
    Link,
}

impl From<orng_tools::Step> for WireStep {
    fn from(step: orng_tools::Step) -> WireStep {
        match step {
            orng_tools::Step::Backup => WireStep::Backup,
            orng_tools::Step::Patch => WireStep::Patch,
            orng_tools::Step::Verify => WireStep::Verify,
            orng_tools::Step::Activate => WireStep::Activate,
            orng_tools::Step::Link => WireStep::Link,
        }
    }
}

impl From<WireStep> for orng_tools::Step {
    fn from(step: WireStep) -> orng_tools::Step {
        match step {
            WireStep::Backup => orng_tools::Step::Backup,
            WireStep::Patch => orng_tools::Step::Patch,
            WireStep::Verify => orng_tools::Step::Verify,
            WireStep::Activate => orng_tools::Step::Activate,
            WireStep::Link => orng_tools::Step::Link,
        }
    }
}

/// One JSON value per line, in both directions.
///
/// A line rather than a length prefix because the whole conversation is JSON
/// already and a newline cannot occur inside one of these values: `serde_json`
/// escapes every control character it writes, and the document bytes cross as
/// an array of numbers rather than as text.
fn send(to: &mut impl Write, what: &impl Serialize) -> std::io::Result<()> {
    let line = serde_json::to_string(what)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    to.write_all(line.as_bytes())?;
    to.write_all(b"\n")?;
    to.flush()
}

/// The flag that says this process is the child, and names the pipe to call
/// back on. Internal: nothing documents it and nothing but [`run`] passes it.
pub const SERVE: &str = "--orng-elevated-apply";
/// The installation the child is to work on, which never travels in the job.
pub const INSTALL: &str = "--orng-install";
/// The home of the account that asked, which the child must not look up for
/// itself - see the module note on what it does not discover.
pub const HOME: &str = "--orng-home";

/// Whether this platform gives an application any way to ask for more rights.
///
/// Windows does, through the consent dialog `runas` raises. The others do not -
/// not in any form this application could use: `sudo` is a terminal's and a
/// graphical prompt means an authorisation framework and a privileged helper
/// that has to be installed, which is a different piece of work and not one an
/// installation under `/Applications` has ever needed.
///
/// So what this decides is whether a press that cannot write is *refused* or
/// merely *carried elsewhere*. On Windows an installation under `Program Files`
/// is the ordinary case and the press must still work.
pub const fn can_ask() -> bool {
    cfg!(windows)
}

/// Carry the job to a process that holds the rights, and watch it.
///
/// `say` is told each step as it is reported. `Report::Finished` never reaches
/// it: how the run ended is this call's own return value, because a caller that
/// had to watch for it in the callback could forget to.
pub fn run(
    task: &Task,
    install: &Installation,
    home: &OrngHome,
    say: &impl Fn(Report),
) -> Result<Manifest, String> {
    #[cfg(windows)]
    {
        self::windows::run(task, install, home, say)
    }
    #[cfg(not(windows))]
    {
        // Unreachable from the window, which refuses the press instead - see
        // `App::blocking`. Said rather than panicked on, because what put us
        // here would be the installation's permissions having changed between
        // the session being read and the press being made, and that is the
        // machine's doing rather than a fault in this code.
        let _ = (task, install, home, say);
        Err("this installation is not writable by this account, and this platform has no \
             way for an application to ask for more rights"
            .to_owned())
    }
}

/// Read the reports a child sends, up to and including how it ended.
///
/// Shared by every transport, because the conversation is the same one whether
/// it crossed a pipe or a pair of streams in a test.
///
/// Windows is the only platform that starts a child at all - see [`can_ask`] -
/// so it is the only one where this is reachable outside a test. Stated as a
/// `cfg` rather than silenced with an `allow`, so that a platform that gains a
/// way to ask has to say so here.
#[cfg(any(windows, test))]
fn listen(
    from: impl Read,
    say: &impl Fn(Report),
) -> Result<Manifest, String> {
    let mut ended = None;
    for line in BufReader::new(from).lines() {
        let line = line.map_err(|e| format!("the elevated run stopped being readable: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let report: Report = serde_json::from_str(&line)
            .map_err(|e| format!("the elevated run said something unreadable: {e}"))?;
        match report {
            Report::Finished(outcome) => ended = Some(outcome),
            progress => say(progress),
        }
    }

    match ended {
        Some(Ok(entries)) => {
            Manifest::parse(&entries).map_err(|e| format!("the list it wrote does not read: {e}"))
        }
        Some(Err(why)) => Err(why),
        // The pipe closed with nothing said about the ending. Silence must not
        // read as success, for the reason `Applying::poll` says it must not.
        None => Err("the elevated run stopped without reporting".to_owned()),
    }
}

/// This process's orders, when it is the child rather than the window.
///
/// Parsed by hand rather than through an argument parser: these two are not a
/// command line anybody types, they are one call between two copies of one
/// binary, and a parser would invite them to grow into an interface that has to
/// be kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serving {
    /// Where to call back.
    pipe: String,
    /// The installation to work on. Never taken from the job - see the module
    /// note on why this one is on the command line.
    install: PathBuf,
    /// The home of the account that asked. Never discovered here - see the
    /// module note on why an elevated process must not look it up.
    home: PathBuf,
}

impl Serving {
    /// The orders, if this process has them.
    ///
    /// `None` for the window, which is every invocation a user makes.
    pub fn from_arguments(arguments: impl Iterator<Item = String>) -> Option<Serving> {
        let mut pipe = None;
        let mut install = None;
        let mut home = None;
        let mut arguments = arguments.skip(1);
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                SERVE => pipe = arguments.next(),
                INSTALL => install = arguments.next().map(PathBuf::from),
                HOME => home = arguments.next().map(PathBuf::from),
                _ => {}
            }
        }
        // All three or none. Any short of that is this application having been
        // started with something it does not understand, and the window is the
        // right answer to that.
        Some(Serving { pipe: pipe?, install: install?, home: home? })
    }

    /// Hold the conversation, and answer with this process's exit code.
    ///
    /// Nought whenever the window was told how it went, including when it went
    /// badly: a failure that was reported is not a failure to report. Only
    /// being unable to say anything at all is a non-zero exit, and then the
    /// exit code is the only thing the window has to go on.
    pub fn serve(self) -> i32 {
        let Ok(channel) = self.connect() else {
            return 2;
        };
        let (input, mut output) = (&channel, &channel);

        match Installation::at(&self.install) {
            Ok(install) => {
                if serve(input, output, install, OrngHome::at(&self.home)).is_err() {
                    return 3;
                }
            }
            // Reported rather than exited on: the window can say this, and a
            // bare exit code cannot.
            Err(why) => {
                let refused = Report::Finished(Err(format!(
                    "{} is not a Bitwig Studio installation: {why}",
                    self.install.display()
                )));
                if send(&mut output, &refused).is_err() {
                    return 3;
                }
            }
        }
        0
    }

    /// Open the pipe the window is listening on.
    fn connect(&self) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new().read(true).write(true).open(&self.pipe)
    }
}

/// The child's half of the conversation: read one job, run it, report.
///
/// Takes what it reads and what it writes rather than opening them, so that the
/// protocol can be exercised without a process at all - and so that the one
/// piece that is genuinely Windows' (the pipe, and the elevation that created
/// this process) is the only piece that cannot be.
///
/// The installation and the home are the caller's, off the command line. See
/// the module's two notes on why neither is the child's to choose.
pub fn serve(
    input: impl Read,
    mut output: impl Write,
    install: Installation,
    home: OrngHome,
) -> std::io::Result<()> {
    let mut lines = BufReader::new(input).lines();
    let line = match lines.next() {
        Some(line) => line?,
        // The parent went away before it said what to do. Nothing has been
        // written and there is nobody left to tell.
        None => return Ok(()),
    };

    let outcome = run_job(&line, install, home, &mut output);
    send(&mut output, &Report::Finished(outcome))
}

/// Parse the task, carry it out, and report every step as it goes.
fn run_job(
    line: &str,
    install: Installation,
    home: OrngHome,
    output: &mut impl Write,
) -> Result<String, String> {
    let task: Task =
        serde_json::from_str(line).map_err(|e| format!("the job did not read: {e}"))?;
    match task {
        Task::Apply(job) => apply(job, install, home, output),
        Task::Restore { from } => restore(&from, &install, &home),
    }
}

/// Prepare if this press prepares, then write the entries.
///
/// The twin of `work::run`, step for step. Both take their update from
/// `Job::update`, so what differs between them is where they run and nothing
/// about what they do.
fn apply(
    job: Job,
    install: Installation,
    home: OrngHome,
    output: &mut impl Write,
) -> Result<String, String> {
    let to = job.destination(install, home);
    let update = job.update()?;

    if job.work == Work::PrepareThenEntries {
        let plan = orng_tools::Plan::compute(&to).map_err(|e| e.to_string())?;
        let steps = plan.steps().map(WireStep::from).collect();
        let _ = send(output, &Report::Planned(steps));
        plan.apply(|step| {
            let _ = send(output, &Report::Began(step.into()));
        })
        .map_err(|e| e.to_string())?;
        let _ = send(output, &Report::Registering);
    }
    update.apply(&to).map(|manifest| manifest.to_tsv()).map_err(|e| e.to_string())
}

/// Put a pristine copy back over the installation.
///
/// **The path is checked against the copies this machine actually holds** and
/// not restored because it was named. A backup is a directory under
/// `~/.orng/backups` whose name is the build it was taken from, and `Backup::
/// list` is what knows that - so a path that is not one of those is refused
/// here rather than being copied over an installation by a process holding
/// rights the window did not.
///
/// The entry list is deliberately untouched, which is what leaves the
/// installation in the `Needs re-apply` state the window already says.
///
/// The home is the caller's and is never discovered here: the copies to hold
/// `from` against are the ones under the *asking* account's home, and an
/// elevated process may not be running as that account at all.
fn restore(
    from: &std::path::Path,
    install: &Installation,
    home: &OrngHome,
) -> Result<String, String> {
    let backups = orng_tools::Backup::list(home).map_err(|e| e.to_string())?;
    let backup = backups
        .into_iter()
        .find(|backup| backup.directory() == from)
        .ok_or_else(|| format!("{} is not a backup this machine holds", from.display()))?;

    backup.restore(install).map_err(|e| e.to_string())?;
    // A restore writes no entry list, so there is none to hand back. The window
    // reads the machine again afterwards, which is where its list comes from.
    Ok(Manifest::default().to_tsv())
}

/// Windows' half: the pipe, the consent dialog, and waiting on the child.
///
/// None of this is reachable on another platform and none of it is a general
/// facility. It is here rather than behind a crate because what it does is one
/// specific thing - raise one consent dialog, hold one conversation - and a
/// dependency that did it in general would still leave every decision below to
/// be made here.
#[cfg(windows)]
mod windows {
    use std::ffi::{OsStr, OsString};
    use std::io::{Read, Write};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{FromRawHandle, OwnedHandle};

    use orng_tools::{Installation, Manifest, OrngHome};
    use windows_sys::Win32::Foundation::{
        ERROR_CANCELLED, ERROR_IO_PENDING, GetLastError, HANDLE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
    };
    use windows_sys::Win32::System::IO::{CancelIo, GetOverlappedResult, OVERLAPPED};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    use windows_sys::Win32::System::Threading::{
        CreateEventW, GetExitCodeProcess, INFINITE, ResetEvent, WaitForMultipleObjects,
    };
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    use super::{HOME, INSTALL, Report, SERVE, Task, listen, send};

    /// How much the pipe holds before a writer has to wait for a reader.
    ///
    /// A hint rather than a limit - Windows grows the buffer as it needs to -
    /// but a job carrying a few documents crosses in one write if this is
    /// roomy, and the documents are the only large thing that crosses.
    const PIPE_BUFFER: u32 = 256 * 1024;

    /// Carry the job to an elevated copy of this application.
    pub fn run(
        task: &Task,
        install: &Installation,
        home: &OrngHome,
        say: &impl Fn(Report),
    ) -> Result<Manifest, String> {
        // Named before the child is launched, because the name is what the
        // child is told to call back on.
        let name = pipe_name();
        let pipe = Pipe::create(&name)?;
        // The connect is armed before the child starts. Armed after, a child
        // quick enough to call back first would find nothing listening.
        let connecting = pipe.arm_connect()?;

        let child = launch(&name, install, home)?;

        // Whichever happens first: the child calls back, or it dies without
        // doing so. Waiting only on the connect is what would hang the run for
        // ever on a child that failed before it opened the pipe.
        //
        // Every way out from here but the first takes the connect back before
        // it returns: it is still in flight, and the kernel holds a pointer
        // into `connecting` until it is told to stop.
        let ready = unsafe {
            WaitForMultipleObjects(2, [connecting.event(), child.handle()].as_ptr(), 0, INFINITE)
        };
        if ready == WAIT_OBJECT_0 + 1 {
            connecting.abandon(&pipe);
            return Err(child.why_it_gave_up());
        }
        if ready != WAIT_OBJECT_0 {
            // Neither handle: the wait itself failed. The child may well still
            // be running, and still be writing into the installation, so this
            // must not be reported as a process that ended - its exit code
            // would read `STILL_ACTIVE` and be drawn as the reason it stopped.
            connecting.abandon(&pipe);
            return Err(format!("could not wait for an elevated run: {}", last()));
        }
        connecting.finish(&pipe)?;

        let mut channel = pipe.stream()?;
        send(&mut channel, task)
            .map_err(|e| format!("the elevated run could not be told: {e}"))?;
        listen(channel, say)
    }

    /// A name no other process is using, and that none could have prepared.
    ///
    /// The randomness is what makes squatting the name a guess rather than a
    /// race: a process running as this user could otherwise create the pipe
    /// first and feed the elevated child a job of its own. `FILE_FLAG_FIRST_
    /// PIPE_INSTANCE` is the other half - if the name somehow is taken, the
    /// create fails and nothing is launched at all.
    fn pipe_name() -> String {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).expect("the system has no randomness");
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        format!(r"\\.\pipe\orng-registry-{}-{hex}", std::process::id())
    }

    /// A UTF-16 string with the terminator Windows expects.
    ///
    /// Takes an `OsStr` rather than a `&str` so that a path goes across as the
    /// UTF-16 Windows gave it. `to_string_lossy` would rewrite an unpaired
    /// surrogate - legal in a Windows path - as U+FFFD, and the binary or the
    /// installation named after that is one that does not exist.
    fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
        text.as_ref().encode_wide().chain(std::iter::once(0)).collect()
    }

    /// The pipe this process listens on, closed when it goes out of scope.
    struct Pipe(OwnedHandle);

    impl Pipe {
        fn create(name: &str) -> Result<Pipe, String> {
            let handle = unsafe {
                CreateNamedPipeW(
                    wide(name).as_ptr(),
                    PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    // One instance. A second caller on this name is not a
                    // second child of ours; there is only ever one.
                    1,
                    PIPE_BUFFER,
                    PIPE_BUFFER,
                    0,
                    std::ptr::null_mut(),
                )
            };
            if handle.is_null() || handle == -1isize as HANDLE {
                return Err(format!("could not open a channel to an elevated run: {}", last()));
            }
            Ok(Pipe(unsafe { OwnedHandle::from_raw_handle(handle) }))
        }

        fn raw(&self) -> HANDLE {
            use std::os::windows::io::AsRawHandle;
            self.0.as_raw_handle()
        }

        /// Start listening, and answer with the thing to wait on.
        fn arm_connect(&self) -> Result<Connecting, String> {
            let connecting = Connecting::new()?;
            let pending = unsafe { ConnectNamedPipe(self.raw(), connecting.overlapped()) };
            // Overlapped, so the expected answer is "not yet". Anything else is
            // either a client already there or a genuine failure.
            if pending == 0 && unsafe { GetLastError() } != ERROR_IO_PENDING {
                return Err(format!("could not listen for an elevated run: {}", last()));
            }
            Ok(connecting)
        }

        /// The pipe as something that reads and writes.
        ///
        /// Fallible because the stream keeps one transfer structure for its
        /// whole life rather than building one per call, and creating it is a
        /// call that can fail.
        fn stream(&self) -> Result<Stream<'_>, String> {
            Ok(Stream { pipe: self, transfer: Connecting::new()? })
        }
    }

    /// A connect in flight, and the event that says it landed.
    struct Connecting {
        event: OwnedHandle,
        overlapped: Box<OVERLAPPED>,
    }

    impl Connecting {
        fn new() -> Result<Connecting, String> {
            // Manual reset, unsignalled, unnamed. Manual because it is waited on
            // and then read, and an auto-reset event would be clear by then.
            let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
            if event.is_null() {
                return Err(format!("could not wait for an elevated run: {}", last()));
            }
            let event = unsafe { OwnedHandle::from_raw_handle(event) };
            let mut overlapped: Box<OVERLAPPED> = Box::new(unsafe { std::mem::zeroed() });
            overlapped.hEvent = raw_of(&event);
            Ok(Connecting { event, overlapped })
        }

        fn event(&self) -> HANDLE {
            raw_of(&self.event)
        }

        fn overlapped(&self) -> *mut OVERLAPPED {
            // Cast away the shared borrow: the kernel writes into this
            // structure for as long as the operation is in flight, which is
            // why it is boxed and why `Connecting` owns it until the wait is
            // over.
            std::ptr::from_ref(self.overlapped.as_ref()).cast_mut()
        }

        /// Collect the result of the connect the wait said had landed.
        fn finish(&self, pipe: &Pipe) -> Result<(), String> {
            let mut moved = 0u32;
            let ok = unsafe {
                GetOverlappedResult(pipe.raw(), self.overlapped(), &mut moved, 0)
            };
            if ok == 0 {
                return Err(format!("an elevated run never called back: {}", last()));
            }
            Ok(())
        }

        /// Take back an operation that is never going to land.
        ///
        /// **Not optional, and not tidiness.** The kernel writes status into
        /// this structure and signals this event whenever the operation ends,
        /// and both are freed the moment `self` is dropped. Returning from a
        /// failed wait without this leaves an in-flight `ConnectNamedPipe`
        /// pointing at a box that is about to go, and closing the pipe a moment
        /// later is what makes it end - into freed memory.
        ///
        /// Waited for rather than checked: a cancelled operation reports
        /// failure, which is the expected answer. What is needed is that it has
        /// finished, not how.
        fn abandon(&self, pipe: &Pipe) {
            let mut moved = 0u32;
            unsafe {
                CancelIo(pipe.raw());
                GetOverlappedResult(pipe.raw(), self.overlapped(), &mut moved, 1);
            }
        }

        /// Put it back, ready for another transfer over the same handle.
        ///
        /// The structure carries the last transfer's status and the event is
        /// still signalled from it, so both have to be cleared before the next
        /// one starts - otherwise a wait would return immediately on the answer
        /// before it.
        fn reset(&mut self) {
            let event = raw_of(&self.event);
            *self.overlapped = unsafe { std::mem::zeroed() };
            self.overlapped.hEvent = event;
            unsafe { ResetEvent(event) };
        }
    }

    /// The pipe, as `std::io`.
    ///
    /// The handle is overlapped, so neither `ReadFile` nor `WriteFile` may be
    /// left to finish by itself: each is started with a structure and then
    /// waited for. That is what `std::fs::File` would not do - it assumes a
    /// synchronous handle and would report a transfer of nothing every time.
    struct Stream<'a> {
        pipe: &'a Pipe,
        /// The one structure every transfer here runs through, put back between
        /// them by [`Connecting::reset`].
        ///
        /// One rather than one per call: a job with documents in it crosses in
        /// several writes and the reply comes back in `BufReader`-sized reads,
        /// and building a structure and a kernel event for each was an event
        /// created and closed per few kilobytes. Only ever one is in flight,
        /// because both `Read` and `Write` take `&mut self`.
        transfer: Connecting,
    }

    impl Stream<'_> {
        /// Run one overlapped transfer to completion.
        fn transfer(
            &mut self,
            start: impl FnOnce(HANDLE, *mut OVERLAPPED) -> i32,
        ) -> std::io::Result<usize> {
            self.transfer.reset();
            let started = start(self.pipe.raw(), self.transfer.overlapped());
            if started == 0 && unsafe { GetLastError() } != ERROR_IO_PENDING {
                return Err(std::io::Error::last_os_error());
            }
            let mut moved = 0u32;
            // Wait for it: the last argument is what says "block until done".
            let ok = unsafe {
                GetOverlappedResult(self.pipe.raw(), self.transfer.overlapped(), &mut moved, 1)
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(moved as usize)
        }
    }

    impl Read for Stream<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            use windows_sys::Win32::Storage::FileSystem::ReadFile;
            let length = buffer.len().min(u32::MAX as usize) as u32;
            let at = buffer.as_mut_ptr();
            match self.transfer(|handle, overlapped| unsafe {
                ReadFile(handle, at, length, std::ptr::null_mut(), overlapped)
            }) {
                Ok(moved) => Ok(moved),
                // The child finished and closed its end, which is how a
                // conversation ends rather than how one fails.
                Err(e) if is_end_of_pipe(&e) => Ok(0),
                Err(e) => Err(e),
            }
        }
    }

    impl Write for Stream<'_> {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            use windows_sys::Win32::Storage::FileSystem::WriteFile;
            let length = buffer.len().min(u32::MAX as usize) as u32;
            let at = buffer.as_ptr();
            self.transfer(|handle, overlapped| unsafe {
                WriteFile(handle, at, length, std::ptr::null_mut(), overlapped)
            })
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Whether this is the other end having gone away in good order.
    fn is_end_of_pipe(error: &std::io::Error) -> bool {
        use windows_sys::Win32::Foundation::{ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED};
        matches!(
            error.raw_os_error().map(|code| code as u32),
            Some(ERROR_BROKEN_PIPE) | Some(ERROR_PIPE_NOT_CONNECTED)
        )
    }

    /// The elevated child, and the handle that says when it is gone.
    struct Child(OwnedHandle);

    impl Child {
        fn handle(&self) -> HANDLE {
            raw_of(&self.0)
        }

        /// Why a child that exited before calling back gave up.
        ///
        /// Its exit code rather than a guess. A child that failed to parse its
        /// own arguments, or that could not open the pipe, has already gone by
        /// the time this is asked, and the code is all that is left of it.
        fn why_it_gave_up(&self) -> String {
            let mut code = 0u32;
            let read = unsafe { GetExitCodeProcess(self.handle(), &mut code) };
            if read == 0 {
                return "the elevated run ended before it said anything".to_owned();
            }
            format!("the elevated run ended before it said anything (exit code {code})")
        }
    }

    /// Ask Windows for a copy of this application that holds the rights.
    ///
    /// `runas` is the verb that raises the consent dialog. The dialog is the
    /// system's own and cannot be drawn, suppressed or answered from here,
    /// which is the property that makes it worth anything.
    fn launch(pipe: &str, install: &Installation, home: &OrngHome) -> Result<Child, String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("this application cannot find its own binary: {e}"))?;
        // Built as an `OsString` and never through `format!`: two of the three
        // values are paths, and `Display` on a path is lossy. Quoted, because a
        // pipe name, an installation path and a home may all hold spaces and
        // this is one string by the time Windows reads it.
        let mut arguments = OsString::new();
        for (flag, value) in [
            (SERVE, OsStr::new(pipe)),
            (INSTALL, install.root().as_os_str()),
            (HOME, home.user_home().as_os_str()),
        ] {
            if !arguments.is_empty() {
                arguments.push(" ");
            }
            arguments.push(flag);
            arguments.push(" \"");
            arguments.push(value);
            arguments.push("\"");
        }

        let verb = wide("runas");
        let file = wide(&exe);
        let parameters = wide(&arguments);
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
        // The process handle is the thing that has to come back: it is what
        // says the child died, and waiting on it is how this avoids polling.
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        // The child draws nothing. Everything the user sees is this window.
        info.nShow = SW_HIDE;

        let started = unsafe { ShellExecuteExW(&mut info) };
        if started == 0 {
            let code = unsafe { GetLastError() };
            if code == ERROR_CANCELLED {
                // Not a failure of anything. The user was asked and said no,
                // and nothing has been written.
                return Err("administrator rights were declined, so nothing was changed"
                    .to_owned());
            }
            return Err(format!("could not ask for administrator rights: {}", last()));
        }
        if info.hProcess.is_null() {
            return Err("Windows granted the rights but started nothing".to_owned());
        }
        Ok(Child(unsafe { OwnedHandle::from_raw_handle(info.hProcess) }))
    }

    fn raw_of(handle: &OwnedHandle) -> HANDLE {
        use std::os::windows::io::AsRawHandle;
        handle.as_raw_handle()
    }

    /// The last error, as a sentence rather than a number.
    fn last() -> String {
        std::io::Error::last_os_error().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A whole job crosses and comes back as itself.
    ///
    /// The rows especially: they cross in the entry list's own format, which is
    /// the claim this module makes by reusing it rather than inventing a
    /// per-row representation. A row that lost a column on the way would place
    /// a document under a registration that no longer described it.
    #[test]
    fn a_job_comes_back_off_the_wire_as_it_went() {
        let job = a_job();
        let line = serde_json::to_string(&job).expect("a job did not serialise");
        let back: Job = serde_json::from_str(&line).expect("a job did not come back");

        assert_eq!(back, job, "a job did not survive the wire");
    }

    /// And it describes the same update on the other side.
    ///
    /// This is the property the whole module rests on: one press, described
    /// once, and the same work whether it runs here or in a child. The rows the
    /// update will write are what a caller can ask it for, so they are what is
    /// held against the job.
    #[test]
    fn a_job_that_crossed_describes_the_same_update() {
        let job = a_job();
        let line = serde_json::to_string(&job).expect("a job did not serialise");
        let back: Job = serde_json::from_str(&line).expect("a job did not come back");

        let here = job.update().expect("the job does not describe an update");
        let there = back.update().expect("the job that crossed does not describe one");
        assert_eq!(
            there.writing().collect::<Vec<_>>(),
            here.writing().collect::<Vec<_>>(),
            "the job wrote different rows after crossing"
        );
        assert_eq!(there.entries(), here.entries(), "the job left a different list behind");
    }

    /// Bytes that are not the kind of document the row claims are refused
    /// before anything is placed.
    ///
    /// The child is the elevated one, so what it is handed is what it has to
    /// distrust: these bytes end up written into a library under an identity
    /// the same message chose.
    #[test]
    fn a_placing_whose_bytes_are_not_a_document_is_refused() {
        let mut job = a_job();
        job.documents[0].bytes = b"not a Bitwig document".to_vec();

        job.update().expect_err("bytes that are not a document were placed anyway");
    }

    /// And a kind that is not one at all, which is the other half of the same
    /// message being untrusted.
    ///
    /// Refused as the message is read rather than when the document is built:
    /// the kind is a [`Kind`] on both sides of the wire now, so there is no
    /// point after this at which a job could be holding a kind that is not one.
    #[test]
    fn a_placing_whose_kind_is_not_one_is_refused() {
        let line = serde_json::to_string(&a_job()).expect("a job did not serialise");
        let tampered = line.replace("\"bwdevice\"", "\"bwexploit\"");
        assert_ne!(tampered, line, "the kind this test rewrites is not in a job any more");

        serde_json::from_str::<Job>(&tampered)
            .expect_err("a document of no known kind crossed anyway");
    }

    /// And so is a removal that says something other than what becomes of a
    /// document, for the same reason: the answer decides whether work the user
    /// cannot get back survives.
    #[test]
    fn a_removal_that_does_not_say_what_becomes_of_the_document_is_refused() {
        let mut job = a_job();
        job.remove(Uuid::new_v4(), TheDocument::Deleted);
        let line = serde_json::to_string(&job).expect("a job did not serialise");
        let tampered = line.replace("\"deleted\"", "\"maybe\"");
        assert_ne!(tampered, line, "the outcome this test rewrites is not in a job any more");

        serde_json::from_str::<Job>(&tampered).expect_err("a removal of no known kind crossed");
    }

    /// Both tasks cross, and cross as different things.
    ///
    /// The two presses reach the same directories and are told to the child
    /// down the same pipe, so what keeps them apart is only this enum. One that
    /// read back as the other would put a backup over an installation somebody
    /// asked to register a device into.
    #[test]
    fn both_tasks_cross_and_stay_apart() {
        let tasks = [
            Task::Apply(a_job()),
            Task::Restore { from: PathBuf::from("/home/someone/.orng/backups/5.1.9") },
        ];
        for task in &tasks {
            let line = serde_json::to_string(task).expect("a task did not serialise");
            let back: Task = serde_json::from_str(&line).expect("a task did not come back");
            assert_eq!(&back, task, "a task did not survive the wire");
        }
        assert_ne!(tasks[0], tasks[1], "the two tasks are the same value");
    }

    /// A whole conversation, read as the window reads it.
    ///
    /// The order matters as much as the content: the progress the dialog draws
    /// is a sequence, and a report arriving out of turn would mark the wrong
    /// row running.
    #[test]
    fn a_run_that_went_well_is_read_back_in_order() {
        let said = std::cell::RefCell::new(Vec::new());
        let entries = Manifest::default().to_tsv();
        let transcript = transcript(&[
            Report::Planned(vec![WireStep::Backup, WireStep::Patch]),
            Report::Began(WireStep::Backup),
            Report::Began(WireStep::Patch),
            Report::Registering,
            Report::Finished(Ok(entries)),
        ]);

        let list = listen(transcript.as_bytes(), &|report| said.borrow_mut().push(report))
            .expect("a run that went well was read as a failure");

        assert!(list.is_empty(), "a run that wrote nothing came back with rows");
        assert_eq!(
            said.into_inner(),
            [
                Report::Planned(vec![WireStep::Backup, WireStep::Patch]),
                Report::Began(WireStep::Backup),
                Report::Began(WireStep::Patch),
                Report::Registering,
            ],
            "the progress was not read back as it was said"
        );
    }

    /// How it ended is the call's answer and never the callback's.
    ///
    /// A caller that had to watch for `Finished` among the progress could
    /// forget to, and then a failed run would draw as one still going.
    #[test]
    fn the_ending_does_not_arrive_as_progress() {
        let said = std::cell::RefCell::new(Vec::new());
        let transcript = transcript(&[Report::Finished(Err("the archive did not verify".into()))]);

        let why = listen(transcript.as_bytes(), &|report| said.borrow_mut().push(report))
            .expect_err("a run that failed was read as a success");

        assert_eq!(why, "the archive did not verify");
        assert!(said.into_inner().is_empty(), "the ending was reported as progress as well");
    }

    /// A child that stopped without saying how it went is a failure.
    ///
    /// The same rule `Applying::poll` holds for a worker that vanished, and for
    /// the same reason: silence read as success would show a finished run over
    /// an installation nothing was done to.
    #[test]
    fn a_run_that_stopped_without_reporting_is_not_a_success() {
        let transcript = transcript(&[Report::Began(WireStep::Patch)]);

        let why = listen(transcript.as_bytes(), &|_| {})
            .expect_err("a run that never reported was read as a success");

        assert!(
            why.contains("without reporting"),
            "a run that stopped silently was blamed on something else: {why}"
        );
    }

    /// And one that said something unreadable is a failure too, rather than
    /// being skipped on the way to an ending that never comes.
    #[test]
    fn a_run_that_said_something_unreadable_is_not_a_success() {
        listen("{not json at all}\n".as_bytes(), &|_| {})
            .expect_err("an unreadable report was read as a success");
    }

    /// The child's orders, and the fact that an ordinary launch has none.
    #[test]
    fn only_a_child_reads_orders_off_the_command_line() {
        let window = ["orng-registry".to_owned()];
        assert_eq!(
            Serving::from_arguments(window.into_iter()),
            None,
            "the window was mistaken for an elevated child"
        );

        let serving = Serving::from_arguments(orders().into_iter())
            .expect("a child did not read its orders");
        assert_eq!(serving.pipe, r"\\.\pipe\orng-registry-1-abc");
        assert_eq!(serving.install, PathBuf::from(r"C:\Program Files\Bitwig Studio"));
        // The home the *window* resolved, and not one this process could have
        // looked up: an elevated child may be running as an administrator whose
        // profile is not the one Bitwig will read.
        assert_eq!(serving.home, PathBuf::from(r"C:\Users\someone"));
    }

    /// Anything short of all three is not orders.
    ///
    /// A child told where to call back but not what to work on would have to
    /// discover an installation for itself, and one told nothing about a home
    /// would take its own - both of which are what the module exists to stop.
    #[test]
    fn part_of_the_orders_is_not_orders() {
        for missing in [SERVE, INSTALL, HOME] {
            let mut short = orders();
            let flag = short.iter().position(|argument| argument == missing).expect("the flag");
            // The flag and the value behind it, which is how it is given.
            short.drain(flag..=flag + 1);
            assert_eq!(
                Serving::from_arguments(short.into_iter()),
                None,
                "a child with no {missing} went looking for one"
            );
        }
    }

    /// A child's whole command line, as the window builds it.
    fn orders() -> Vec<String> {
        vec![
            "orng-registry".to_owned(),
            SERVE.to_owned(),
            r"\\.\pipe\orng-registry-1-abc".to_owned(),
            INSTALL.to_owned(),
            r"C:\Program Files\Bitwig Studio".to_owned(),
            HOME.to_owned(),
            r"C:\Users\someone".to_owned(),
        ]
    }

    /// A job the child cannot read is reported rather than died on.
    ///
    /// The child has no interface, so a failure it does not put on the wire is
    /// a failure the window can only describe as silence.
    #[test]
    fn a_job_that_does_not_read_is_reported_back() {
        let root = tempfile::tempdir().expect("somewhere to put an installation");
        let install = orng_tools::testing::install(root.path());
        let mut wire = Vec::new();

        serve("{not a job}\n".as_bytes(), &mut wire, install, OrngHome::at(root.path()))
            .expect("the child could not report");

        let line = String::from_utf8(wire).expect("the child said something that is not text");
        let report: Report =
            serde_json::from_str(line.trim_end()).expect("the child said something unreadable");
        assert!(
            matches!(report, Report::Finished(Err(why)) if why.contains("did not read")),
            "a job that does not read was not reported as such"
        );
    }

    /// A child that dies before it calls back ends the run instead of hanging
    /// it.
    ///
    /// The one failure the parent's own Windows code can be held to from a
    /// test, and the one most worth holding it to: the connect blocks until
    /// somebody opens the pipe, so waiting on it alone would wait for ever on a
    /// child that never got that far. What proves it is that `current_exe` here
    /// is the test binary rather than the application - it is started with
    /// arguments libtest does not understand, so it exits without ever opening
    /// the pipe, which is exactly the case being tested.
    ///
    /// **Ignored, because it starts a process and asks for rights.** On a
    /// machine whose session is already elevated `runas` is silent; anywhere
    /// else it raises the consent dialog, and a suite must not. `--ignored`
    /// runs it, as it does the two that need the network.
    #[cfg(windows)]
    #[ignore = "starts a process and asks for administrator rights"]
    #[test]
    fn a_child_that_never_calls_back_ends_the_run_rather_than_hanging_it() {
        let root = tempfile::tempdir().expect("somewhere to put an installation");
        let install = orng_tools::testing::install(root.path());

        let why = run(&Task::Apply(a_job()), &install, &OrngHome::at(root.path()), &|_| {})
            .expect_err("a child that never called back was read as a success");

        // The exact answer on a machine where `runas` is silent. A consent
        // dialog that was dismissed instead says `declined`, which is a
        // different outcome and should read as a failure of this test rather
        // than be swallowed by it.
        assert!(
            why.contains("before it said anything"),
            "a child that died before calling back was blamed on something else: {why}"
        );
    }

    /// Reports as they arrive down a pipe: one per line.
    fn transcript(reports: &[Report]) -> String {
        let mut wire = Vec::new();
        for report in reports {
            send(&mut wire, report).expect("a report did not send");
        }
        String::from_utf8(wire).expect("a report is not text")
    }

    /// A job with one document to add, which is the shape every other test
    /// here bends out of true.
    fn a_job() -> Job {
        let document = a_document();
        let registration = Registration::from_document(&document, "Breath Follower.bwdevice")
            .expect("a document this crate built is not registrable");

        let mut job = Job {
            work: Work::Entries,
            library: PathBuf::from("/somewhere/Bitwig Studio"),
            placement: Strategy::Link,
            base: Manifest::default(),
            written: Manifest::default(),
            documents: Vec::new(),
            removed: Vec::new(),
        };
        job.add(registration, &document);
        job
    }

    fn a_document() -> Document {
        orng_tools::testing::document(
            Kind::Device,
            Uuid::parse_str("11111111-2222-3333-4444-555555555555").expect("a uuid"),
            "Breath Follower",
        )
    }

    /// A report crosses as itself, which is the whole of what the protocol
    /// promises. Every variant, because the one that is skipped is the one that
    /// breaks: `Finished` is what the window waits for.
    #[test]
    fn every_report_survives_the_wire() {
        let reports = [
            Report::Planned(vec![WireStep::Backup, WireStep::Link]),
            Report::Began(WireStep::Verify),
            Report::Registering,
            Report::Finished(Ok("#orng-registry 4\n".to_owned())),
            Report::Finished(Err("the archive did not verify".to_owned())),
        ];
        for report in reports {
            let mut wire = Vec::new();
            send(&mut wire, &report).expect("a report did not send");
            let line = String::from_utf8(wire).expect("a report is not text");
            let back: Report = serde_json::from_str(line.trim_end())
                .expect("a report did not come back off the wire");
            assert_eq!(back, report);
        }
    }

    /// Each report is one line, which is what the reader on the other end
    /// relies on to find where one ends and the next begins.
    #[test]
    fn a_report_is_one_line() {
        let mut wire = Vec::new();
        send(&mut wire, &Report::Finished(Ok("#orng-registry 4\nrow\twith\ttabs\n".to_owned())))
            .expect("a report did not send");

        assert_eq!(
            wire.iter().filter(|byte| **byte == b'\n').count(),
            1,
            "a report carrying a multi-line entry list crossed as more than one line"
        );
    }

    /// Every step crosses and comes back as the same step.
    ///
    /// The mapping is by hand on both sides, so nothing but a test says the two
    /// halves agree - and a step that crossed as another step would draw the
    /// wrong row of the progress list as running.
    #[test]
    fn every_step_crosses_as_itself() {
        for step in orng_tools::Step::ALL {
            let there: WireStep = step.into();
            let back: orng_tools::Step = there.into();
            assert_eq!(back, step, "{step:?} did not survive crossing");
        }
    }
}
