// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tool that guards the ORNG Catalog repository.
//!
//! Every rule it applies lives in `orng-catalog`, which is also what the
//! application reads the catalog with. One implementation, so the repository and
//! the installer cannot disagree about what a valid item is.
//!
//! Two commands take a file explicitly rather than finding it in the checkout,
//! and that is the point of them:
//!
//! - `owners` is given the owners file, because it must be the one on the base
//!   branch. The copy in a pull request is written by the contributor being
//!   checked, who could add themselves to it in the commit under review.
//! - `check --against` is given the last published index, because permanence is
//!   a promise about what was published, not about what the branch says now.
//!
//! Making the caller name both files keeps that decision where it can be read,
//! in a workflow, rather than buried in a default.
//!
//! The signing key is the exception to that: it arrives in an environment
//! variable and there is deliberately no flag for it. A file path on a command
//! line is recorded in shell history and echoed by a workflow log, and a secret
//! that can be named there eventually is.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use orng_catalog::{
    Authorization, History, Index, Item, Owners, PublicKey, Revision, SecretKey, Severity,
    Signature, authorize, scan, validate,
};

/// Where the signing key is read from, and the only place it is read from.
const SIGNING_KEY: &str = "ORNG_CATALOG_SIGNING_KEY";

/// Outcomes, as a process exit status. A workflow branches on these, so they are
/// part of the interface and not an afterthought.
mod exit {
    /// Nothing to report.
    pub const OK: u8 = 0;
    /// The tree, the pull request, or the index and signature it was given, is
    /// not acceptable.
    pub const REJECTED: u8 = 1;
    /// The tool could not run: a missing file, an unreadable checkout, a key it
    /// cannot make sense of. A verdict on the caller's arguments rather than on
    /// what was being checked.
    pub const FAILED: u8 = 2;
    /// Allowed as far as ownership goes, but a human has to decide.
    pub const REVIEW: u8 = 3;
}

#[derive(Parser)]
#[command(
    name = "orng-catalog-lint",
    about = "Validates an ORNG Catalog checkout and generates its index",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check a checkout against itself, and optionally against what is published.
    Check {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// The last published index. Without it only internal rules run, because
        /// there is no history to break.
        #[arg(long, value_name = "INDEX.JSON")]
        against: Option<PathBuf>,
    },
    /// Generate the index.
    Index {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Where to write it. Standard output if absent.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Commit the index was generated from. For a caller holding a tree that
        /// is not a checkout; `--from-git` reads it from the checkout instead.
        #[arg(long, value_name = "SHA")]
        revision: Option<String>,
        /// Read the revisions out of the checkout at `--root`: its `HEAD` for the
        /// index, and for each item the change that last touched its directory.
        ///
        /// Off by default, so the command stays a projection of the tree and
        /// needs no git. Continuous integration turns it on after a merge, which
        /// is the only moment a per-item answer exists.
        #[arg(long, conflicts_with = "revision")]
        from_git: bool,
    },
    /// Sign an index, so that serving it is not the same as authoring it.
    ///
    /// The signature is detached and published as a second release asset: the
    /// bytes signed here are byte for byte the bytes the application downloads,
    /// with no canonicalisation between them, and `index.json` itself is
    /// unchanged for anything that reads it without checking.
    Sign {
        /// The index to sign, exactly as it will be served.
        #[arg(long, value_name = "INDEX.JSON")]
        index: PathBuf,
        /// Where to write the detached signature. Standard output if absent.
        #[arg(long, value_name = "INDEX.JSON.SIG")]
        out: Option<PathBuf>,
    },
    /// Check an index against its signature, the way the application will.
    Verify {
        /// The index as served, byte for byte.
        #[arg(long, value_name = "INDEX.JSON")]
        index: PathBuf,
        /// The detached signature published beside it.
        #[arg(long, value_name = "INDEX.JSON.SIG")]
        signature: PathBuf,
        /// The catalog's public key, as hex. A flag rather than an environment
        /// variable because it is public, and a workflow that states which key
        /// it trusts says something worth reading in the log.
        #[arg(long, value_name = "HEX")]
        public_key: String,
    },
    /// Make a signing key, for a human setting the catalog up once.
    ///
    /// Prints the secret half to standard output, so run it on a machine you
    /// trust and paste the result straight into the repository secret. Never in
    /// continuous integration: whatever it printed there would be in the log.
    Keygen,
    /// Decide whether an account may change these paths.
    Owners {
        /// The owners file **as it exists on the base branch**.
        #[arg(long, value_name = "OWNERS.TOML")]
        owners: PathBuf,
        /// Numeric GitHub account id of whoever opened the pull request. Numeric
        /// because a login can be changed by its holder and the freed name
        /// claimed by somebody else.
        #[arg(long, value_name = "ID")]
        actor_id: u64,
        /// Every path the pull request touches, both sides of any rename.
        #[arg(value_name = "PATH", required = true)]
        changed: Vec<String>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Check { root, against } => check(&root, against.as_deref()),
        Command::Index { root, out, revision, from_git } => {
            index(&root, out.as_deref(), revision, from_git)
        }
        Command::Sign { index, out } => sign(&index, out.as_deref()),
        Command::Verify { index, signature, public_key } => {
            verify(&index, &signature, &public_key)
        }
        Command::Keygen => keygen(),
        Command::Owners { owners, actor_id, changed } => {
            ownership(&owners, actor_id, &changed)
        }
    }
}

