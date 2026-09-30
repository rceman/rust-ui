use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonic committed revision. Crate-private payload — opaque publicly.
/// Every committed/requested revision and every binding token is minted from
/// ONE checked global serial, so native result revisions, programmatic
/// request revisions and binding identities can never collide or repeat.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TextRevision(u64);

/// Opaque mount-binding identity — one per mounted editable peer.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BindingToken(u64);

static TEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_serial() -> u64 {
    TEXT_SERIAL
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_add(1))
        .expect("text serial overflow")
}

impl TextRevision {
    pub(crate) fn mint() -> Self {
        TextRevision(next_serial())
    }

    #[cfg(test)]
    pub(crate) fn raw(v: u64) -> Self {
        TextRevision(v)
    }

    pub(crate) fn raw_val(&self) -> u64 {
        self.0
    }
}

impl BindingToken {
    pub(crate) fn mint() -> Self {
        BindingToken(next_serial())
    }

    #[cfg(test)]
    pub(crate) fn raw(v: u64) -> Self {
        BindingToken(v)
    }
}

/// Who produced an edit/commit — the peer (native) or the app (programmatic).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EditOrigin {
    /// committed by the native peer (typing, IME, paste, built-in ops)
    NativePeer,
    /// applied set-text acknowledged by the peer (NOT a user edit)
    Programmatic,
}

/// A committed text change — the payload of `on_edit`.
/// `result` is the committed revision; `base` the revision it built on.
#[derive(Clone, Debug)]
pub struct TextEdit {
    /// committed UTF-8 text after the edit
    pub(crate) text: String,
    /// revision the edit was based on
    pub(crate) base: TextRevision,
    /// committed result revision
    pub(crate) result: TextRevision,
    /// which side committed it
    pub(crate) origin: EditOrigin,
    /// the mount binding that produced it
    pub(crate) binding: BindingToken,
}

impl TextEdit {
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn base_revision(&self) -> TextRevision {
        self.base
    }
    pub fn result_revision(&self) -> TextRevision {
        self.result
    }
    pub fn origin(&self) -> EditOrigin {
        self.origin
    }
    pub fn binding(&self) -> BindingToken {
        self.binding
    }
}

/// Emitted when a native commit lands on a diverged base while a programmatic
/// request was in flight — the request is rejected, native wins.
#[derive(Clone, Debug)]
pub struct TextConflict {
    /// the programmatic request that was superseded
    pub rejected_revision: TextRevision,
    pub rejected_text: String,
    /// the peer's committed snapshot that won
    pub committed_revision: TextRevision,
    pub committed_text: String,
    /// the binding that owns the conflict
    pub binding: BindingToken,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AcceptOutcome {
    /// applied: committed advanced to `result`
    Applied,
    /// ignored: stale, out-of-order or already-committed result
    IgnoredStale,
    /// ignored: wrong binding token (foreign or pre-remount callback)
    IgnoredOrigin,
}

/// Selection attached to the committed revision it refers to; byte offsets
/// are valid UTF-8 boundaries into `TextValue::text()`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TextSelection {
    pub revision: TextRevision,
    /// anchor/focus are BYTE offsets into the UTF-8 committed snapshot
    pub anchor: usize,
    pub focus: usize,
}

/// A single programmatic intent — replace/clear. `Rc` shared into the staged
/// node snapshot so a view pass never copies the text.
#[derive(Debug)]
pub(crate) struct Proposal {
    /// revision the app observed when it issued the request
    pub base: TextRevision,
    /// unique requested revision (global serial)
    pub requested: TextRevision,
    pub text: Rc<str>,
}

/// Mount lease: the ONLY shared bookkeeping between `TextValue` and a mounted
/// peer. `Some(token)` = mounted; the peer stamps every native callback with
/// it, and remount issues a fresh token so old-origin callbacks are fenced.
pub(crate) type LeaseCell = Rc<Cell<Option<BindingToken>>>;

/// Immutable staged snapshot of a `TextValue` — clone of Rc handles only,
/// no string copying. This is what the commit path works against.
#[derive(Clone)]
pub(crate) struct TextSnapshot {
    pub lease: LeaseCell,
    pub committed: Rc<str>,
    pub revision: TextRevision,
    pub pending: Option<Rc<Proposal>>,
}

