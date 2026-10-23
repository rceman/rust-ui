#[test]
#[cfg(target_os = "windows")]
fn selection_band_renders_contract_dark() {
    use crate::platform::win32::space::{LogicalRect, ScaleFactor};
    let lib = crate::platform::win32::Msftedit::load().expect("msftedit");
    let hwnd = probe_hwnd();
    let cfg = crate::platform::win32::PeerConfig {
        multiline: true,
        read_only: false,
        face: "Segoe UI".into(),
        size_twips: 280,
        line_twips: 300,
        fg: [0., 0., 0., 1.],
        fg_authored: crate::style::Color::rgba(10, 10, 10, 255),
        sel_bg: [0., 0., 0., 1.],
        sel_fg: [1., 1., 1., 1.],
        bold: false,
    };
    let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut peer = crate::platform::win32::WindowlessPeer::create(
        1,
        &lib,
        hwnd,
        ScaleFactor(1.0),
        &cfg,
        sink,
        std::sync::Arc::new(std::sync::Mutex::new(
            crate::platform::win32::text::TimerPool::new(),
        )),
    )
    .expect("peer create");
    crate::node::TextPeer::initialize(
        &mut peer,
        "First line of a note.\nSecond line with more text.\nThird line closes it out.",
        crate::text::TextRevision::mint(),
        crate::text::BindingToken::mint(),
    )
    .expect("init");
    crate::node::TextPeer::apply_selection(&mut peer, Some((22, 33))).expect("sel");
    peer.apply_bounds(
        LogicalRect {
            x: 0.,
            y: 0.,
            width: 340.,
            height: 62.,
        },
        ScaleFactor(1.0),
    )
    .expect("bounds");
    // WHITE canvas — the near-black band must be a real discriminant,
    // not the background: count dark pixels in the selected run's rows
    // vs the un-selected control row directly above
    let fb = draw_peer_dib_on(&peer, [1., 1., 1., 1.]);
    let dark = |y0: usize, y1: usize| -> usize {
        let mut n = 0;
        for y in y0..y1 {
            for x in 0..200usize {
                let o = (y * 800 + x) * 4;
                if fb[o + 2] < 60 && fb[o + 1] < 60 && fb[o] < 60 {
                    n += 1;
                }
            }
        }
        n
    };
    let band_px = dark(25, 45); // "Second line" band rows
    let ctrl_px = dark(4, 20); // un-selected first line: glyphs only
    assert!(
        band_px > ctrl_px + 300,
        "selection band must add dark ink over the glyph baseline,          got band={band_px}px vs control={ctrl_px}px — if the band is          skipped both counts collapse to glyph-only pixels"
    );
}

// ---------------------------------------------------------------------------
// popup placement — the shared `resolve_popup_placement` policy
// ---------------------------------------------------------------------------