fn check(root: &Path, against: Option<&Path>) -> ExitCode {
    let (items, failures) = scan(root);
    for failure in &failures {
        eprintln!("{failure}");
    }

    let mut report = validate::check(&items);
    if let Some(path) = against {
        let published = match read_index(path) {
            Ok(index) => index,
            Err(code) => return code,
        };
        report.problems.extend(validate::check_against(&items, &published).problems);
    }

    for problem in &report.problems {
        let label = match problem.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        println!("{label}: {problem}");
    }

    let errors = report.problems.iter().filter(|p| p.severity() == Severity::Error).count();
    println!(
        "{} item{}, {errors} error{}, {} warning{}",
        items.len(),
        plural(items.len()),
        plural(errors),
        report.problems.len() - errors,
        plural(report.problems.len() - errors),
    );

    // A failure to read an item is not a rule violation, but it is still a tree
    // that cannot be published, so it has to fail the run.
    if !failures.is_empty() {
        return ExitCode::from(exit::FAILED);
    }
    match errors {
        0 => ExitCode::from(exit::OK),
        _ => ExitCode::from(exit::REJECTED),
    }
}

fn index(root: &Path, out: Option<&Path>, revision: Option<String>, from_git: bool) -> ExitCode {
    let (items, failures) = scan(root);
    if !failures.is_empty() {
        for failure in &failures {
            eprintln!("{failure}");
        }
        eprintln!("refusing to generate an index from a tree that does not read");
        return ExitCode::from(exit::FAILED);
    }

    let history = match history(root, &items, revision, from_git) {
        Ok(history) => history,
        Err(code) => return code,
    };
    let json = Index::build(&items, &history).to_json();
    match out {
        None => println!("{json}"),
        Some(path) => {
            if let Err(e) = std::fs::write(path, format!("{json}\n")) {
                eprintln!("{}: {e}", path.display());
                return ExitCode::from(exit::FAILED);
            }
            eprintln!("{} item{} written to {}", items.len(), plural(items.len()), path.display());
        }
    }
    ExitCode::from(exit::OK)
}

/// Assemble what the tree cannot say about itself.
///
/// The library takes history as data and never runs git, because the
/// application links that library too and must not need a git client. Knowing
/// that this tool runs inside a checkout is the binary's business.
fn history(
    root: &Path,
    items: &[Item],
    revision: Option<String>,
    from_git: bool,
) -> Result<History, ExitCode> {
    if let Some(text) = revision {
        return Ok(History::at(parse_revision(&text)?));
    }
    if !from_git {
        return Ok(History::default());
    }

    let mut history = History::at(parse_revision(&git(root, &["rev-parse", "HEAD"])?)?);
    for item in items {
        let dir = item.dir();
        // `--first-parent` is what makes this the *merging* commit. Without it
        // git answers with the contributor's own commit from inside the branch,
        // which nobody reviewed and which is not on the published history at
        // all. Squash merges give the same answer either way.
        let found = git(root, &["log", "-1", "--first-parent", "--format=%H", "--", &dir])?;
        // Empty when the path has never been committed, which is the normal
        // state of an item still sitting in a working tree.
        if !found.is_empty() {
            history.record(dir, parse_revision(&found)?);
        }
    }
    Ok(history)
}