/// One-way value: `text()`/`revision()` read the last ACKNOWLEDGED snapshot;
/// `replace`/`clear` express intent (never immediate app-visible change —
/// the staged proposal applies at commit and acknowledges as `Programmatic`).
///
/// `TextValue` deliberately has no public `Clone` — a value binds to at most
/// one mounted editable peer; internal snapshots share Rc handles only.
pub struct TextValue {
    committed: Rc<str>,
    revision: TextRevision,
    /// newest programmatic intent (only one proposal exists at a time)
    pending: Option<Rc<Proposal>>,
    /// intent serial survives clearing/tombstoning — a later request never
    /// reuses an older requested revision (global mint guarantees it)
    lease: LeaseCell,
    /// the request rejected by the last keep_native — retained so a stale
    /// conflict can never erase a NEWER queued proposal
    tombstone: Option<TextRevision>,
}

impl TextValue {
    pub fn new(text: impl Into<String>) -> Self {
        TextValue {
            committed: Rc::from(text.into().as_str()),
            revision: TextRevision::mint(),
            pending: None,
            lease: Rc::new(Cell::new(None)),
            tombstone: None,
        }
    }

    /// Last ACKNOWLEDGED committed text — `&str` view, no clone.
    pub fn text(&self) -> &str {
        &self.committed
    }

    pub fn revision(&self) -> TextRevision {
        self.revision
    }

    pub fn is_empty(&self) -> bool {
        self.committed.is_empty()
    }

    /// The peer committed `edit` — apply iff it is current-binding, in-order
    /// (base == acknowledged revision) and moves forward (result > revision).
    /// Foreign, stale and out-of-order edits are ignored — never clamped.
    pub fn accept(&mut self, edit: TextEdit) -> AcceptOutcome {
        if self.lease.get() != Some(edit.binding) {
            return AcceptOutcome::IgnoredOrigin;
        }
        if edit.base != self.revision || edit.result <= self.revision {
            return AcceptOutcome::IgnoredStale;
        }
        self.committed = Rc::from(edit.text.as_str());
        self.revision = edit.result;
        // a Programmatic acknowledgement clears only the matching proposal
        if edit.origin == EditOrigin::Programmatic
            && self
                .pending
                .as_ref()
                .is_some_and(|p| p.requested == edit.result)
        {
            self.pending = None;
        }
        AcceptOutcome::Applied
    }

    /// Programmatic intent: replace with `text`. Returns the unique requested
    /// revision. The acknowledged `text()` does not change until the peer's
    /// `Programmatic` acknowledgement is accepted.
    pub fn replace(&mut self, text: impl Into<String>) -> TextRevision {
        let requested = TextRevision::mint();
        self.pending = Some(Rc::new(Proposal {
            base: self.revision,
            requested,
            text: Rc::from(text.into().as_str()),
        }));
        requested
    }

    /// Programmatic intent: clear the control.
    pub fn clear(&mut self) -> TextRevision {
        self.replace("")
    }

    /// The outstanding proposal (crate/test visibility).
    pub(crate) fn pending(&self) -> Option<&Proposal> {
        self.pending.as_ref().map(|p| &**p)
    }

    /// Reconcile after a conflict: accept the peer's committed snapshot and
    /// tombstone the matching request. Requires the live binding AND the
    /// exact pending request — a stale conflict cannot alter committed text
    /// or erase a newer proposal.
    pub fn keep_native(&mut self, conflict: TextConflict) -> AcceptOutcome {
        if self.lease.get() != Some(conflict.binding) {
            return AcceptOutcome::IgnoredOrigin;
        }
        let Some(p) = &self.pending else {
            return AcceptOutcome::IgnoredStale;
        };
        if p.requested != conflict.rejected_revision {
            return AcceptOutcome::IgnoredStale;
        }
        // a conflict snapshot emitted before newer acknowledged native
        // commits must not roll them back
        if conflict.committed_revision < self.revision {
            return AcceptOutcome::IgnoredStale;
        }
        // tombstone first, then adopt the peer's winning snapshot
        self.tombstone = Some(p.requested);
        self.pending = None;
        self.committed = Rc::from(conflict.committed_text.as_str());
        self.revision = conflict.committed_revision;
        AcceptOutcome::Applied
    }

