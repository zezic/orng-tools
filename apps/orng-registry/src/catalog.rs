// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Reading ORNG Catalog.
//!
//! One file over plain HTTPS, then documents by path. No API, no token, no
//! account, no git client.
//!
//! The index is the trust anchor: every row carries the digest a download is
//! checked against, so whoever serves it decides what gets installed into a
//! DAW. It is therefore verified against a key compiled into this binary,
//! before it is parsed, and the key is never fetched. A key served beside the
//! thing it vouches for is chosen by the party the signature exists to
//! distrust.
//!
//! That is what makes a mirror safe to add later. A mirror can be wrong; it
//! cannot be believed.

use std::time::Duration;

use orng_catalog::{Digest, Index, IndexEntry, PublicKey, Revision, Signature};
use orng_tools::{Document, Kind};

/// The catalog itself, which is a repository: this is where the review of an
/// item happened and where a link to the change that published it goes.
const REPOSITORY: &str = "https://github.com/zezic/orng-catalog";

/// Where the catalog publishes. A release per merge, so this URL always serves
/// an index tied to one reviewed commit.
const INDEX_URL: &str =
    "https://github.com/zezic/orng-catalog/releases/latest/download/index.json";
const SIGNATURE_URL: &str =
    "https://github.com/zezic/orng-catalog/releases/latest/download/index.json.sig";

/// Where a published change can be read, which is what the detail panel's
/// `Reviewed in` leads to.
///
/// Built here rather than at the point of drawing, because the repository this
/// application trusts is this module's business: an index that arrived over the
/// network must not be able to say where its own review happened.
pub fn commit(revision: &orng_catalog::Revision) -> String {
    format!("{REPOSITORY}/commit/{revision}")
}

/// Where a document itself is read from: the repository's own contents, at one
/// named commit.
///
/// The index gives a repository-relative path and says nothing about a host,
/// because it is a projection of the tree and the tree has no host. This is that
/// decision, made here for the reason [`commit`] is made here: an index that
/// arrived over the network must not be able to say where its own bytes come
/// from.
///
/// **Pinned to the revision the index itself names, never to a branch.** A
/// branch moves, so a document fetched from one is bytes the verified index
/// never described - and the digest check would report that as
/// `Verification failed`, which the design is explicit is a trust event and must
/// not read like a race with somebody else's merge. At a commit the only thing a
/// mismatch can mean is that the bytes were interfered with, which is what makes
/// refusing them the right answer.
const CONTENT_URL: &str = "https://raw.githubusercontent.com/zezic/orng-catalog";

/// Where one item's document is fetched from.
///
/// Private, because the only caller is [`Install`] and the only safe way to use
/// the answer is to hash what comes back.
fn document_url(revision: &Revision, path: &str) -> String {
    // The path comes out of a signed index and is a repository path, so its
    // components are file names rather than anything a URL has to be protected
    // from. Encoded anyway for the space a document name may carry: a raw space
    // in a request line is not a URL at all.
    let path: Vec<String> =
        path.split('/').map(|part| part.replace(' ', "%20")).collect();
    format!("{CONTENT_URL}/{revision}/{}", path.join("/"))
}

/// The key ORNG Catalog signs with.
///
/// Compiled in, deliberately, and public by definition. Changing it is a
/// release of this application: there is no revocation, and an older build goes
/// on trusting the key it was built with.
const PUBLIC_KEY: &str = "5377b5a59aa5519e5e955abb63cdc59f7175a9d684b57023cb582d3c6369e720";

/// Long enough for a slow link, short enough that a wedged connection does not
/// look like a hung window. The file is under a kilobyte.
const CONNECT: Duration = Duration::from_secs(5);
const TOTAL: Duration = Duration::from_secs(20);

/// Fetch the published index, or say why not.
///
/// **Blocking, and private.** [`Fetching`] is the only way in from outside this
/// module, and it starts a thread. Nothing the window can reach will wait on a
/// socket: a blocking call is fine on a worker and unacceptable on the thread
/// that draws, and the difference is worth enforcing with visibility rather
/// than with a comment asking nicely.
///
/// There is no variant of this that returns an unverified index. Verification
/// happens over the bytes that arrived, before they are parsed, so a caller
/// cannot hold a parsed index that was never proved.
fn fetch() -> Result<Index, String> {
    let key = PublicKey::from_hex(PUBLIC_KEY).map_err(|e| format!("built-in key: {e}"))?;
    let index = get(INDEX_URL, INDEX_LIMIT)?;
    let signature = get(SIGNATURE_URL, INDEX_LIMIT)?;
    let signature = Signature::parse(&String::from_utf8_lossy(&signature))
        .map_err(|e| format!("signature: {e}"))?;

    Index::verified(&index, &signature, &key).map_err(|e| format!("{e}"))
}

