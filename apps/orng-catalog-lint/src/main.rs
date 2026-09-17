// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tool that guards the ORNG Catalog repository.
//!
//! Every rule it applies lives in `orng-catalog`, which is also what the
//! application reads the catalog with. One implementation, so the repository and
//! the installer cannot disagree about what a valid item is.
//!
//! Two of the three commands take a file explicitly rather than finding it in
//! the checkout, and that is the point of them:
//!
//! - `owners` is given the owners file, because it must be the one on the base
//!   branch. The copy in a pull request is written by the contributor being
//!   checked, who could add themselves to it in the commit under review.
//! - `check --against` is given the last published index, because permanence is
//!   a promise about what was published, not about what the branch says now.
//!
//! Making the caller name both files keeps that decision where it can be read,
//! in a workflow, rather than buried in a default.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use orng_catalog::{
    Authorization, History, Index, Item, Owners, Revision, Severity, authorize, scan, validate,
};

/// Outcomes, as a process exit status. A workflow branches on these, so they are
/// part of the interface and not an afterthought.
mod exit {
    /// Nothing to report.
    pub const OK: u8 = 0;
    /// The tree, or the pull request, is not acceptable.
    pub const REJECTED: u8 = 1;
    /// The tool could not run: a missing file, an unreadable checkout.
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