    // ----- internal (peer/runtime plumbing) -----

    pub(crate) fn snapshot(&self) -> TextSnapshot {
        TextSnapshot {
            lease: self.lease.clone(),
            committed: self.committed.clone(),
            revision: self.revision,
            pending: self.pending.clone(),
        }
    }

    /// Was this pending request already rejected? Tombstone survives until a
    /// NEW intent replaces `pending` (which drops the tombstone match by
    /// having a fresh requested serial anyway).
    pub(crate) fn is_tombstoned(&self, requested: TextRevision) -> bool {
        self.tombstone == Some(requested)
    }

    /// Stage-time binding acquire: lease free => claim + return token.
    /// Duplicate mounts of the same value are rejected BEFORE commit.
    pub(crate) fn acquire_binding(&self) -> Option<BindingToken> {
        if self.lease.get().is_some() {
            return None;
        }
        let t = BindingToken::mint();
        self.lease.set(Some(t));
        Some(t)
    }

    /// Runtime-side acquire (lease cell only — used by the arena path which
    /// holds the snapshot's lease, not the value itself).
    pub(crate) fn acquire_lease(lease: &LeaseCell) -> Option<BindingToken> {
        if lease.get().is_some() {
            return None;
        }
        let t = BindingToken::mint();
        lease.set(Some(t));
        Some(t)
    }

    pub(crate) fn release_lease(lease: &LeaseCell, token: BindingToken) {
        if lease.get() == Some(token) {
            lease.set(None);
        }
    }

    pub(crate) fn lease_cell(&self) -> LeaseCell {
        self.lease.clone()
    }
}

impl Default for TextValue {
    fn default() -> Self {
        Self::new("")
    }
}

// ---------- checked UTF-16 <-> UTF-8 conversion boundary ----------

/// Native peers speak UTF-16. Conversion failures are typed — never silent
/// lossy coercion and never a wrong byte offset.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TextPeerError {
    InvalidUtf16,
    OutOfBounds,
}

pub(crate) fn utf16_to_utf8(units: &[u16]) -> Result<String, TextPeerError> {
    String::from_utf16(units).map_err(|_| TextPeerError::InvalidUtf16)
}

/// UTF-16 unit index -> UTF-8 byte index; `None` when out of range or inside
/// a surrogate pair (or a non-char-boundary position).
pub(crate) fn utf16_index_to_utf8(text: &str, utf16_index: usize) -> Option<usize> {
    let mut units = 0usize;
    for (byte, ch) in text.char_indices() {
        if units == utf16_index {
            return Some(byte);
        }
        units += ch.len_utf16();
        if units > utf16_index {
            return None; // inside the char's UTF-16 encoding
        }
    }
    if units == utf16_index {
        Some(text.len())
    } else {
        None
    }
}

/// UTF-8 byte index -> UTF-16 unit index; `None` on non-char-boundary.
pub(crate) fn utf8_index_to_utf16(text: &str, byte_index: usize) -> Option<usize> {
    let mut units = 0usize;
    for (byte, ch) in text.char_indices() {
        if byte == byte_index {
            return Some(units);
        }
        units += ch.len_utf16();
    }
    if byte_index == text.len() {
        Some(units)
    } else {
        None
    }
}

/// Selection validity: committed-revision match + char-boundary byte offsets.
pub(crate) fn validate_selection(
    sel: TextSelection,
    committed_rev: TextRevision,
    committed: &str,
) -> bool {
    if sel.revision != committed_rev {
        return false;
    }
    for off in [sel.anchor, sel.focus] {
        if off > committed.len() || !committed.is_char_boundary(off) {
            return false;
        }
    }
    true
}

/// Lease-pointer identity for the runtime's mounted-editor index.
pub(crate) fn lease_id_of(lease: &LeaseCell) -> usize {
    Rc::as_ptr(lease) as usize
}