/// A published index is a kilobyte or so. A cap means a server that answers with
/// something enormous costs a refusal rather than memory.
const INDEX_LIMIT: u64 = 4 * 1024 * 1024;

fn get(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT))
        .timeout_global(Some(TOTAL))
        .user_agent(concat!("orng-registry/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();

    let mut response = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|e| format!("{url}: {e}"))
}

/// A fetch running on another thread.
///
/// The same shape as a preparation: start it, poll it, draw whatever it has
/// said. A window must not wait on a socket.
pub struct Fetching {
    result: std::sync::mpsc::Receiver<Result<Index, String>>,
    pub outcome: Option<Result<Index, String>>,
}

impl Fetching {
    pub fn start(ctx: eframe::egui::Context) -> Fetching {
        let (tx, result) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(fetch());
            // Wake the window rather than leaving it to notice. Nothing else is
            // going to happen on this thread, and nothing is coming from the
            // user that would redraw it.
            ctx.request_repaint();
        });
        Fetching { result, outcome: None }
    }

    /// Returns whether anything arrived.
    pub fn poll(&mut self) -> bool {
        use std::sync::mpsc::TryRecvError;
        if self.outcome.is_some() {
            return false;
        }
        match self.result.try_recv() {
            Ok(outcome) => {
                self.outcome = Some(outcome);
                true
            }
            Err(TryRecvError::Empty) => false,
            // The worker died without answering. Silence is not an empty
            // catalog, and must not be drawn as one.
            Err(TryRecvError::Disconnected) => {
                self.outcome = Some(Err("the fetch stopped without reporting".to_owned()));
                true
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }

    /// One held still, for drawing without a network. Tests only.
    #[cfg(test)]
    pub fn frozen(outcome: Result<Index, String>) -> Fetching {
        let (tx, result) = std::sync::mpsc::channel();
        std::mem::forget(tx);
        Fetching { result, outcome: Some(outcome) }
    }
}

/// Why an item was not installed.
///
/// Two variants because the design draws two states with two different remedies,
/// and insists they not be confused: `Download failed` offers `Retry` and
/// `Verification failed` offers `Copy details` and deliberately does not. So the
/// distinction is made where the failure happens rather than read back out of a
/// message at the point of drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Nothing arrived, or the request never got that far. Ordinary, and the
    /// same request may well work in a minute.
    Download(String),
    /// Something arrived and it is not the item the index described.
    ///
    /// A trust event rather than a network condition: the catalog's review is
    /// the only thing standing between a stranger's DSP and a user's projects,
    /// and the digest is how that review reaches this machine. Asking again gets
    /// the same bytes, which is why nothing here offers to.
    Verification(String),
}

impl Refused {
    /// The worker's own words, for the control that copies them.
    pub fn details(&self) -> &str {
        match self {
            Refused::Download(why) | Refused::Verification(why) => why,
        }
    }
}

/// Fetch one item's document, and prove it is the one the index described.
///
/// **Blocking, and private**, for the reason [`fetch`] is: [`Install`] is the
/// only way in and it starts a thread.
fn fetch_document(item: &IndexEntry, from: &Revision) -> Result<Document, Refused> {
    let url = document_url(from, &item.path);
    // One byte over what the row states, so a body that is too long is read as
    // too long rather than silently cut to the right length and then reported as
    // a hash that did not match - which would accuse the catalog of something
    // the server did.
    let bytes = get(&url, item.size + 1).map_err(Refused::Download)?;
    checked(item, bytes, &url)
}

/// What arrived, held against what the index said would arrive.
///
/// Apart from the fetch on purpose. This is the whole of the trust boundary and
/// it is decided by three comparisons against a signed row, none of which needs
/// a socket - so it is tested against bytes rather than against a server that
/// has to be persuaded to lie.
fn checked(item: &IndexEntry, bytes: Vec<u8>, url: &str) -> Result<Document, Refused> {
    if bytes.len() as u64 != item.size {
        return Err(Refused::Verification(format!(
            "{url}\nthe index states {} bytes and {} arrived",
            item.size,
            bytes.len()
        )));
    }
    let arrived = Digest::of(&bytes);
    if arrived != item.digest {
        return Err(Refused::Verification(format!(
            "{url}\nthe index states {}\nwhat arrived is {arrived}",
            item.digest
        )));
    }

    // These are the published bytes - the digest says so - so a document that
    // does not read is the catalog carrying something this build cannot open
    // rather than anything the network did. Refused on the same terms, because
    // the remedies are what separate the two states and this one is not fixed by
    // asking again either.
    Document::parse(Kind::from(item.kind), bytes).map_err(|why| {
        Refused::Verification(format!("{url}\nthe published document did not read: {why}"))
    })
}

