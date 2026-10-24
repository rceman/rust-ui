
impl StagedNode {
    /// Slot placeholder while a node is moved out of the flat buffer.
    pub(crate) fn placeholder() -> Self {
        StagedNode {
            automation_id: None,
            part_key: None,
            role: None,
            key: ChildKey::Static {
                kind: 0,
                ordinal: u32::MAX,
            },
            data: NodeData::Empty,
            presented: Presented::default(),
            visibility: Visibility::Hidden,
            layout: LayoutSpec::default(),
            factories: EventFactorySet::default(),
            adapters: Rc::new(Vec::new()),
            children: Vec::new(),
        }
    }

    pub(crate) fn kind_tag(&self) -> u8 {
        match &self.data {
            NodeData::Empty => 0,
            NodeData::Container { kind, .. } => *kind,
            NodeData::Label { .. } => crate::node::KIND_LABEL,
            NodeData::Button { .. } => crate::node::KIND_BUTTON,
            NodeData::Editor { multiline, .. } => {
                if *multiline {
                    crate::node::KIND_TEXT_AREA
                } else {
                    crate::node::KIND_TEXT_INPUT
                }
            }
            NodeData::Custom { .. } => crate::node::KIND_CUSTOM,
            NodeData::Action { .. } => crate::node::KIND_ACTION,
            NodeData::Icon { .. } => crate::node::KIND_ICON,
            NodeData::Toggle { .. } => crate::node::KIND_TOGGLE,
            NodeData::Popup { .. } => crate::node::KIND_POPUP,
        }
    }

    /// lease identity for the mounted-editor index / duplicate checks
    pub(crate) fn lease_id(&self) -> Option<usize> {
        match &self.data {
            NodeData::Editor { snapshot, .. } => Some(Rc::as_ptr(&snapshot.lease) as usize),
            _ => None,
        }
    }
}

/// One frame = one parent's staging context. `retained` is the matched
/// retained node the frame corresponds to — its children drive O(1) matching
/// and Rc reuse.
pub(crate) struct UiFrame {
    /// child indices into `Tx.nodes` (emission order)
    pub children: Vec<u32>,
    /// single counter for static (no explicit key) siblings — identity is
    /// (kind, emission ordinal)
    pub static_ordinals: u32,
    /// duplicate detection bucketed by key hash — real equality inside the
    /// bucket, never hash-as-identity
    pub key_buckets: HashMap<u64, Vec<ErasedKey>>,
    /// the retained node this frame is staging against (None = no prior)
    pub retained: Option<NodeId>,
    /// retained children bucketed by key hash — built lazily on first match
    pub retained_buckets: Option<HashMap<u64, Vec<u32>>>,
}

impl UiFrame {
    /// Allocate the next static-sibling identity: single emission counter +
    /// kind tag (NOT a per-kind counter).
    fn child_key(
        &mut self,
        kind: u8,
        explicit: Option<ErasedKey>,
    ) -> Result<ChildKey, UiDiagnostic> {
        match explicit {
            Some(k) => {
                let h = k.hash();
                if let Some(bucket) = self.key_buckets.get(&h) {
                    // true equality — collisions can't alias
                    if bucket.iter().any(|e| e.key_eq(&k)) {
                        return Err(UiDiagnostic::DuplicateKey);
                    }
                }
                self.key_buckets.entry(h).or_default().push(k.clone());
                Ok(ChildKey::Explicit(k))
            }
            None => {
                let o = self.static_ordinals;
                self.static_ordinals += 1;
                Ok(ChildKey::Static { kind, ordinal: o })
            }
        }
    }
}

/// Non-generic transaction staging — flat node buffer + frame stack; the
/// retained arena is read-only through `view` and used for matching + Rc
/// reuse. Buffers are cleared (capacity retained) each view pass.
pub(crate) struct Tx<'r> {
    /// >0 while an `ui.action` draw closure runs — actionable/native-peer
    /// children staged inside are InvalidComposition diagnostics
    pub(crate) action_depth: u32,
    /// flat staged node arena — `StagedNode.children` index into this
    pub nodes: Vec<StagedNode>,
    pub frames: Vec<UiFrame>,
    /// active scope-adapter chain (leaf -> root)
    pub adapters: MsgAdapters,
    /// current window theme (staged changes propagate at commit)
    pub theme: Theme,
    /// OS appearance — supplied by the runtime, independent of `theme`
    pub appearance: Appearance,
    /// diagnostics staged by builders (fatal structural errors)
    pub diagnostics: Vec<UiDiagnostic>,
    /// read-only retained arena for matching + `Rc<str>` reuse
    pub retained: &'r Arena,
}

