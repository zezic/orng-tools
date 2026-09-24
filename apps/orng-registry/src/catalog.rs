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
//!
//! **The last index that verified is kept on disk**, so a window opened with no
//! network browses an old catalog rather than none. What is kept is the bytes
//! and the signature over them and never a re-serialised index, which means
//! reading the cache back is the same check the download went through: a file
//! edited under `~/.orng` is refused on exactly the terms a tampered download
//! is, and the trust boundary does not move because the bytes came off a disk.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use orng_catalog::{Digest, Index, IndexEntry, PublicKey, Revision, Signature};
use orng_tools::{Document, Kind, OrngHome, Uuid};
use ureq::http::Uri;

/// The catalog itself, which is a repository: this is where the review of an
/// item happened and where a link to the change that published it goes.
const REPOSITORY: &str = "https://github.com/zezic/orng-catalog";

/// Where the catalog publishes. A release per merge, so this URL always serves
/// an index tied to one reviewed commit.
const INDEX_URL: &str =
    "https://github.com/zezic/orng-catalog/releases/latest/download/index.json";
/// The asset's own name, which is how the signature is found beside whichever
/// release `latest` turned out to be - see [`beside`].
const INDEX_ASSET: &str = "index.json";

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
    // components are file names. A file name is still not a URL: the separators
    // are the path and everything else in a component is a name the author
    // chose, so each component is escaped and the slashes are not.
    let path: Vec<String> = path.split('/').map(escaped).collect();
    format!("{CONTENT_URL}/{revision}/{}", path.join("/"))
}

/// One path component, with every byte a URL gives a meaning to written as an
/// escape.
///
/// The unreserved set and nothing else, which is the only rule that needs no
/// judgement about what a host will do. A space was escaped here before and the
/// rest were not, and the rest are what turn a legitimate name into a different
/// request: `#` cuts the URL short at a fragment and `?` at a query, so the
/// bytes that came back would be some other file's - and the digest check can
/// only report that as `Verification failed`, which the design is explicit is a
/// trust event rather than a name nobody escaped.
fn escaped(component: &str) -> String {
    component
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            reserved => format!("%{reserved:02X}"),
        })
        .collect()
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

/// A verified index, with the bytes it was proved from.
///
/// The bytes travel with it because they are what gets kept: an index this
/// application re-serialised is one the signature no longer covers, so the only
/// copy worth writing down is the one that arrived.
struct Fetched {
    index: Index,
    bytes: Vec<u8>,
    signature: Vec<u8>,
}

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
fn fetch() -> Result<Fetched, String> {
    // **Both assets out of one release, and `latest` is resolved once.** It is
    // a redirect, so asking for it twice can straddle a publication and bring
    // back the index of one release beside the signature of the next - a pair
    // that verifies against nothing and is reported as a bad signature, which
    // points at the key rather than at the race. So the index's own answer says
    // which release this is, and the signature is then asked for by name.
    let (bytes, hops) = got(INDEX_URL, INDEX_LIMIT)?;
    let signature = get(&beside(&hops), INDEX_LIMIT)?;
    let index = verified(&bytes, &signature)?;
    Ok(Fetched { index, bytes, signature })
}

/// The signature published beside an index, given the addresses the request for
/// that index passed through.
///
/// **Not the address the bytes finally came from.** GitHub answers a release
/// asset out of its storage host, under an opaque identifier and a signed
/// query, so that address names neither the release nor the asset. The hop
/// before it does: `latest` redirects to `/releases/download/<tag>/index.json`,
/// the last address in the chain still named for the asset, and the signature
/// is one name beside it. With no redirect at all that is the request itself,
/// and the signature is asked for from `latest` exactly as the index was.
fn beside(hops: &[Uri]) -> String {
    let release = hops
        .iter()
        .rev()
        .find(|hop| hop.path().strip_suffix(INDEX_ASSET).is_some_and(|dir| dir.ends_with('/')))
        .expect("the index is asked for by its own name, and the request is the first hop");
    format!(
        "{}://{}{}.sig",
        release.scheme_str().expect("a request ureq made has a scheme"),
        release.authority().expect("a request ureq made has a host"),
        release.path()
    )
}