/// One item being fetched, on another thread.
///
/// The same shape as [`Fetching`], and separate from it because they answer
/// different questions: that one is the catalog and this is one row of it.
pub struct Install {
    result: std::sync::mpsc::Receiver<Result<Document, Refused>>,
    /// The row this is installing, as it stood when the press happened.
    ///
    /// Carried rather than looked up again when it finishes. The digest that was
    /// checked is this row's, so the registration written has to be this row's
    /// too: a catalog fetched again in between would otherwise let one item's
    /// bytes be registered under another's version.
    pub item: IndexEntry,
    pub outcome: Option<Result<Document, Refused>>,
}

impl Install {
    /// Start it. `from` is the commit the index named itself built from, which
    /// is where the documents it describes are read from.
    pub fn start(item: IndexEntry, from: Option<Revision>, ctx: eframe::egui::Context) -> Install {
        let (tx, result) = std::sync::mpsc::channel();
        let fetching = item.clone();
        std::thread::spawn(move || {
            let outcome = match from {
                Some(revision) => fetch_document(&fetching, &revision),
                // An index built inside a pull request carries no revision, and
                // a branch is not an answer: the bytes there are not the bytes
                // this index described. Retryable, because the next fetch of the
                // catalog may well name one.
                None => Err(Refused::Download(
                    "this catalog index does not name the commit it was built from, so there \
                     is no reviewed copy of the document to fetch"
                        .to_owned(),
                )),
            };
            let _ = tx.send(outcome);
            ctx.request_repaint();
        });
        Install { result, item, outcome: None }
    }