fn anchor(x: f32, y: f32, w: f32, h: f32) -> crate::geom::Rect {
    crate::geom::Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

#[test]
fn popup_placement_bottom_center_offset() {
    // bottom + center + side gap — popup lands centered under the anchor
    let r = crate::layout::resolve_popup_placement(
        anchor(100.0, 50.0, 80.0, 32.0),
        crate::geom::Side::Bottom,
        crate::geom::Align::Center,
        crate::geom::Point { x: 0.0, y: 4.0 },
        crate::geom::Size {
            width: 120.0,
            height: 60.0,
        },
        anchor(0.0, 0.0, 800.0, 600.0),
    );
    assert_eq!((r.x, r.y, r.width, r.height), (80.0, 86.0, 120.0, 60.0));
}

#[test]
fn popup_placement_start_end_align() {
    let vp = anchor(0.0, 0.0, 800.0, 600.0);
    let nat = crate::geom::Size {
        width: 100.0,
        height: 40.0,
    };
    let s = crate::layout::resolve_popup_placement(
        anchor(100.0, 50.0, 80.0, 32.0),
        crate::geom::Side::Bottom,
        crate::geom::Align::Start,
        crate::geom::Point::default(),
        nat,
        vp,
    );
    assert_eq!(s.x, 100.0);
    let e = crate::layout::resolve_popup_placement(
        anchor(100.0, 50.0, 80.0, 32.0),
        crate::geom::Side::Bottom,
        crate::geom::Align::End,
        crate::geom::Point::default(),
        nat,
        vp,
    );
    assert_eq!(e.x, 80.0);
}

#[test]
fn popup_placement_flips_when_no_room() {
    // anchor near the bottom edge: Bottom overflows -> flips to Top
    let r = crate::layout::resolve_popup_placement(
        anchor(100.0, 560.0, 80.0, 32.0),
        crate::geom::Side::Bottom,
        crate::geom::Align::Start,
        crate::geom::Point { x: 0.0, y: 4.0 },
        crate::geom::Size {
            width: 100.0,
            height: 60.0,
        },
        anchor(0.0, 0.0, 800.0, 600.0),
    );
    assert_eq!(r.y, 496.0); // 560 - 60 - 4
}

#[test]
fn popup_placement_clamps_to_viewport() {
    // unclamped Start would push past the right edge -> pinned inside
    let r = crate::layout::resolve_popup_placement(
        anchor(760.0, 50.0, 30.0, 30.0),
        crate::geom::Side::Bottom,
        crate::geom::Align::Start,
        crate::geom::Point::default(),
        crate::geom::Size {
            width: 100.0,
            height: 40.0,
        },
        anchor(0.0, 0.0, 800.0, 600.0),
    );
    assert_eq!(r.x, 700.0);
    // horizontal side placement: Right overflows -> flips Left
    let r = crate::layout::resolve_popup_placement(
        anchor(700.0, 100.0, 80.0, 40.0),
        crate::geom::Side::Right,
        crate::geom::Align::Start,
        crate::geom::Point { x: 4.0, y: 0.0 },
        crate::geom::Size {
            width: 120.0,
            height: 60.0,
        },
        anchor(0.0, 0.0, 800.0, 600.0),
    );
    assert_eq!(r.x, 576.0); // 700 - 120 - 4
}

// ---------------------------------------------------------------------------
// Slice 3 — directional group navigation (`nav_target` shared policy)
// ---------------------------------------------------------------------------

fn mk_nav_node(
    role: Option<crate::node::SemanticRole>,
    parent: Option<u32>,
    data: crate::node::NodeData,
) -> crate::node::Node {
    use std::rc::Rc;
    crate::node::Node {
        automation_id: None,
        part_key: None,
        role,
        key: crate::key::ChildKey::Static {
            kind: 0,
            ordinal: 0,
        },
        parent,
        children: Vec::new(),
        data,
        presented: crate::node::Presented::default(),
        visibility: crate::Visibility::Visible,
        layout: crate::geom::LayoutSpec::default(),
        factories: crate::event::EventFactorySet::default(),
        adapters: Rc::new(Vec::new()),
        peer: None,
        dirty: 0,
    }
}

fn mk_nav_tab(disabled: bool) -> crate::node::NodeData {
    use std::rc::Rc;
    crate::node::NodeData::Button {
        text: Rc::from("tab"),
        variant: crate::theme::ButtonVariant::Tab,
        style: Default::default(),
        disabled,
        size: crate::theme::ControlSize::Md,
        motion: None,
        tooltip: None,
        icon_start: None,
        icon_end: None,
        icon_only: false,
        accessible_name: None,
        invalid: false,
        selected: false,
        expanded: None,
        icon_color: None,
    }
}

#[test]
fn nav_target_cycles_tabs_wraps_and_jumps() {
    use crate::event::Key;
    use crate::node::{NodeData, SemanticRole};
    let mut a = crate::arena::Arena::new();
    let list = a.alloc(mk_nav_node(
        Some(SemanticRole::TabList),
        None,
        NodeData::Container {
            kind: crate::node::KIND_BOX,
            props: Default::default(),
        },
    ));
    let t0 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(list.slot),
        mk_nav_tab(false),
    ));
    let t1 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(list.slot),
        mk_nav_tab(false),
    ));
    let t2 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(list.slot),
        mk_nav_tab(false),
    ));
    let order = vec![list, t0, t1, t2];
    // arrows wrap through paint order; Home/End jump
    assert_eq!(
        crate::layout::nav_target(&a, &order, t0, Key::ArrowRight),
        Some(t1)
    );
    assert_eq!(
        crate::layout::nav_target(&a, &order, t1, Key::ArrowRight),
        Some(t2)
    );
    assert_eq!(
        crate::layout::nav_target(&a, &order, t2, Key::ArrowRight),
        Some(t0),
        "right on the last tab wraps to the first"
    );
    assert_eq!(
        crate::layout::nav_target(&a, &order, t0, Key::ArrowLeft),
        Some(t2),
        "left on the first tab wraps to the last"
    );
    assert_eq!(
        crate::layout::nav_target(&a, &order, t1, Key::Home),
        Some(t0)
    );
    assert_eq!(
        crate::layout::nav_target(&a, &order, t0, Key::End),
        Some(t2)
    );
    // a non-nav key declines — the generic key dispatch handles it
    assert_eq!(crate::layout::nav_target(&a, &order, t0, Key::Tab), None);
    assert_eq!(
        crate::layout::nav_target(&a, &order, t0, Key::ArrowDown),
        None,
        "vertical arrows do not navigate a horizontal tablist"
    );
}

