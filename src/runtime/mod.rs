use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::arena::Arena;
use crate::event::{EVENT_QUEUE_CAP, EventQueue};
use crate::geom::Visibility;
use crate::key::{ChildKey, ErasedKey, KeyId};
use crate::node::{
    ContainerProps, KIND_GROUP, Node, NodeData, NodeEvent, NodeId, PeerDecision, QueuedEvent,
    TextPeer,
};
use crate::sched::{SchedEvent, Scheduler};
use crate::tasks::{
    CancelWatch, Envelope, Executor, Job, MAILBOX_CAP, Mailbox, ScopePath, TaskRegistry,
    TaskStartError,
};
use crate::text::{
    TextConflict, TextEdit, TextRevision, TextValue, lease_id_of, validate_selection,
};
use crate::theme::{Appearance, ReducedMotion, Theme};
use crate::ui::{StagedNode, Tx, Ui, UiFrame};
use crate::{UiDiagnostic, UiError, UiResult};

/// Update context handed to `update` — task spawning, cancellation and
/// scoped namespaces. No view/arena access from here.
pub struct UpdateCtx<'cx, M> {
    registry: &'cx mut TaskRegistry,
    executor: &'cx Option<Arc<dyn Executor>>,
    mailbox: &'cx Arc<Mailbox>,
    scope: ScopePath,
    _m: std::marker::PhantomData<M>,
}

/// `spawn` is the only `M: Send` gate — worker bodies ship thread-side.
impl<'cx, M: Send + 'static> UpdateCtx<'cx, M> {
    /// Spawn a keyed task; `key` is (scope-path, key) namespaced — any prior
    /// registration is fenced synchronously before the new future starts.
    /// `M: Send` is required only here — the root `M` itself needn't be Send
    /// for apps that never spawn tasks.
    pub fn spawn<Fut>(
        &mut self,
        key: impl KeyId,
        job: impl FnOnce(Job<M>) -> Fut + Send + 'static,
    ) -> Result<(), TaskStartError>
    where
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let Some(executor) = self.executor else {
            return Err(TaskStartError::NoExecutor);
        };
        let executor = executor.clone();
        let (j, reg_id) = self.registry.spawn::<M>(&self.scope, key, self.mailbox)?;
        let (cancelled, completed, cancel_waiters) = self.registry.job_bits(reg_id);
        let fut = CancelWatch {
            inner: Box::pin(job(j)),
            cancelled,
            completed,
            cancel_waiters,
            mailbox: self.mailbox.clone(),
            watch_registered: false,
        };
        if executor.spawn(Box::pin(fut)).is_err() {
            // executor refused — fence the fresh registration so nothing leaks
            self.registry.cancel_reg(reg_id, self.mailbox);
            return Err(TaskStartError::Rejected);
        }
        Ok(())
    }
}

/// `cancel`/`scope` work for any `M` — they never touch the worker side.
impl<'cx, M: 'static> UpdateCtx<'cx, M> {
    /// Cancel the live task registered under `key` (scope-local).
    pub fn cancel(&mut self, key: impl KeyId) {
        self.registry.cancel(&self.scope, key, self.mailbox);
    }

    /// `cx.scope(key, map)` — child namespace: `spawn`/`cancel` keys are
    /// independent per scope path, and child messages lift through `map`.
    /// Scope adapters run UI-side only — no `Send`/`Sync` bound, so an
    /// `Rc` capture or a non-`Send` root `M` typechecks.
    pub fn scope<ChildMsg: 'static>(
        &mut self,
        key: impl KeyId,
        map: impl Fn(ChildMsg) -> M + 'static,
    ) -> UpdateCtx<'_, ChildMsg> {
        let lift: Rc<dyn Fn(Box<dyn Any>) -> Box<dyn Any>> =
            Rc::new(move |c| Box::new(map(*c.downcast::<ChildMsg>().unwrap())) as Box<dyn Any>);
        UpdateCtx {
            registry: self.registry,
            executor: self.executor,
            mailbox: self.mailbox,
            scope: self.scope.push(ErasedKey::new(key), lift),
            _m: std::marker::PhantomData,
        }
    }
}

