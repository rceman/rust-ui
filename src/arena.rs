use crate::node::{Node, NodeId};

/// Retained arena: slot indices are recycled via a free list; `generation`
/// is checked (never wrapping) so a stale `NodeId` can never claim a reused
/// slot.
pub(crate) struct Arena {
    slots: Vec<Option<Node>>,
    generations: Vec<u64>,
    free: Vec<u32>,
}

impl Arena {
    pub(crate) fn new() -> Self {
        Arena {
            slots: Vec::new(),
            generations: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Allocate a node (fresh or reused slot). Generation is monotonic and
    /// checked — overflow panics rather than wrapping.
    pub(crate) fn alloc(&mut self, node: Node) -> NodeId {
        if let Some(slot) = self.free.pop() {
            debug_assert!(self.slots[slot as usize].is_none());
            self.slots[slot as usize] = Some(node);
            NodeId {
                slot,
                generation: self.generations[slot as usize],
            }
        } else {
            let slot = self.slots.len() as u32;
            self.slots.push(Some(node));
            self.generations.push(1);
            NodeId {
                slot,
                generation: 1,
            }
        }
    }

    /// Invalidate the id *before* teardown callbacks run — a queued event for
    /// the old generation is dead the moment removal begins.
    pub(crate) fn invalidate(&mut self, id: NodeId) {
        assert_eq!(
            self.generations[id.slot as usize], id.generation,
            "invalidating a stale NodeId"
        );
        let g = &mut self.generations[id.slot as usize];
        *g = g.checked_add(1).expect("node generation overflow");
    }

    /// Raw slot access during teardown — the generation is already dead by
    /// contract (invalidate ran first), so this bypasses the liveness check.
    /// Internal only; callers must hold the invalidation protocol.
    pub(crate) fn slot_mut(&mut self, slot: u32) -> Option<&mut Node> {
        self.slots.get_mut(slot as usize)?.as_mut()
    }
    /// live slot-vec length — capacity probe for recycling tests
    pub(crate) fn slot_len(&self) -> usize {
        self.slots.len()
    }

    /// live-checked slot read — ancestors are stored by slot
    pub(crate) fn slot(&self, slot: u32) -> Option<&Node> {
        self.slots.get(slot as usize)?.as_ref()
    }

    /// Take the node out after invalidation/teardown.
    pub(crate) fn free(&mut self, id: NodeId) {
        assert!(
            self.generations[id.slot as usize] != id.generation,
            "free must follow invalidate"
        );
        self.slots[id.slot as usize] = None;
        self.free.push(id.slot);
    }

    pub(crate) fn get(&self, id: NodeId) -> Option<&Node> {
        let live = self.generations.get(id.slot as usize)? == &id.generation;
        if !live {
            return None;
        }
        self.slots[id.slot as usize].as_ref()
    }

    pub(crate) fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        let live = self.generations.get(id.slot as usize)? == &id.generation;
        if !live {
            return None;
        }
        self.slots[id.slot as usize].as_mut()
    }

    /// Is the id live: generation match AND a node still in the slot — a
    /// freed slot's current generation must not appear live.
    pub(crate) fn is_live(&self, id: NodeId) -> bool {
        self.generations.get(id.slot as usize) == Some(&id.generation)
            && self
                .slots
                .get(id.slot as usize)
                .is_some_and(|s| s.is_some())
    }

    pub(crate) fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Current generation of a slot (live or dead).
    pub(crate) fn generation_of(&self, slot: u32) -> u64 {
        self.generations[slot as usize]
    }
}