/// The check itself, over bytes and nothing else.
///
/// Apart from the fetch for the reason [`checked`] is, and it earns that twice
/// over now: the download and the cache both come through here, so there is one
/// place where an index becomes believable and it takes no socket and no disk.
fn verified(bytes: &[u8], signature: &[u8]) -> Result<Index, String> {
    let key = PublicKey::from_hex(PUBLIC_KEY).map_err(|e| format!("built-in key: {e}"))?;
    let signature = Signature::parse(&String::from_utf8_lossy(signature))
        .map_err(|e| format!("signature: {e}"))?;

    Index::verified(bytes, &signature, &key).map_err(|e| format!("{e}"))
}

/// A published index is a kilobyte or so. A cap means a server that answers with
/// something enormous costs a refusal rather than memory.
const INDEX_LIMIT: u64 = 4 * 1024 * 1024;

/// The one agent every request here goes through.
///
/// Built once rather than per call: an agent carries the connection pool and
/// the TLS configuration, so a fresh one for each of the two requests a refresh
/// makes is a second handshake to a host the first one is still connected to.
fn agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT))
            .timeout_global(Some(TOTAL))
            .user_agent(concat!("orng-registry/", env!("CARGO_PKG_VERSION")))
            // What `beside` reads the release off.
            .save_redirect_history(true)
            .build()
            .new_agent()
    })
}

fn get(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    got(url, limit).map(|(bytes, _)| bytes)
}

/// The same, and every address the request passed through on the way.
///
/// The second half is what [`fetch`] needs and no other caller does: these URLs
/// redirect, and which release a redirect landed on is the only way to ask for
/// the asset beside it rather than for `latest` a second time.
fn got(url: &str, limit: u64) -> Result<(Vec<u8>, Vec<Uri>), String> {
    use ureq::ResponseExt;

    let mut response = agent().get(url).call().map_err(|e| format!("{url}: {e}"))?;
    let hops = response
        .get_redirect_history()
        .expect("the agent is built to keep the redirect history")
        .to_vec();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|e| format!("{url}: {e}"))?;
    Ok((bytes, hops))
}

/// A fetch running on another thread.
///
/// The same shape as a preparation: start it, poll it, draw whatever it has
/// said. A window must not wait on a socket.
///
/// Private, because [`Catalog`] is what the window holds: a fetch on its own
/// cannot answer "what can be browsed", and a window that asked it would have
/// no catalog every time the network was down.
struct Fetching {
    result: std::sync::mpsc::Receiver<Result<Fetched, String>>,
    outcome: Option<Result<Fetched, String>>,
}

impl Fetching {
    fn start(ctx: eframe::egui::Context) -> Fetching {
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

    /// Take the answer if there is one.
    ///
    /// Says nothing about whether there was: [`Catalog::poll`] is the only
    /// caller and it reads [`Fetching::outcome`] afterwards, which is the one
    /// question that is still true on the frame after the answer landed.
    fn poll(&mut self) {
        use std::sync::mpsc::TryRecvError;
        if self.outcome.is_some() {
            return;
        }
        match self.result.try_recv() {
            Ok(outcome) => self.outcome = Some(outcome),
            Err(TryRecvError::Empty) => {}
            // The worker died without answering. Silence is not an empty
            // catalog, and must not be drawn as one.
            Err(TryRecvError::Disconnected) => {
                self.outcome = Some(Err("the fetch stopped without reporting".to_owned()));
            }
        }
    }
}

/// The catalog as this window has it.
///
/// Three questions, and they are deliberately not one: what can be browsed,
/// what the last attempt came to, and how old what is browsable is. A window
/// that is offline with a cache answers all three at once and the design draws
/// all three at once - the list, `cached` on the action bar, and the age beside
/// the view switch. A single `Result` could only ever say one of them, which is
/// why the fetch alone was never enough to hold.
pub struct Catalog {
    /// The last index that verified, when its bytes arrived, and what was
    /// refused against it. Read off the disk on opening and replaced by every
    /// fetch that succeeds.
    held: Option<Held>,
    /// The fetch in flight. Taken the frame it answers, because a fetch that
    /// has reported is not work in flight - the same rule [`crate::app::App`]
    /// applies to an [`Install`].
    fetching: Option<Fetching>,
    /// Why the last attempt produced nothing. Cleared by one that succeeds.
    failed: Option<String>,
    /// Where a verified index is written back to. **`None` under the tests**,
    /// for the reason [`crate::settings::Preferences`] keeps its own `None`
    /// there: a fixture must neither read nor write the cache of whoever ran
    /// it. Also `None` on a machine with no home directory, which still
    /// browses - it just starts every run with nothing.
    home: Option<OrngHome>,
}

/// An index, the moment its bytes arrived, and the installs refused against it.
struct Held {
    index: Index,
    fetched: SystemTime,
    /// Items an install attempt refused, until something happens that could
    /// change the answer.
    ///
    /// The design's two failure states are per item and not per window - a
    /// catalog of forty rows where one did not verify is thirty-nine rows that
    /// are still fine - so they are held against the identity that failed. Kept
    /// until that item is pressed again or a refresh brings back a *different*
    /// index, because nothing else that happens makes them stop being true - and
    /// a refresh that confirms the index they were made against does not either.
    ///
    /// **Here, beside the index, because each is a claim about one row of it.**
    /// Held apart, an index could be replaced and leave them standing: the bar
    /// reads `Install refused` while any is held, so one against an item the
    /// catalog has since stopped publishing would sit there for the rest of the
    /// run with no row under it to explain itself.
    refused: BTreeMap<Uuid, Refused>,
}

impl Held {
    fn new(index: Index, fetched: SystemTime) -> Held {
        Held { index, fetched, refused: BTreeMap::new() }
    }
}

/// How old the index in hand is, as the bar states it.
///
/// Three states rather than a duration beside a boolean, because the design
/// draws three and each carries what the next one needs: `Never fetched` with
/// no age at all, an age in `ink_3` beside a bare icon, and the same age in the
/// accent beside a control that has grown its word - `InstallBar.dc.html:36-40`
/// and `:93-97`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Nothing has ever verified on this machine.
    Never,
    /// Fetched, and recently enough not to say so loudly.
    Current(Duration),
    /// Old enough that the design asks for it to be refreshed.
    Stale(Duration),
}

