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

use orng_catalog::{Index, PublicKey, Signature};

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
    let index = get(INDEX_URL)?;
    let signature = get(SIGNATURE_URL)?;
    let signature = Signature::parse(&String::from_utf8_lossy(&signature))
        .map_err(|e| format!("signature: {e}"))?;

    Index::verified(&index, &signature, &key).map_err(|e| format!("{e}"))
}

fn get(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT))
        .timeout_global(Some(TOTAL))
        .user_agent(concat!("orng-registry/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();

    let mut response = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
    response
        .body_mut()
        // A published index is a kilobyte or so. A cap means a server that
        // answers with something enormous costs a refusal rather than memory.
        .with_config()
        .limit(4 * 1024 * 1024)
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
}