#[test]
fn nav_target_skips_disabled_tabs() {
    use crate::event::Key;
    use crate::node::SemanticRole;
    let mut a = crate::arena::Arena::new();
    let list = a.alloc(mk_nav_node(
        Some(SemanticRole::TabList),
        None,
        crate::node::NodeData::Container {
            kind: crate::node::KIND_BOX,
            props: Default::default(),
        },
    ));
    let t0 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(list.slot),
        mk_nav_tab(false),
    ));
    let t1 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(list.slot),
        mk_nav_tab(true), // disabled — skipped
    ));
    let t2 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(list.slot),
        mk_nav_tab(false),
    ));
    let order = vec![list, t0, t1, t2];
    assert_eq!(
        crate::layout::nav_target(&a, &order, t0, Key::ArrowRight),
        Some(t2),
        "the disabled middle tab is skipped"
    );
    assert_eq!(
        crate::layout::nav_target(&a, &order, t2, Key::ArrowLeft),
        Some(t0)
    );
    // End lands on the last ENABLED tab, not the disabled one
    let t3 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(list.slot),
        mk_nav_tab(true),
    ));
    let order = vec![list, t0, t1, t2, t3];
    assert_eq!(
        crate::layout::nav_target(&a, &order, t0, Key::End),
        Some(t2)
    );
}

#[test]
fn nav_target_options_move_inside_the_popup() {
    use crate::event::Key;
    use crate::node::{NodeData, SemanticRole};
    let mut a = crate::arena::Arena::new();
    let pop = a.alloc(mk_nav_node(
        None,
        None,
        NodeData::Popup {
            anchor: None,
            side: crate::geom::Side::Bottom,
            align: crate::geom::Align::Center,
            offset: Default::default(),
            role: crate::node::PopupRole::Listbox,
            modal: false,
            dismiss_light: true,
            focus_on_open: true,
            props: Default::default(),
        },
    ));
    let col = a.alloc(mk_nav_node(
        None,
        Some(pop.slot),
        NodeData::Container {
            kind: crate::node::KIND_COLUMN,
            props: Default::default(),
        },
    ));
    let mut opt = |disabled| {
        mk_nav_node(
            Some(SemanticRole::Option),
            Some(col.slot),
            mk_nav_tab(disabled),
        )
    };
    let o0 = a.alloc(opt(false));
    let o1 = a.alloc(opt(true)); // disabled — skipped
    let o2 = a.alloc(opt(false));
    let order = vec![pop, col, o0, o1, o2];
    assert_eq!(
        crate::layout::nav_target(&a, &order, o0, Key::ArrowDown),
        Some(o2),
        "options move vertically, skipping disabled"
    );
    assert_eq!(
        crate::layout::nav_target(&a, &order, o2, Key::ArrowUp),
        Some(o0)
    );
    assert_eq!(
        crate::layout::nav_target(&a, &order, o0, Key::End),
        Some(o2)
    );
    assert_eq!(
        crate::layout::nav_target(&a, &order, o0, Key::ArrowRight),
        None,
        "horizontal arrows do not navigate a vertical listbox"
    );
}