impl Tx<'_> {
    /// The retained node a frame is staging against, and its children.
    fn retained_children(&self, frame: &UiFrame) -> &[u32] {
        frame
            .retained
            .and_then(|id| self.retained.get(id))
            .map(|n| n.children.as_slice())
            .unwrap_or(&[])
    }

    /// Find the retained child matching `key`/`kind` in the current frame —
    /// hash-bucketed, equality-confirmed; never a quadratic sibling scan.
    fn match_retained(&mut self, frame_idx: usize, key: &ChildKey, kind: u8) -> Option<NodeId> {
        let h = key.hash();
        if self.frames[frame_idx].retained_buckets.is_none() {
            let frame = &self.frames[frame_idx];
            let mut buckets: HashMap<u64, Vec<u32>> = HashMap::new();
            for &slot in self.retained_children(frame) {
                let g = self.retained.generation_of(slot);
                if let Some(n) = self.retained.get(NodeId {
                    slot,
                    generation: g,
                }) {
                    buckets.entry(n.key.hash()).or_default().push(slot);
                }
            }
            self.frames[frame_idx].retained_buckets = Some(buckets);
        }
        let buckets = self.frames[frame_idx].retained_buckets.as_ref().unwrap();
        for &slot in buckets.get(&h).into_iter().flatten() {
            let g = self.retained.generation_of(slot);
            if let Some(n) = self.retained.get(NodeId {
                slot,
                generation: g,
            }) && n.key.key_eq(key)
                && n.kind_tag() == kind
            {
                return Some(NodeId {
                    slot,
                    generation: g,
                });
            }
        }
        None
    }

    /// Stage a leaf: key allocation + retained match + push into flat buffer.
    /// `build` gets the matched retained node (for Rc reuse).
    pub(crate) fn stage_leaf(
        &mut self,
        kind: u8,
        explicit: Option<ErasedKey>,
        visibility: Visibility,
        layout: LayoutSpec,
        factories: EventFactorySet,
        build: impl FnOnce(Option<&crate::node::Node>) -> NodeData,
    ) {
        if let Some(d) = layout.validate() {
            self.diagnostics.push(d);
            return;
        }
        // action content is decorative — no actionable/focusable/native
        // peer children inside a semantic action
        if self.action_depth > 0
            && matches!(
                kind,
                crate::node::KIND_BUTTON
                    | crate::node::KIND_ACTION
                    | crate::node::KIND_TEXT_INPUT
                    | crate::node::KIND_TEXT_AREA
                    | crate::node::KIND_TOGGLE
                    | crate::node::KIND_POPUP
            )
        {
            self.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let frame_idx = self.frames.len() - 1;
        let key = match self.frames[frame_idx].child_key(kind, explicit) {
            Ok(k) => k,
            Err(d) => {
                self.diagnostics.push(d);
                return;
            }
        };
        let retained_node = self
            .match_retained(frame_idx, &key, kind)
            .and_then(|id| self.retained.get(id));
        let data = build(retained_node);
        let idx = self.nodes.len() as u32;
        self.nodes.push(StagedNode {
            automation_id: None,
            part_key: None,
            role: None,
            key,
            data,
            presented: Presented::default(),
            visibility,
            layout,
            factories,
            adapters: self.adapters.clone(),
            children: Vec::new(),
        });
        self.frames[frame_idx].children.push(idx);
    }

    /// Stage a container: draw children into a fresh frame against the
    /// matched retained node, then append the container. `build` produces
    /// the node data (Container for layout kinds, Action for `ui.action`).
    fn stage_container<M: 'static>(
        &mut self,
        kind: u8,
        explicit: Option<ErasedKey>,
        props: ContainerProps,
        layout: LayoutSpec,
        factories: EventFactorySet,
        visibility: Visibility,
        draw: impl FnOnce(&mut Ui<'_, '_, M>),
        action_depth: bool,
        build: impl FnOnce(ContainerProps) -> NodeData,
    ) {
        if let Some(d) = layout.validate() {
            self.diagnostics.push(d);
            return;
        }
        // nested actionable inside an action's decorative content is a
        // structural diagnostic, not event bubbling
        if self.action_depth > 0
            && matches!(
                kind,
                crate::node::KIND_BUTTON
                    | crate::node::KIND_ACTION
                    | crate::node::KIND_TEXT_INPUT
                    | crate::node::KIND_TEXT_AREA
                    | crate::node::KIND_POPUP
            )
        {
            self.diagnostics.push(UiDiagnostic::InvalidComposition);
            return;
        }
        let frame_idx = self.frames.len() - 1;
        let key = match self.frames[frame_idx].child_key(kind, explicit) {
            Ok(k) => k,
            Err(d) => {
                self.diagnostics.push(d);
                return;
            }
        };
        let retained = self.match_retained(frame_idx, &key, kind);
        // push a child frame bound to the matched retained node
        self.frames.push(UiFrame {
            children: Vec::new(),
            static_ordinals: 0,
            retained_buckets: None,
            key_buckets: HashMap::new(),
            retained,
        });
        if action_depth {
            self.action_depth += 1;
        }
        // hand a Ui view of the same Tx to draw — self re-borrowed
        {
            let mut ui = Ui::<M> {
                tx: self,
                _m: std::marker::PhantomData,
            };
            draw(&mut ui);
        }
        if action_depth {
            self.action_depth -= 1;
        }
        let child_frame = self.frames.pop().unwrap();
        let data = build(props);
        // leaf kinds staged inside an action are checked at stage_leaf;
        // the container itself inside an action is still invalid
        let idx = self.nodes.len() as u32;
        self.nodes.push(StagedNode {
            automation_id: None,
            part_key: None,
            role: None,
            key,
            data,
            presented: Presented::default(),
            visibility,
            layout,
            factories,
            adapters: self.adapters.clone(),
            children: child_frame.children,
        });
        self.frames[frame_idx].children.push(idx);
    }
}