impl Freshness {
    /// Whether the design states this one loudly: the accent rather than the
    /// quiet grey, and the control with its label rather than the icon alone.
    ///
    /// `Never` is stale - `ORNG Registry.dc.html:472` sets `stale: true` on the
    /// scenario whose freshness reads `Never fetched`.
    pub fn is_stale(self) -> bool {
        !matches!(self, Freshness::Current(_))
    }
}

/// When an index stops being described as current.
///
/// **Ours, not the bundle's.** It draws 20 minutes as current and 12 days as
/// stale and states no line between them - `docs/design-review.md` round 3
/// item 6. A week, because of what reaching it now means: this window checks on
/// every launch, so seven days without one succeeding is a machine that has
/// been off the network for a week rather than a catalog nobody has published
/// to, and that is worth saying out loud.
const STALE_AFTER: Duration = Duration::from_secs(7 * 24 * 60 * 60);

impl Catalog {
    /// What the disk already has, with no network at all.
    ///
    /// Opening is not fetching: this reads two files and checks a signature,
    /// and the window is drawable the moment it returns. [`Catalog::refresh`]
    /// is the other half and it is a separate call for that reason.
    pub fn opened(home: Option<OrngHome>) -> Catalog {
        let held = home.as_ref().and_then(cached);
        Catalog { held, fetching: None, failed: None, home }
    }

    /// Ask the catalog again.
    ///
    /// A press while one is already in flight is nothing rather than a second
    /// thread. The control stays pressable because the design draws it that
    /// way, and what it asks for is already happening.
    pub fn refresh(&mut self, ctx: &eframe::egui::Context) {
        if self.fetching.is_none() {
            self.fetching = Some(Fetching::start(ctx.clone()));
        }
    }

    /// Take whatever the fetch has said, and keep it.
    ///
    /// **Only a different catalog takes the refusals with it**, which is
    /// narrower than anything arriving: a refresh that confirms what was already
    /// held moves the age and nothing else, and one that failed moves neither.
    pub fn poll(&mut self) {
        let Some(fetching) = &mut self.fetching else { return };
        // Asked of the outcome rather than of the call, because "did anything
        // arrive just now" is false again on the next frame and "is there an
        // answer here" is not. The same distinction `App::pump` draws around
        // an `Install`, and the reason it asks `is_running` rather than
        // trusting the poll.
        fetching.poll();
        if fetching.outcome.is_none() {
            return;
        }
        let fetching = self.fetching.take().expect("it answered a moment ago");
        match fetching.outcome.expect("a fetch that answered carries an outcome") {
            Ok(fetched) => {
                if let Some(home) = &self.home {
                    keep(home, &fetched);
                }
                // Now, rather than reading back the time the file landed with:
                // the two are one write, and a machine with nowhere to keep it
                // still knows when this arrived.
                let mut held = Held::new(fetched.index, SystemTime::now());
                if let Some(before) = self.held.take()
                    && before.index == held.index
                {
                    held.refused = before.refused;
                }
                self.held = Some(held);
                self.failed = None;
            }
            // **What is held is not dropped.** It verified when it arrived and
            // it still does, and the design is explicit that browsing an older
            // index offline is a degraded state and not an error. So nothing on
            // screen is a different catalog, and nothing worked out against the
            // one held has stopped being true.
            Err(why) => self.failed = Some(why),
        }
    }