#[test]
fn nav_target_cycles_mixed_menu_items_and_skips_disabled() {
    use crate::event::Key;
    use crate::node::{NodeData, SemanticRole};
    use std::rc::Rc;
    let mut arena = crate::arena::Arena::new();
    let popup = arena.alloc(mk_nav_node(
        None,
        None,
        NodeData::Popup {
            anchor: None,
            side: crate::geom::Side::Bottom,
            align: crate::geom::Align::Start,
            offset: Default::default(),
            role: crate::node::PopupRole::Menu,
            modal: false,
            dismiss_light: true,
            focus_on_open: true,
            props: Default::default(),
        },
    ));
    let make = |role, disabled| {
        mk_nav_node(
            Some(role),
            Some(popup.slot),
            NodeData::Action {
                label: Rc::from("item"),
                style: crate::style::ActionStyle::new(Default::default()),
                disabled,
                checked: None,
            },
        )
    };
    let a = arena.alloc(make(SemanticRole::MenuItem, false));
    let b = arena.alloc(make(SemanticRole::MenuItemCheckbox, true));
    let c = arena.alloc(make(SemanticRole::MenuItemRadio, false));
    let d = arena.alloc(make(SemanticRole::MenuItemDestructive, false));
    let order = vec![popup, a, b, c, d];
    assert_eq!(
        crate::layout::nav_target(&arena, &order, a, Key::ArrowDown),
        Some(c),
        "menu arrows share one sequence and skip disabled items"
    );
    assert_eq!(
        crate::layout::nav_target(&arena, &order, c, Key::ArrowDown),
        Some(d)
    );
    assert_eq!(
        crate::layout::nav_target(&arena, &order, d, Key::Home),
        Some(a)
    );
    assert_eq!(
        crate::layout::nav_target(&arena, &order, a, Key::ArrowRight),
        None
    );
}

#[test]
fn nav_target_ignores_foreign_groups_and_hidden_nodes() {
    use crate::event::Key;
    use crate::node::SemanticRole;
    let mut a = crate::arena::Arena::new();
    // two independent tablists — nav must not leak across groups
    let mk_list = || {
        mk_nav_node(
            Some(SemanticRole::TabList),
            None,
            crate::node::NodeData::Container {
                kind: crate::node::KIND_BOX,
                props: Default::default(),
            },
        )
    };
    let l1 = a.alloc(mk_list());
    let l2 = a.alloc(mk_list());
    let a0 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(l1.slot),
        mk_nav_tab(false),
    ));
    let a1 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(l1.slot),
        mk_nav_tab(false),
    ));
    let _b0 = a.alloc(mk_nav_node(
        Some(SemanticRole::Tab),
        Some(l2.slot),
        mk_nav_tab(false),
    ));
    // hidden tab in l1 — skipped like disabled
    let mut h = mk_nav_node(Some(SemanticRole::Tab), Some(l1.slot), mk_nav_tab(false));
    h.visibility = crate::Visibility::Hidden;
    let h_id = a.alloc(h);
    let order = vec![l1, a0, a1, h_id, l2, _b0];
    assert_eq!(
        crate::layout::nav_target(&a, &order, a1, Key::ArrowRight),
        Some(a0),
        "nav wraps within l1 only — never reaches l2 or hidden nodes"
    );
}