/// The retained core: ONE arena + flat reusable staging buffers + bounded
/// task/mailbox delivery + a narrow chrome scheduler.
pub(crate) struct Runtime<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
    pub(crate) state: S,
    update: U,
    view: V,
    pub(crate) arena: Arena,
    pub(crate) root: NodeId,
    pub(crate) events: EventQueue,
    pub(crate) mailbox: Arc<Mailbox>,
    pub(crate) registry: TaskRegistry,
    pub(crate) executor: Option<Arc<dyn Executor>>,
    pub(crate) sched: Scheduler,
    pub(crate) theme: Theme,
    /// actual OS appearance — independent of the selected theme
    appearance: Appearance,
    /// platform-supplied system-color lookup used ONLY when
    /// `appearance.forced_colors` — the classifier resolves roles to real
    /// system RGB so a theme/HC flip diffs honestly
    forced_resolver: Option<Box<dyn Fn(crate::style::SystemColor) -> [f32; 4]>>,
    /// platform OS reduced-motion probe — resolves an authored
    /// `ReducedMotion::System` at every re-resolution point; None = no OS
    /// signal (System behaves as NoPreference)
    motion_resolver: Option<Box<dyn Fn() -> bool>>,
    /// the AUTHORED reduced-motion policy — `theme.reduced_motion` keeps
    /// the authored value (System stays System) so an OS change can
    /// re-resolve it; `sched.reduced` holds the resolved bool
    authored_reduced: crate::theme::ReducedMotion,
    appearance_dirty: bool,
    /// how many times `view` has been evaluated
    pub(crate) view_count: u64,
    /// mounted-editor index: lease identity -> node (O(1) move detection)
    mounted_editors: HashMap<usize, NodeId>,
    /// flat reusable transaction buffer — allocated once per runtime
    scratch_nodes: Vec<StagedNode>,
    /// typed diagnostics collected across commits
    pub(crate) diagnostics: Vec<UiDiagnostic>,
    /// stale/foreign/out-of-order events dropped this session
    pub(crate) dropped_stale_events: u64,
    pub(crate) rejected_edits: u64,
    /// composing-snapshot selection drops
    pub(crate) suppressed_selections: u64,
    /// an enqueue overflowed during commit — surfaced as QueueOverflow
    last_queue_error: bool,
    peer_factory: Option<Box<dyn Fn(crate::node::PeerSpec) -> crate::UiResult<Box<dyn TextPeer>>>>,
    /// chrome scheduler events produced between polls (policy flips)
    chrome_backlog: Vec<SchedEvent>,
    _m: std::marker::PhantomData<fn() -> M>,
}

impl<S, M, U, V> Runtime<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
}

/// Whole-transaction validation — every `TextValue` lease must bind at most
/// one staged editor; O(n) via a hash set, runs before any peer mutation.
/// Guaranteed teardown — every error path and normal drop runs shutdown:
/// peers released once, leases cleared, queues fenced.
impl<S, M, U, V> Drop for Runtime<S, M, U, V>
where
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn validate_bindings(tx: &Tx) -> Option<UiDiagnostic> {
    let mut seen: HashSet<usize> = HashSet::new();
    for n in &tx.nodes {
        if let Some(lid) = n.lease_id()
            && !seen.insert(lid)
        {
            return Some(UiDiagnostic::DuplicateTextBinding);
        }
    }
    None
}

/// Style preflight — every staged style is validated BEFORE any native
/// mutation: finite numbers, nonnegative geometry, positive text sizes,
/// valid radii/shadow -> `InvalidUi(InvalidStyle)`; a nonopaque editor
/// backing/foreground (impossible platform capability) -> `Unsupported`.
fn validate_styles(tx: &Tx, dark: bool) -> Option<UiError> {
    for n in &tx.nodes {
        let ok = match &n.data {
            NodeData::Container { kind, props } => {
                let patch_ok = crate::style::box_patch_ok(&props.patch);
                let full_ok = props.full.is_none_or(|f| crate::style::box_style_ok(&f));
                let resolved_ok = props
                    .resolved_box(*kind)
                    .is_none_or(|b| crate::style::box_style_ok(&b));
                patch_ok && full_ok && resolved_ok
            }
            NodeData::Label { patch, .. } => crate::style::text_patch_ok(patch),
            NodeData::Button {
                style,
                variant,
                size,
                invalid,
                selected,
                ..
            } => {
                crate::style::button_patch_ok(style)
                    && crate::style::visual_style_ok(&crate::style::resolve_button(
                        *variant,
                        *size,
                        style,
                        crate::style::StyleState::Normal,
                        false,
                        false,
                        *selected,
                        *invalid,
                    ))
            }
            NodeData::Editor { patch, .. } => {
                if !crate::style::text_input_patch_ok(patch) {
                    return Some(UiError::InvalidUi(UiDiagnostic::InvalidStyle));
                }
                // opaque rule — checked against the RESOLVED colors
                let chrome = crate::style::resolve_text_input_chrome(patch);
                let fg = patch.foreground.unwrap_or(crate::style::Color::Role(
                    crate::theme::ColorRole::Foreground,
                ));
                if !crate::style::text_input_opaque(&chrome, fg, dark) {
                    return Some(UiError::Unsupported(
                        "native text requires opaque backing and foreground".into(),
                    ));
                }
                true
            }
            NodeData::Popup { props, .. } => {
                crate::style::box_patch_ok(&props.patch)
                    && props
                        .resolved_box(crate::node::KIND_POPUP)
                        .is_none_or(|b| crate::style::box_style_ok(&b))
            }
            NodeData::Action { style, .. } => crate::style::action_style_ok(style),
            _ => true,
        };
        if !ok {
            return Some(UiError::InvalidUi(UiDiagnostic::InvalidStyle));
        }
    }
    None
}

pub(crate) mod impl_a;
pub(crate) mod impl_b;
pub(crate) mod impl_c;
pub(crate) mod impl_d;