    /// The index in hand, whether it came off the network this run or off the
    /// disk.
    ///
    /// `None` covers the two states the callers treat alike - nothing has ever
    /// verified here, or a fetch is still running - and they are alike: in both
    /// this application knows nothing about what is published and must not say
    /// that anything is up to date either.
    pub fn index(&self) -> Option<&Index> {
        self.held.as_ref().map(|held| &held.index)
    }

    /// Why the last attempt produced no index, for the one surface that states
    /// it: the empty state, which is drawn only when nothing is held.
    pub fn failure(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// Whether what is on screen is an index the last refresh did not replace.
    ///
    /// The design's `cached` word - `ORNG Registry.dc.html:468` - and it turns
    /// on the refresh having failed rather than on where the bytes came from.
    /// An index read off the disk and then confirmed by a fetch that worked is
    /// current, and calling it cached would be the window reporting its own
    /// plumbing.
    pub fn is_cached(&self) -> bool {
        self.held.is_some() && self.failed.is_some()
    }

    /// Why an install of this item was refused, if one was and nothing has
    /// happened since that could change the answer.
    pub fn refusal(&self, item: Uuid) -> Option<&Refused> {
        self.held.as_ref()?.refused.get(&item)
    }

    /// Whether any install was refused, which is the whole of what the bar says
    /// about them: the rows say which, and why.
    pub fn any_refused(&self) -> bool {
        self.held.as_ref().is_some_and(|held| !held.refused.is_empty())
    }

    /// Hold a refusal against the item it was of.
    ///
    /// An install is only ever started from an index in hand, and nothing
    /// takes one away once it is held, so there is always one to hold it
    /// against.
    ///
    /// **Not against an index that no longer publishes the item.** A refresh
    /// may replace the index while the fetch is out, and a refusal held against
    /// the new one for an item it does not carry would keep the bar reading
    /// `Install refused` with no row under it to explain it or to retry.
    pub fn refuse(&mut self, item: Uuid, why: Refused) {
        let held = self.held.as_mut().expect("an install was refused with no index in hand");
        if held.index.items.iter().any(|published| published.uuid == item) {
            held.refused.insert(item, why);
        }
    }

    /// Let an item be tried again: the press on it is a press on the row as it
    /// will be once it is tried, and not on the one that failed.
    pub fn retry(&mut self, item: Uuid) {
        if let Some(held) = &mut self.held {
            held.refused.remove(&item);
        }
    }

    pub fn freshness(&self) -> Freshness {
        let Some(held) = &self.held else { return Freshness::Never };
        // A clock moved backwards since the write says nothing about the index,
        // so it reads as just fetched rather than as an age in the future.
        let age = held.fetched.elapsed().unwrap_or_default();
        if age >= STALE_AFTER { Freshness::Stale(age) } else { Freshness::Current(age) }
    }
}

/// The kept index, if there is one and it still proves.
///
/// Silent about a cache that is not there, which is every first run. Loud about
/// one that is there and does not verify, because that is either a half-written
/// pair or a file somebody edited, and neither should pass without a word - but
/// loud only where a developer reads: the remedy is the refresh that is already
/// starting, and there is nothing for the user to do.
fn cached(home: &OrngHome) -> Option<Held> {
    let bytes = std::fs::read(home.catalog_index()).ok()?;
    let signature = std::fs::read(home.catalog_signature()).ok()?;
    // The file's own write time, which is when this arrived: every successful
    // fetch rewrites both files whether or not the bytes changed, so the age
    // states when the catalog was last confirmed rather than when it last said
    // something new.
    let fetched = std::fs::metadata(home.catalog_index()).and_then(|at| at.modified()).ok()?;
    match verified(&bytes, &signature) {
        Ok(index) => Some(Held::new(index, fetched)),
        Err(why) => {
            eprintln!("the kept catalog index did not verify, so it was ignored: {why}");
            None
        }
    }
}

/// Write an index back, so the next launch has one before it has a network.
///
/// The two files are written in the order they are read in, and a run
/// interrupted between them leaves a pair that does not verify - which
/// [`cached`] refuses and the next fetch overwrites. That is the whole of the
/// recovery, and it is why there is no third file holding a time: anything this
/// pair cannot prove about itself is not worth keeping beside it.
fn keep(home: &OrngHome, fetched: &Fetched) {
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(home.catalog())?;
        std::fs::write(home.catalog_index(), &fetched.bytes)?;
        std::fs::write(home.catalog_signature(), &fetched.signature)
    };
    if let Err(why) = write() {
        // A cache that could not be written costs a slower next launch and
        // nothing else. Reported where a developer sees it and nowhere the user
        // has to act on it.
        eprintln!("the catalog index was not kept: {why}");
    }
}

