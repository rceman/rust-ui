use std::any::Any;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::geom::Point;
use crate::node::{NodeEvent, QueuedEvent};
use crate::text::{TextConflict, TextEdit, TextSelection};

/// Bounded native-event queue (128 entries). Overflow is a typed error —
/// only stale-generation events are ever silently dropped.
pub(crate) const EVENT_QUEUE_CAP: usize = 128;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

/// `button` is `None` for enter/leave (hover carries no button).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PointerEvent {
    pub position: Point,
    pub button: Option<PointerButton>,
    pub modifiers: Modifiers,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: Modifiers,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Key {
    Enter,
    Escape,
    Tab,
    Space,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Char(char),
    Other(u32),
}

/// Scheduled frame tick for `custom` nodes with `frame_events`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FrameTime {
    pub delta: Duration,
    pub absolute: Instant,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ScrollOffset {
    pub x: f32,
    pub y: f32,
}

/// Retained event factories — each produces an erased message boxed as
/// `dyn Any`; the node's `ui.scope` adapters lift it to the root `M`.
/// `M` is never required to be `Clone`: messages are manufactured on demand.
#[derive(Default)]
pub(crate) struct EventFactorySet {
    pub on_press: Option<Box<dyn Fn() -> Box<dyn Any>>>,
    pub on_pointer_enter: Option<Box<dyn Fn(PointerEvent) -> Box<dyn Any>>>,
    pub on_pointer_leave: Option<Box<dyn Fn(PointerEvent) -> Box<dyn Any>>>,
    pub on_pointer_down: Option<Box<dyn Fn(PointerEvent) -> Box<dyn Any>>>,
    pub on_pointer_up: Option<Box<dyn Fn(PointerEvent) -> Box<dyn Any>>>,
    pub on_focus: Option<Box<dyn Fn() -> Box<dyn Any>>>,
    pub on_blur: Option<Box<dyn Fn() -> Box<dyn Any>>>,
    pub on_key: Option<Box<dyn Fn(KeyEvent) -> Option<Box<dyn Any>>>>,
    pub on_edit: Option<Box<dyn Fn(TextEdit) -> Box<dyn Any>>>,
    pub on_submit: Option<Box<dyn Fn() -> Box<dyn Any>>>,
    pub on_selection_changed: Option<Box<dyn Fn(TextSelection) -> Box<dyn Any>>>,
    pub on_conflict: Option<Box<dyn Fn(TextConflict) -> Box<dyn Any>>>,
    pub on_frame: Option<Box<dyn Fn(FrameTime) -> Box<dyn Any>>>,
    pub on_open_change: Option<Box<dyn Fn(bool) -> Box<dyn Any>>>,
    pub on_scroll: Option<Box<dyn Fn(ScrollOffset) -> Box<dyn Any>>>,
}

impl EventFactorySet {
    /// Route one committed event through the retained factories. Returns the
    /// erased message or `None` when the node has no handler for it.
    pub(crate) fn dispatch(&self, ev: NodeEvent) -> Option<Box<dyn Any>> {
        match ev {
            NodeEvent::Press => self.on_press.as_ref().map(|f| f()),
            NodeEvent::Edit(e) => self.on_edit.as_ref().map(|f| f(e)),
            NodeEvent::Submit => self.on_submit.as_ref().map(|f| f()),
            NodeEvent::Selection(s) => self.on_selection_changed.as_ref().map(|f| f(s)),
            NodeEvent::Frame(t) => self.on_frame.as_ref().map(|f| f(t)),
            NodeEvent::Key(k) => self.on_key.as_ref().and_then(|f| f(k)),
            NodeEvent::Conflict(c) => self.on_conflict.as_ref().map(|f| f(c)),
            NodeEvent::Pointer(p, crate::node::PointerPhase::Enter) => {
                self.on_pointer_enter.as_ref().map(|f| f(p))
            }
            NodeEvent::Pointer(p, crate::node::PointerPhase::Leave) => {
                self.on_pointer_leave.as_ref().map(|f| f(p))
            }
            NodeEvent::Pointer(p, crate::node::PointerPhase::Down) => {
                self.on_pointer_down.as_ref().map(|f| f(p))
            }
            NodeEvent::Pointer(p, crate::node::PointerPhase::Up) => {
                self.on_pointer_up.as_ref().map(|f| f(p))
            }
            NodeEvent::Focus(true) => self.on_focus.as_ref().map(|f| f()),
            NodeEvent::Focus(false) => self.on_blur.as_ref().map(|f| f()),
            NodeEvent::OpenChange(o) => self.on_open_change.as_ref().map(|f| f(o)),
            NodeEvent::Scroll(s) => self.on_scroll.as_ref().map(|f| f(s)),
        }
    }
}

#[derive(Default)]
pub(crate) struct EventQueue {
    inner: VecDeque<QueuedEvent>,
}

impl EventQueue {
    pub(crate) fn new() -> Self {
        EventQueue::default()
    }

    pub(crate) fn push(&mut self, ev: QueuedEvent) -> Result<(), QueueOverflow> {
        if self.inner.len() >= EVENT_QUEUE_CAP {
            return Err(QueueOverflow);
        }
        self.inner.push_back(ev);
        Ok(())
    }

    pub(crate) fn pop(&mut self) -> Option<QueuedEvent> {
        self.inner.pop_front()
    }

    /// Teardown drain — drop every queued event.
    pub(crate) fn drain(&mut self) {
        self.inner.clear();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Drop every queued event for a dead slot (removal fences).
    pub(crate) fn purge_node(&mut self, slot: u32) -> u64 {
        let before = self.inner.len();
        self.inner.retain(|e| e.node.slot != slot);
        (before - self.inner.len()) as u64
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct QueueOverflow;