    /// Returns whether anything arrived.
    pub fn poll(&mut self) -> bool {
        use std::sync::mpsc::TryRecvError;
        if self.outcome.is_some() {
            return false;
        }
        match self.result.try_recv() {
            Ok(outcome) => {
                self.outcome = Some(outcome);
                true
            }
            Err(TryRecvError::Empty) => false,
            // The worker died without answering. Silence is not a document, and
            // must never be registered as one.
            Err(TryRecvError::Disconnected) => {
                self.outcome = Some(Err(Refused::Download(
                    "the download stopped without reporting".to_owned(),
                )));
                true
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }

    /// One that has already answered, for driving what the window does with a
    /// fetch without there being a network to do it over. Tests only.
    #[cfg(test)]
    pub fn finished(item: IndexEntry, outcome: Result<Document, Refused>) -> Install {
        let (tx, result) = std::sync::mpsc::channel();
        // The sender is kept alive on purpose, for the reason
        // `Applying::frozen` keeps its own: a disconnected channel with no
        // outcome is how a dead worker is recognised, and this one is not dead.
        std::mem::forget(tx);
        Install { result, item, outcome: Some(outcome) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key has to parse, or every fetch fails at the last step with a
    /// message about our own binary. A typo here is a typo nothing else
    /// catches.
    #[test]
    fn the_built_in_key_is_a_key() {
        let key = PublicKey::from_hex(PUBLIC_KEY).expect("the compiled-in key does not parse");
        assert_eq!(key.to_hex(), PUBLIC_KEY, "the key does not round trip");
    }

    /// The real thing, against the real catalog.
    ///
    /// Ignored by default because it needs the network, and a test that fails
    /// when a train goes into a tunnel is a test people learn to ignore. Run it
    /// with `--ignored` when the published index changes.
    #[test]
    #[ignore = "needs the network"]
    fn the_published_index_verifies() {
        let index = fetch().expect("the published index did not verify");
        assert_eq!(index.schema, orng_catalog::index::SCHEMA);
        assert!(!index.items.is_empty(), "the published catalog is empty");
    }

    /// Both assets come from one release, so they must name one place. A URL
    /// edited in one and not the other would fetch an index and last week's
    /// signature, which verifies against nothing.
    #[test]
    fn both_assets_come_from_the_same_release() {
        let (index, _) = INDEX_URL.rsplit_once('/').expect("a path");
        let (signature, _) = SIGNATURE_URL.rsplit_once('/').expect("a path");
        assert_eq!(index, signature);
        assert!(INDEX_URL.starts_with("https://"), "the index is fetched over plain HTTP");
        assert!(SIGNATURE_URL.starts_with("https://"));
        // And from the repository the links in the detail panel lead into. A
        // link to a commit in some other repository is a link to a review that
        // did not happen.
        assert!(INDEX_URL.starts_with(REPOSITORY), "{INDEX_URL} is not published by {REPOSITORY}");
    }

    /// The real published index, which is what a document URL is built out of.
    fn published() -> Index {
        Index::parse(include_str!("../tests/published-index.json"))
            .expect("the published index does not parse")
    }

    /// A document is fetched at the commit the index names and never from a
    /// branch.
    ///
    /// Asserted against a whole URL rather than against its parts, because the
    /// whole URL is the claim: this exact string was fetched by hand and gave
    /// back bytes hashing to the digest the same index publishes. A revision
    /// dropped out of it would still fetch, from wherever the branch has got to
    /// since, and everything downstream would go on looking right until somebody
    /// merged something.
    #[test]
    fn a_document_is_fetched_at_the_commit_the_index_names() {
        let index = published();
        let item = index.items.first().expect("the published catalog is empty");
        let revision = index.revision.as_ref().expect("a published index names its commit");

        assert_eq!(
            document_url(revision, &item.path),
            "https://raw.githubusercontent.com/zezic/orng-catalog/\
             bd1f83732b8b093154e7956baa724e9855ba8995/content/caviio/volshaper/VOLSHAPER.bwdevice"
        );
        // And the revision in it is the index's own, not a name that moves.
        assert!(document_url(revision, &item.path).contains(revision.as_str()));
        for branch in ["/master/", "/main/", "/HEAD/"] {
            assert!(!document_url(revision, &item.path).contains(branch));
        }
    }

    /// Document names are the author's, and a space in one is a request line
    /// that is not a URL at all.
    #[test]
    fn a_document_name_with_a_space_in_it_still_makes_a_url() {
        let revision = Revision::new(&"a".repeat(40)).expect("a revision");
        assert_eq!(
            document_url(&revision, "content/someone/gate-in/GATE IN.bwmodule"),
            format!("{CONTENT_URL}/{revision}/content/someone/gate-in/GATE%20IN.bwmodule")
        );
        // The separators are not encoded with it: they are the path.
        assert!(!document_url(&revision, "a/b").contains("%2F"));
    }

    /// A row to check bytes against, describing exactly the bytes handed in.
    fn describing(bytes: &[u8]) -> IndexEntry {
        let mut item = published().items.remove(0);
        item.size = bytes.len() as u64;
        item.digest = Digest::of(bytes);
        item
    }

    /// The whole of the trust boundary, in the two ways it can be crossed and
    /// the one way it can be passed.
    ///
    /// The two failures are separated here and not at the point of drawing,
    /// because what separates them is what each one offers: the design gives a
    /// download `Retry` and refuses to give verification one.
    #[test]
    fn bytes_that_are_not_what_the_index_described_are_refused_as_a_trust_event() {
        let document = orng_tools::testing::document(
            Kind::Device,
            "8b330d22-73fa-4ba5-a42f-2f2300cbd8bf".parse().expect("an identity"),
            "VOLSHAPER",
        );
        let bytes = document.bytes().to_vec();
        let item = describing(&bytes);

        // The document the index describes, which is the whole point.
        let read = checked(&item, bytes.clone(), "https://example.invalid/d")
            .expect("the published document was refused");
        assert_eq!(read.identity().uuid, item.uuid);

        // A body of the right length that is not those bytes. The case the
        // digest exists for, and the one a length check cannot catch.
        let mut tampered = bytes.clone();
        *tampered.last_mut().expect("a document is not empty") ^= 0xff;
        let why = checked(&item, tampered, "https://example.invalid/d")
            .expect_err("bytes that hash to something else were accepted");
        assert!(matches!(why, Refused::Verification(_)), "{why:?}");
        assert!(why.details().contains(item.digest.as_str()), "{}", why.details());

        // Too short, and too long - the second is what the read's own cap would
        // otherwise have cut back to exactly the right length.
        for wrong in [&bytes[..bytes.len() - 1], &[bytes.as_slice(), b"x"].concat()[..]] {
            let why = checked(&item, wrong.to_vec(), "https://example.invalid/d")
                .expect_err("a body of the wrong length was accepted");
            assert!(matches!(why, Refused::Verification(_)), "{why:?}");
            assert!(why.details().contains(&item.size.to_string()), "{}", why.details());
        }

        // And bytes that are exactly what was published and are not a document.
        // Not retryable either: asking again gets the same file.
        let nonsense = vec![b'x'; 4096];
        let why = checked(&describing(&nonsense), nonsense, "https://example.invalid/d")
            .expect_err("something that is not a document was accepted");
        assert!(matches!(why, Refused::Verification(_)), "{why:?}");
    }

    /// The real thing, against the real catalog, end to end: fetch the index,
    /// then fetch the document it describes and prove it is that document.
    ///
    /// Ignored for the reason the index's own network test is. This is the one
    /// assertion that the URL shape above is not merely well formed but is where
    /// the bytes actually are.
    #[test]
    #[ignore = "needs the network"]
    fn the_published_document_is_the_one_the_index_describes() {
        let index = fetch().expect("the published index did not verify");
        let item = index.items.first().expect("the published catalog is empty");
        let revision = index.revision.clone().expect("a published index names its commit");
        let document = fetch_document(item, &revision).expect("the published document was refused");
        assert_eq!(document.identity().uuid, item.uuid);
    }
}