/// The four states the window draws, built without a disk or a socket.
///
/// Tests only, and every one of them has no home, so a fixture neither reads
/// nor writes the cache of whoever ran it.
#[cfg(test)]
impl Catalog {
    /// A fetch in flight with nothing to show yet. The state a first run is in
    /// for a second or two, and the one nothing could reach before the window
    /// held a catalog rather than a fetch.
    pub fn fetching() -> Catalog {
        Catalog { held: None, fetching: None, failed: None, home: None }
    }

    /// One that answered a moment ago, which is the ordinary state.
    pub fn just_fetched(index: Index) -> Catalog {
        Catalog {
            held: Some(Held::new(index, SystemTime::now())),
            ..Catalog::fetching()
        }
    }

    /// An index from `ago` back and a refresh that came to nothing: the offline
    /// state, which the design draws `cached` for.
    pub fn cached(index: Index, ago: Duration, why: &str) -> Catalog {
        Catalog {
            held: Some(Held::new(index, SystemTime::now() - ago)),
            failed: Some(why.to_owned()),
            ..Catalog::fetching()
        }
    }

    /// Nothing held, and nothing arrived.
    pub fn unavailable(why: &str) -> Catalog {
        Catalog { failed: Some(why.to_owned()), ..Catalog::fetching() }
    }

    /// Any of the four with a refresh that has already answered, so that what
    /// [`Catalog::poll`] makes of an answer can be driven without a socket.
    ///
    /// The bytes are empty because nothing here reads them: they are what
    /// [`keep`] writes, and a catalog built this way has nowhere to write to.
    pub fn answering(mut self, outcome: Result<Index, String>) -> Catalog {
        self.answer(outcome);
        self
    }