fn git(root: &Path, args: &[&str]) -> Result<String, ExitCode> {
    let output =
        std::process::Command::new("git").arg("-C").arg(root).args(args).output().map_err(|e| {
            eprintln!("could not run git: {e}");
            ExitCode::from(exit::FAILED)
        })?;
    if !output.status.success() {
        eprintln!("git {}: {}", args.join(" "), String::from_utf8_lossy(&output.stderr).trim());
        return Err(ExitCode::from(exit::FAILED));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn parse_revision(text: &str) -> Result<Revision, ExitCode> {
    Revision::new(text).map_err(|e| {
        eprintln!("{e}");
        ExitCode::from(exit::FAILED)
    })
}

/// Sign the index that is on disk, rather than one generated here.
///
/// Regenerating it would sign a file the release never carried: the index the
/// workflow publishes is the one it built a step earlier, with whatever
/// revisions git gave it, and this has to cover those bytes and no others.
fn sign(index: &Path, out: Option<&Path>) -> ExitCode {
    let key = match signing_key() {
        Ok(key) => key,
        Err(code) => return code,
    };
    let bytes = match read_file(index) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };

    let signature = key.sign(&bytes);
    match out {
        None => println!("{signature}"),
        Some(path) => {
            if let Err(e) = std::fs::write(path, format!("{signature}\n")) {
                eprintln!("{}: {e}", path.display());
                return ExitCode::from(exit::FAILED);
            }
            eprintln!("{} signed into {}", index.display(), path.display());
        }
    }
    // Which key signed is public, and a workflow log is the right place for it:
    // it is how anyone can tell a rotation from a compromise after the fact.
    eprintln!("signed by {}", key.public_key());
    ExitCode::from(exit::OK)
}

/// Check a published pair the way the application will.
///
/// Deliberately goes through `Index::verified`, the same call the application
/// makes, so that this command cannot pass on a file the application would
/// refuse.
fn verify(index: &Path, signature: &Path, public_key: &str) -> ExitCode {
    // An unusable key is the operator's own input, not a verdict on the files,
    // so it fails the run rather than rejecting the index.
    let key = match PublicKey::from_hex(public_key.trim()) {
        Ok(key) => key,
        Err(e) => {
            eprintln!("--public-key: {e}");
            return ExitCode::from(exit::FAILED);
        }
    };
    let (bytes, signature_text) = match (read_file(index), read_file(signature)) {
        (Ok(bytes), Ok(text)) => (bytes, text),
        (Err(code), _) | (_, Err(code)) => return code,
    };

    // From here on everything is a verdict on the two files, which is what
    // rejection means. A signature that does not parse is as unusable as one
    // that does not match. Read lossily because the parse is the real check:
    // whatever a replacement character stands in for was not a hex digit.
    let signature = match Signature::parse(&String::from_utf8_lossy(&signature_text)) {
        Ok(signature) => signature,
        Err(e) => {
            println!("refused: {e}");
            return ExitCode::from(exit::REJECTED);
        }
    };

    match Index::verified(&bytes, &signature, &key) {
        Ok(index) => {
            println!(
                "verified: {} item{} signed by {key}",
                index.items.len(),
                plural(index.items.len())
            );
            ExitCode::from(exit::OK)
        }
        Err(e) => {
            println!("refused: {e}");
            ExitCode::from(exit::REJECTED)
        }
    }
}

/// Make the catalog's key pair. Run once, by a human, on a machine they trust.
fn keygen() -> ExitCode {
    let key = SecretKey::generate();
    println!("signing key, secret, for the {SIGNING_KEY} repository secret:");
    println!("  {}", key.to_hex());
    println!("public key, for the application and the release notes:");
    println!("  {}", key.public_key());
    ExitCode::from(exit::OK)
}

/// The signing key, from the environment and from nowhere else.
///
/// No flag takes its place. A path on a command line survives in shell history
/// and in the log of whatever ran it, and a secret that can be named there
/// eventually is.
fn signing_key() -> Result<SecretKey, ExitCode> {
    let Ok(text) = std::env::var(SIGNING_KEY) else {
        eprintln!("{SIGNING_KEY} is not set, and the signing key is read from nowhere else");
        return Err(ExitCode::from(exit::FAILED));
    };
    SecretKey::from_hex(text.trim()).map_err(|e| {
        eprintln!("{SIGNING_KEY}: {e}");
        ExitCode::from(exit::FAILED)
    })
}

fn read_file(path: &Path) -> Result<Vec<u8>, ExitCode> {
    std::fs::read(path).map_err(|e| {
        eprintln!("{}: {e}", path.display());
        ExitCode::from(exit::FAILED)
    })
}

fn ownership(owners_file: &Path, actor: u64, changed: &[String]) -> ExitCode {
    let text = match std::fs::read_to_string(owners_file) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("{}: {e}", owners_file.display());
            return ExitCode::from(exit::FAILED);
        }
    };
    let owners = match Owners::parse(&text) {
        Ok(owners) => owners,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(exit::FAILED);
        }
    };

    let paths: Vec<&str> = changed.iter().map(String::as_str).collect();
    match authorize(&owners, &paths, actor) {
        Authorization::Allowed => {
            println!("allowed: account {actor} owns every path in this change");
            ExitCode::from(exit::OK)
        }
        Authorization::NeedsReview { new_authors } => {
            for author in &new_authors {
                println!("review: {author} is new, so no account owns it yet");
            }
            ExitCode::from(exit::REVIEW)
        }
        Authorization::Refused { reasons } => {
            for reason in &reasons {
                println!("refused: {reason}");
            }
            ExitCode::from(exit::REJECTED)
        }
    }
}

fn read_index(path: &Path) -> Result<Index, ExitCode> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        eprintln!("{}: {e}", path.display());
        ExitCode::from(exit::FAILED)
    })?;
    Index::parse(&text).map_err(|e| {
        eprintln!("{}: {e}", path.display());
        ExitCode::from(exit::FAILED)
    })
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}
