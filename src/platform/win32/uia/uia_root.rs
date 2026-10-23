impl UiaRoot {
    pub(crate) fn new(hwnd: HWND, title: &str) -> windows::core::Result<UiaRoot> {
        let snap = Arc::new(Snapshot {
            closed: std::sync::atomic::AtomicBool::new(false),
            live: Mutex::new(HashMap::new()),
            order: Mutex::new(Vec::new()),
            children: Mutex::new(HashMap::new()),
            hwnd,
            post_hwnd: hwnd,
            focus: Mutex::new(None),
        });
        let site_co = windows_core::ComObject::new(WindowlessSite {
            snap: snap.clone(),
            root: Mutex::new(Weak::new()),
            node: NodeId {
                slot: u32::MAX,
                generation: 0,
            },
        });
        let root_co = windows_core::ComObject::new(RootFragment {
            snap: snap.clone(),
            hwnd,
            name: title.to_string(),
            me_root: Mutex::new(Weak::new()),
            me_frag: Mutex::new(Weak::new()),
        });
        let root: IRawElementProviderFragmentRoot = root_co.to_interface();
        let root_frag: IRawElementProviderFragment = root_co.to_interface();
        // self-references via Weak — no strong root->self cycle
        *root_co.get().me_root.lock().unwrap() = root.downgrade()?;
        *root_co.get().me_frag.lock().unwrap() = root_frag.downgrade()?;
        // site answers child->Parent with the root — Weak, not owned
        *site_co.get().root.lock().unwrap() = root_frag.downgrade()?;
        Ok(UiaRoot {
            snap,
            root,
            root_frag,
            native: Mutex::new(HashMap::new()),
        })
    }

    /// The site a native editor provider calls back through — built lazily
    /// (kept inside UiaRoot to share one site across editors).
    pub(crate) fn site(
        &self,
        node: NodeId,
    ) -> windows::core::Result<IRawElementProviderWindowlessSite> {
        let site_co = windows_core::ComObject::new(WindowlessSite {
            snap: self.snap.clone(),
            root: Mutex::new(Weak::new()),
            node,
        });
        let site: IRawElementProviderWindowlessSite = site_co.to_interface();
        *site_co.get().root.lock().unwrap() = self.root_frag.downgrade()?;
        Ok(site)
    }

    /// Rebuild the live set + active-order children from the retained tree.
    /// Surviving NodeIds keep their provider object — its live state is
    /// updated in place (name/enabled/bounds/actionable follow the real
    /// tree between WM_GETOBJECT calls). Dead ids drop their handle, the
    /// order weak dies, and external references fail `live_ok`. A same-
    /// slot different-generation id is a NEW association — the retired
    /// fragment can never act for its replacement.
    ///
    /// Notifications: structure changes raise `ChildrenInvalidated` on the
    /// root; enabled flips raise `IsEnabled` property-change on the live
    /// fragment. Raised AFTER the new state is installed.
    pub(crate) fn rebuild(&self, children: Vec<ChildBuild>) -> windows::core::Result<()> {
        let mut live = HashMap::new();
        let mut order = Vec::new();
        let mut structure_changed = false;
        let mut enabled_flips: Vec<(IRawElementProviderFragment, bool)> = Vec::new();
        let mut offscreen_flips: Vec<(IRawElementProviderFragment, bool)> = Vec::new();
        let mut name_changes: Vec<(IRawElementProviderFragment, String, String)> = Vec::new();
        let mut bounds_changes: Vec<(IRawElementProviderFragment, UiaRect, UiaRect)> = Vec::new();
        // reorder detection: same set, different sequence is a structure
        // change UIA clients must hear
        let prev_order: Vec<NodeId> = self
            .snap
            .order
            .lock()
            .unwrap()
            .iter()
            .map(|(id, _)| *id)
            .collect();
        {
            let mut table = self.snap.children.lock().unwrap();
            for k in children {
                live.insert(k.id.slot, k.id.generation);
                // reuse the surviving association when the NodeId matches —
                // state updates in place (no provider churn per rebuild)
                if let Some(h) = table.get(&k.id) {
                    *h.state.automation_id.lock().unwrap() = k.automation_id.clone();
                    // name/bounds updates notify — silent value drift is a
                    // live-contract violation
                    {
                        let mut name = h.state.name.lock().unwrap();
                        if *name != k.name {
                            let old = std::mem::replace(&mut *name, k.name.clone());
                            name_changes.push((h.frag.clone(), old, k.name.clone()));
                        }
                    }
                    {
                        let mut rect = h.state.rect.lock().unwrap();
                        if *rect != k.rect {
                            let old = *rect;
                            *rect = k.rect;
                            bounds_changes.push((h.frag.clone(), old, k.rect));
                        }
                    }
                    let prev = h
                        .state
                        .enabled
                        .swap(k.enabled, std::sync::atomic::Ordering::SeqCst);
                    if prev != k.enabled {
                        enabled_flips.push((h.frag.clone(), k.enabled));
                    }
                    let prev = h
                        .state
                        .offscreen
                        .swap(k.offscreen, std::sync::atomic::Ordering::SeqCst);
                    if prev != k.offscreen {
                        offscreen_flips.push((h.frag.clone(), k.offscreen));
                    }
                    h.state
                        .actionable
                        .store(k.actionable, std::sync::atomic::Ordering::SeqCst);
                    *h.state.selected.lock().unwrap() = k.selected;
                    *h.state.checked.lock().unwrap() = k.checked;
                    *h.state.expanded.lock().unwrap() = k.expanded;
                    order.push((k.id, h.frag.downgrade()?));
                    continue;
                }
                structure_changed = true;
                let state = Arc::new(ChildState {
                    automation_id: Mutex::new(k.automation_id.clone()),
                    name: Mutex::new(k.name),
                    rect: Mutex::new(k.rect),
                    enabled: std::sync::atomic::AtomicBool::new(k.enabled),
                    actionable: std::sync::atomic::AtomicBool::new(k.actionable),
                    offscreen: std::sync::atomic::AtomicBool::new(k.offscreen),
                    selected: Mutex::new(k.selected),
                    checked: Mutex::new(k.checked),
                    expanded: Mutex::new(k.expanded),
                });
                // editor → wrap the peer's native provider (delegating
                // fragment owns Name/Bounds/Enabled/Focus); painted → our
                // own fragment
                let native = if k.peer_node {
                    match k.native {
                        Some(f) => Some(f),
                        None => self
                            .native
                            .lock()
                            .unwrap()
                            .get(&k.id.slot)
                            .filter(|(g, _)| *g == k.id.generation)
                            .map(|(_, f)| f.clone()),
                    }
                } else {
                    None
                };
                let frag: IRawElementProviderFragment = if let Some(inner) = native {
                    let inner_simple: IRawElementProviderSimple = inner.cast()?;
                    let co = windows_core::ComObject::new(EditorFragment {
                        inner,
                        inner_simple,
                        snap: self.snap.clone(),
                        node: k.id,
                        state: state.clone(),
                        root: Mutex::new(Weak::new()),
                    });
                    let f: IRawElementProviderFragment = co.to_interface();
                    *co.get().root.lock().unwrap() = self.root_frag.downgrade()?;
                    f
                } else {
                    let co = windows_core::ComObject::new(PaintFragment {
                        snap: self.snap.clone(),
                        node: k.id,
                        state: state.clone(),
                        control_type: k.ct,
                        localized: k.localized,
                        invoke: Mutex::new(Weak::new()),
                        root: Mutex::new(Weak::new()),
                    });
                    let f: IRawElementProviderFragment = co.to_interface();
                    let invoke: IInvokeProvider = co.to_interface();
                    *co.get().invoke.lock().unwrap() = invoke.downgrade()?;
                    *co.get().root.lock().unwrap() = self.root_frag.downgrade()?;
                    f
                };
                table.insert(
                    k.id,
                    ChildShared {
                        frag: frag.clone(),
                        state,
                    },
                );
                order.push((k.id, frag.downgrade()?));
            }
            // prune dead + stale-generation associations
            if table
                .keys()
                .any(|id| live.get(&id.slot) != Some(&id.generation))
            {
                structure_changed = true;
            }
            table.retain(|id, _| live.get(&id.slot) == Some(&id.generation));
            self.native
                .lock()
                .unwrap()
                .retain(|slot, (g, _)| live.get(slot) == Some(g));
            *self.snap.live.lock().unwrap() = live;
            if !structure_changed {
                // reorder-only: same live ids in a different sequence
                let new_order: Vec<NodeId> = order.iter().map(|(id, _)| *id).collect();
                if new_order != prev_order {
                    structure_changed = true;
                }
            }
            *self.snap.order.lock().unwrap() = order;
        }
        // notifications ride the public API — safe from any thread
        unsafe {
            for (frag, old, new) in name_changes {
                if let Ok(simple) = frag.cast::<IRawElementProviderSimple>() {
                    let _ = UiaRaiseAutomationPropertyChangedEvent(
                        &simple,
                        UIA_NamePropertyId,
                        &VARIANT::from(BSTR::from(old)),
                        &VARIANT::from(BSTR::from(new)),
                    );
                }
            }
            for (frag, old, new) in bounds_changes {
                if let Ok(simple) = frag.cast::<IRawElementProviderSimple>() {
                    let _ = UiaRaiseAutomationPropertyChangedEvent(
                        &simple,
                        UIA_BoundingRectanglePropertyId,
                        &VARIANT::from(bounding_variant(old)),
                        &VARIANT::from(bounding_variant(new)),
                    );
                }
            }
            for (frag, enabled) in enabled_flips {
                if let Ok(simple) = frag.cast::<IRawElementProviderSimple>() {
                    let _ = UiaRaiseAutomationPropertyChangedEvent(
                        &simple,
                        UIA_IsEnabledPropertyId,
                        &VARIANT::from(!enabled),
                        &VARIANT::from(enabled),
                    );
                }
            }
            for (frag, offscreen) in offscreen_flips {
                if let Ok(simple) = frag.cast::<IRawElementProviderSimple>() {
                    let _ = UiaRaiseAutomationPropertyChangedEvent(
                        &simple,
                        UIA_IsOffscreenPropertyId,
                        &VARIANT::from(!offscreen),
                        &VARIANT::from(offscreen),
                    );
                }
            }
            if structure_changed && let Ok(simple) = self.root.cast::<IRawElementProviderSimple>() {
                let _ = UiaRaiseStructureChangedEvent(
                    &simple,
                    StructureChangeType_ChildrenInvalidated,
                    std::ptr::null_mut(),
                    0,
                );
            }
        }
        Ok(())
    }

    /// Register a native editor provider (built by the peer + site) —
    /// keyed on the full NodeId's generation so a replacement peer can
    /// never inherit the retired provider.
    pub(crate) fn register_native(
        &self,
        id: NodeId,
        frag: IRawElementProviderFragment,
    ) -> UiResult {
        self.native
            .lock()
            .unwrap()
            .insert(id.slot, (id.generation, frag));
        Ok(())
    }

    /// Runtime focus moved — record the owner so GetFocus/HasKeyboardFocus
    /// resolve through the live order, and raise the focus event off the
    /// focused fragment (external clients depend on the notification).
    pub(crate) fn set_focus(&self, id: Option<NodeId>) {
        let changed = *self.snap.focus.lock().unwrap() != id;
        *self.snap.focus.lock().unwrap() = id;
        if !changed {
            return;
        }
        let frag = id.and_then(|id| {
            self.snap
                .children
                .lock()
                .unwrap()
                .get(&id)
                .map(|c| c.frag.clone())
        });
        if let Some(frag) = frag
            && let Ok(simple) = frag.cast::<IRawElementProviderSimple>()
        {
            unsafe {
                let _ = UiaRaiseAutomationEvent(&simple, UIA_AutomationFocusChangedEventId);
            }
        }
    }

    /// Disconnect all UIA — the root unregisters from UIA's tables (a stale
    /// external reference to the ROOT dies too), then the closed flag +
    /// tables clear BEFORE the backend tears down peers and surfaces.
    pub(crate) fn close(&self) {
        self.snap
            .closed
            .store(true, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            let _ = self
                .root
                .cast::<IRawElementProviderSimple>()
                .ok()
                .map(|r| UiaDisconnectProvider(&r));
        }
        self.snap.live.lock().unwrap().clear();
        self.snap.order.lock().unwrap().clear();
        self.snap.children.lock().unwrap().clear();
        self.native.lock().unwrap().clear();
    }

    pub(crate) fn provider(&self) -> IRawElementProviderFragmentRoot {
        self.root.clone()
    }
}

/// Every exit path breaks the snapshot<->fragment cycle and disconnects
/// the root from UIA's tables — Drop is the cleanup contract's backstop,
/// not just the normal close() route.
impl Drop for UiaRoot {
    fn drop(&mut self) {
        self.close();
    }
}