    /// The same, on a catalog already in hand - so that what was held against
    /// it before the refresh is still there to be kept or taken.
    pub fn answer(&mut self, outcome: Result<Index, String>) {
        let (tx, result) = std::sync::mpsc::channel();
        // Kept alive on purpose, for the reason `Install::finished` keeps its
        // own: a disconnected channel with no outcome is how a dead worker is
        // recognised, and this one is not dead.
        std::mem::forget(tx);
        let outcome = outcome
            .map(|index| Fetched { index, bytes: Vec::new(), signature: Vec::new() });
        self.fetching = Some(Fetching { result, outcome: Some(outcome) });
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

    /// Take the answer if there is one.
    ///
    /// Says nothing about whether one arrived, for the reason [`Fetching::poll`]
    /// says nothing: "did anything arrive just now" is false again on the next
    /// frame, and [`Install::is_running`] is the question that is still true -
    /// which is the one the only caller asks.
    pub fn poll(&mut self) {
        use std::sync::mpsc::TryRecvError;
        if self.outcome.is_some() {
            return;
        }
        match self.result.try_recv() {
            Ok(outcome) => self.outcome = Some(outcome),
            Err(TryRecvError::Empty) => {}
            // The worker died without answering. Silence is not a document, and
            // must never be registered as one.
            Err(TryRecvError::Disconnected) => {
                self.outcome = Some(Err(Refused::Download(
                    "the download stopped without reporting".to_owned(),
                )));
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

    /// One that is still out and never answers. Tests only.
    #[cfg(test)]
    pub fn fetching(item: IndexEntry) -> Install {
        let (tx, result) = std::sync::mpsc::channel();
        // Kept alive, for the reason `Install::finished` gives.
        std::mem::forget(tx);
        Install { result, item, outcome: None }
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
        let fetched = fetch().expect("the published index did not verify");
        assert_eq!(fetched.index.schema, orng_catalog::index::SCHEMA);
        assert!(!fetched.index.items.is_empty(), "the published catalog is empty");
        // And the bytes that came with it are the bytes that were proved, which
        // is the whole of what makes them safe to write down and read back.
        assert_eq!(
            verified(&fetched.bytes, &fetched.signature).expect("the kept bytes do not verify"),
            fetched.index
        );
    }

    /// Both assets come from one release, so they must name one place: the
    /// signature is always the index's own address with `.sig` on the end, so
    /// no edit to one can leave the other behind.
    #[test]
    fn both_assets_come_from_the_same_release() {
        assert!(INDEX_URL.ends_with(&format!("/{INDEX_ASSET}")), "{INDEX_URL} is not the index");
        let request: Uri = INDEX_URL.parse().expect("the index URL is a URL");
        assert_eq!(beside(&[request]), format!("{INDEX_URL}.sig"));
        assert!(INDEX_URL.starts_with("https://"), "the index is fetched over plain HTTP");
        // And from the repository the links in the detail panel lead into. A
        // link to a commit in some other repository is a link to a review that
        // did not happen.
        assert!(INDEX_URL.starts_with(REPOSITORY), "{INDEX_URL} is not published by {REPOSITORY}");
    }

    /// The signature is asked for from the release `latest` named, which is the
    /// hop before the storage host and not the address the bytes came from.
    ///
    /// The chain is the one GitHub really answers with, the storage query cut
    /// short. Taking the last hop instead finds no asset name in it at all.
    #[test]
    fn the_signature_is_asked_for_beside_the_release_the_index_came_from() {
        let hops: Vec<Uri> = [
            INDEX_URL,
            "https://github.com/zezic/orng-catalog/releases/download/index-bd1f837/index.json",
            "https://release-assets.githubusercontent.com/github-production-release-asset/\
             1373401772/7aeadc60-49a2-4c2c-8a63-23336161b189?sp=r&sv=2018-11-09&sr=b",
        ]
        .iter()
        .map(|hop| hop.parse().expect("a URL"))
        .collect();

        assert_eq!(
            beside(&hops),
            "https://github.com/zezic/orng-catalog/releases/download/index-bd1f837/index.json.sig"
        );
    }

    /// And against the real thing, which is the only place the shape of that
    /// chain is decided.
    #[test]
    #[ignore = "needs the network"]
    fn the_published_signature_is_found_beside_its_release() {
        let (_, hops) = got(INDEX_URL, INDEX_LIMIT).expect("the published index did not arrive");
        let signature = beside(&hops);
        assert!(signature.contains("/releases/download/"), "not a release's own: {signature}");
        assert!(!signature.contains("/latest/"), "asked of latest a second time: {signature}");
    }

    /// The real published index, which is what a document URL is built out of.
    fn published() -> Index {
        Index::parse(include_str!("../tests/published-index.json"))
            .expect("the published index does not parse")
    }

    /// The same index as bytes, with the signature ORNG Catalog published over
    /// exactly those bytes.
    ///
    /// The real pair, because the cache is only worth testing against one: the
    /// check it goes through is the compiled-in key, and nothing this test
    /// could sign for itself would exercise that. `the_published_index_verifies`
    /// is what says the pair is still the published one, and it needs the
    /// network; this needs neither a network nor a secret key.
    fn published_pair() -> Fetched {
        let bytes = include_bytes!("../tests/published-index.json").to_vec();
        let signature = include_bytes!("../tests/published-index.json.sig").to_vec();
        let index = verified(&bytes, &signature).expect("the published pair does not verify");
        Fetched { index, bytes, signature }
    }

    /// The signed index is bytes, and a checkout must hand them over unchanged.
    ///
    /// A signature is over exact bytes, so a checkout that rewrote the file's
    /// line endings produces a pair that cannot verify - and `verified` can only
    /// report that as a bad signature, which points at the key or the release
    /// rather than at the checkout. This is the assertion that names the real
    /// cause. `.gitattributes` marks both files `-text` to prevent it; this is
    /// what fails loudly if that rule is lost or a new checkout ignores it.
    ///
    /// Git for Windows defaults `core.autocrlf` to true, which is why the two
    /// tests that verify this pair failed only on the Windows runner.
    #[test]
    fn the_signed_index_is_checked_out_byte_for_byte() {
        let bytes = include_bytes!("../tests/published-index.json");
        assert!(
            !bytes.windows(2).any(|pair| pair == b"\r\n"),
            "the signed index was checked out with its line endings rewritten, so the \
             signature published over it cannot verify"
        );
    }

    /// An index kept on disk is read back, and read back through the same check
    /// it arrived through.
    ///
    /// The round trip whole, because the halves are worth nothing apart: bytes
    /// written in a shape [`cached`] cannot prove are bytes the next launch
    /// throws away, and the failure would be invisible - a window that simply
    /// went on having no catalog until it had a network.
    #[test]
    fn a_kept_index_is_read_back_and_proved() {
        let temp = tempfile::tempdir().expect("somewhere to keep it");
        let home = OrngHome::at(temp.path());
        let published = published_pair();

        assert!(cached(&home).is_none(), "an empty home had a catalog in it");
        keep(&home, &published);
        let held = cached(&home).expect("what was just written did not come back");
        assert_eq!(held.index, published.index);
        // And it is dated, which is what the bar states an age from.
        assert!(
            held.fetched.elapsed().expect("the clock went backwards") < Duration::from_secs(60),
            "the kept index came back dated some other time than when it was written"
        );
    }

    /// A cache somebody edited is refused, on the terms a tampered download is.
    ///
    /// This is the reason the bytes are kept rather than a re-serialised index.
    /// `~/.orng` is an ordinary directory in the user's home, so the file is
    /// writable by anything running as them - and an index decides what gets
    /// downloaded into a DAW and what digest it is held against.
    ///
    /// **The edit leaves valid JSON on purpose.** A byte flipped at the end of
    /// the file is caught by the parser, so refusing it says nothing about the
    /// signature; what has to be refused is a file that parses perfectly and
    /// describes a different download. Here that is one character of the digest
    /// every fetched document is checked against.
    #[test]
    fn a_kept_index_that_was_edited_is_refused() {
        let temp = tempfile::tempdir().expect("somewhere to keep it");
        let home = OrngHome::at(temp.path());
        keep(&home, &published_pair());

        let written = std::fs::read_to_string(home.catalog_index()).expect("it was just written");
        let edited = written.replace("\"digest\": \"bfda", "\"digest\": \"bfdb");
        assert_ne!(edited, written, "the digest this edits is not in the fixture any more");
        assert!(Index::parse(&edited).is_ok(), "the edit broke the file rather than its meaning");
        std::fs::write(home.catalog_index(), &edited).expect("could not edit it");
        assert!(cached(&home).is_none(), "an edited index was believed");

        // And the half-written pair a run interrupted between the two writes
        // would leave. Refused for the same reason and by the same check.
        keep(&home, &published_pair());
        std::fs::remove_file(home.catalog_signature()).expect("it was just written");
        assert!(cached(&home).is_none(), "an index with no signature beside it was believed");
    }

    /// The three things the bar asks a catalog, in the states that separate
    /// them.
    ///
    /// `cached` and `stale` are not the same question and the design draws them
    /// apart: one is about the last refresh having failed and the other is
    /// about age. An index fetched a fortnight ago and confirmed a minute ago
    /// is neither.
    ///
    /// **The two ages are the bundle's and the line between them is not.** It
    /// draws 20 minutes as current (`InstallBar.dc.html:61`) and 12 days as
    /// stale (`ORNG Registry.dc.html:467`) and states nothing in between, so
    /// those two are what is pinned here. [`STALE_AFTER`] is ours and can move
    /// anywhere inside that bracket without this failing, which is the honest
    /// shape: asserting it against itself would prove only that a constant is
    /// equal to itself.
    #[test]
    fn what_the_bar_asks_a_catalog() {
        let minutes = Duration::from_secs(20 * 60);
        let days = Duration::from_secs(12 * 24 * 60 * 60);

        let fresh = Catalog::just_fetched(published());
        assert!(fresh.index().is_some());
        assert!(!fresh.is_cached(), "a catalog that just arrived was called cached");
        // A bound rather than `Duration::ZERO`. The age is read off the clock at
        // the moment the question is asked, so zero is true only where the two
        // calls land in one tick - which is what let this pass in release on one
        // machine and fail in debug on all three.
        assert!(
            matches!(fresh.freshness(), Freshness::Current(age) if age < Duration::from_secs(60)),
            "a catalog that just arrived is not current: {:?}",
            fresh.freshness()
        );
        assert!(!fresh.freshness().is_stale());

        // Old enough to be stated loudly, and offline with it.
        let old = Catalog::cached(published(), days, "no route to host");
        assert!(old.index().is_some(), "an offline window lost the index it had");
        assert!(old.is_cached());
        assert!(
            matches!(old.freshness(), Freshness::Stale(_)),
            "the index the design draws as stale is not stale here"
        );

        // Offline for twenty minutes is offline and is not old, which is the
        // two questions being two.
        let recent = Catalog::cached(published(), minutes, "no route to host");
        assert!(recent.is_cached());
        assert!(
            !recent.freshness().is_stale(),
            "the index the design draws as current is stale here"
        );

        // Nothing ever fetched: no index, no age, and stale - `:472` draws the
        // labelled control on exactly this state.
        let never = Catalog::unavailable("no route to host");
        assert!(never.index().is_none());
        assert!(!never.is_cached(), "a window with nothing held cannot be showing a cached one");
        assert_eq!(never.freshness(), Freshness::Never);
        assert!(never.freshness().is_stale());

        // And the one state that has no answer yet, which is neither a failure
        // nor a catalog.
        let waiting = Catalog::fetching();
        assert!(waiting.index().is_none());
        assert!(waiting.failure().is_none());
        assert_eq!(waiting.freshness(), Freshness::Never);
    }

    /// A refresh takes the refusals with it only when it brings back a
    /// *different* catalog, and not merely when it answers.
    ///
    /// A refusal is a claim about one row of the index it was made against, so
    /// a replaced index takes it along. Taking it on every answer would be
    /// worse than untidy: every launch confirms an index that has not moved,
    /// and would silently undo the user's last press.
    #[test]
    fn a_refresh_takes_the_refusals_only_with_a_catalog_that_is_not_the_one_already_held() {
        let item = published().items[0].uuid;
        let refused = || Refused::Download("the connection closed".to_owned());
        let holding = |index: Index| {
            let mut catalog = Catalog::just_fetched(index);
            catalog.refuse(item, refused());
            assert!(catalog.any_refused());
            catalog
        };

        let mut confirmed = holding(published()).answering(Ok(published()));
        confirmed.poll();
        assert_eq!(
            confirmed.refusal(item),
            Some(&refused()),
            "a refresh that confirmed the index took the refusals with it"
        );
        // It still moved the age, which is the other half of what a refresh is
        // for, and it cleared the offline state.
        assert!(!confirmed.is_cached());

        let mut emptied = published();
        emptied.items.clear();
        let mut changed = holding(published()).answering(Ok(emptied));
        changed.poll();
        assert_eq!(changed.refusal(item), None, "a different index kept the old one's refusals");
        assert!(!changed.any_refused());

        // And an install that answers after that refresh is not held against
        // the new index, which no longer has a row for it: the bar would read
        // `Install refused` with nothing under it to retry.
        changed.refuse(item, refused());
        assert!(!changed.any_refused(), "a refusal was held against an index without its item");

        let mut failed = holding(published()).answering(Err("no route to host".to_owned()));
        failed.poll();
        assert!(failed.refusal(item).is_some(), "a refresh that failed took the refusals");
        assert!(failed.index().is_some(), "a refresh that failed dropped the index it had");
        assert!(failed.is_cached());

        // And a poll with nothing in flight is not an answer at all, which is
        // every frame the window draws between refreshes.
        let mut idle = holding(published());
        idle.poll();
        assert!(idle.refusal(item).is_some());

        // Pressed again, the item is the row as it will be once tried, and not
        // the one that failed.
        idle.retry(item);
        assert!(!idle.any_refused());
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

    /// And every other character a URL gives a meaning to, which a document
    /// name is entitled to carry.
    ///
    /// A `#` is the one that costs most: it starts a fragment, so the request
    /// is cut short and whatever comes back is some other file. The digest
    /// check can only call that `Verification failed` - a trust event the
    /// design refuses to offer a retry for - over a name nobody escaped.
    #[test]
    fn a_document_name_is_escaped_rather_than_read_as_part_of_the_url() {
        let revision = Revision::new(&"a".repeat(40)).expect("a revision");
        let url = |name: &str| document_url(&revision, &format!("content/someone/{name}"));

        let fragment = url("GATE #2.bwdevice");
        assert!(fragment.ends_with("/GATE%20%232.bwdevice"), "{fragment}");
        let query = url("WHAT?.bwdevice");
        assert!(query.ends_with("/WHAT%3F.bwdevice"), "{query}");
        // Left alone, because these are what a file name is made of and
        // escaping them buys nothing a reader would thank us for.
        assert!(url("A-B_C.2~x.bwdevice").ends_with("/A-B_C.2~x.bwdevice"));
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
        let index = fetch().expect("the published index did not verify").index;
        let item = index.items.first().expect("the published catalog is empty");
        let revision = index.revision.clone().expect("a published index names its commit");
        let document = fetch_document(item, &revision).expect("the published document was refused");
        assert_eq!(document.identity().uuid, item.uuid);
    }
}
