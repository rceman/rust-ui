// Shared in-page measurement - single source for capture.js (contract
// generation) and check.js (independent live capture-authority audit).

// In-page measurement. Contract authority is the post-force painted state.
const IN_PAGE_MEASURE = `(() => {
  const round = (n) => Math.round(n * 1000) / 1000;
  const errors = [];

  // keyed part identity: chain of data-part ancestors inside the element,
  // with data-key qualifiers - e.g. item[production].indicator, row[0].cell[1].
  const partKey = (p, stop) => {
    const chain = [];
    let cur = p;
    while (cur && cur !== stop && cur !== document.body) {
      if (cur.hasAttribute && cur.hasAttribute("data-part")) {
        const k = cur.getAttribute("data-key");
        chain.unshift(
          k ? cur.getAttribute("data-part") + "[" + k + "]"
            : cur.getAttribute("data-part"));
      }
      cur = cur.parentElement;
    }
    return chain.join(".");
  };

  // C02 text/part ownership: data-part-owner="<automation_id>" attributes a
  // subtree outside the owner's DOM subtree (e.g. the checkbox row label) to
  // that owner element's parts + text runs. Nearest marker wins: walking up
  // from a text node, the first of [data-part-owner] / [data-automation-id]
  // / [data-text-exempt] decides.
  const ownerOf = (node) => {
    let cur = node.nodeType === 3 ? node.parentElement : node;
    while (cur && cur !== document.documentElement) {
      if (cur.hasAttribute && cur.hasAttribute("data-part-owner"))
        return { kind: "owner", id: cur.getAttribute("data-part-owner"), partEl: cur };
      if (cur.hasAttribute && cur.hasAttribute("data-automation-id"))
        return { kind: "owner", id: cur.getAttribute("data-automation-id") };
      if (cur.hasAttribute && cur.hasAttribute("data-text-exempt"))
        return { kind: "exempt", cls: cur.getAttribute("data-text-exempt") };
      cur = cur.parentElement;
    }
    return { kind: "none" };
  };
  // closed exemption vocabulary - gallery chrome only, never specimen text
  const TEXT_EXEMPT = new Set(["gallery-caption", "gallery-heading", "gallery-shell"]);

  // index data-part-owner elements by owner id
  const ownedPartEls = {};
  for (const p of document.querySelectorAll("[data-part-owner]")) {
    const oid = p.getAttribute("data-part-owner");
    (ownedPartEls[oid] = ownedPartEls[oid] || []).push(p);
  }

  // canvas font metrics for an effective font (ascent/descent)
  const ctx2d = document.createElement("canvas").getContext("2d");
  const fontMetrics = (cs) => {
    ctx2d.font = cs.fontStyle + " " + cs.fontVariant + " " + cs.fontWeight + " " + cs.fontSize + " / " + cs.lineHeight + " " + cs.fontFamily;
    try { ctx2d.font = cs.fontStyle + " " + cs.fontWeight + " " + cs.fontSize + " " + cs.fontFamily; } catch (_) {}
    const m = ctx2d.measureText("Mg");
    return { ascent: m.fontBoundingBoxAscent, descent: m.fontBoundingBoxDescent };
  };
  // baseline_y (element-local, px from element top): the Range rect of a
  // text node already starts at the font box top (ascent above baseline),
  // so baseline = rangeTop + fontBoundingBoxAscent of the effective font.
  const baselineOf = (lineTop, lineHeight, fm) => round(lineTop + fm.ascent);

  // Mirror an input/textarea into a styled div positioned identically, then
  // measure the mirror's text lines (Range.getClientRects). Value text uses
  // the control color; placeholder text uses the ::placeholder color.
  const measureControl = (el, elRect) => {
    const cs = getComputedStyle(el);
    const isTa = el.tagName === "TEXTAREA";
    const runs = [];
    const entries = [];
    if (el.value && el.value.trim()) entries.push({ kind: "value", text: el.value, color: cs.color });
    if (!el.value && el.placeholder && el.placeholder.trim())
      entries.push({ kind: "placeholder", text: el.placeholder, color: getComputedStyle(el, "::placeholder").color });
    if (!entries.length) return runs;
    const fm = fontMetrics(cs);
    const lh = parseFloat(cs.lineHeight) || fm.ascent + fm.descent;
    for (const ent of entries) {
      const m = document.createElement("div");
      m.setAttribute("style",
        "position:absolute;left:" + elRect.x + "px;top:" + elRect.y + "px;" +
        "width:" + elRect.width + "px;" +
        (isTa ? "" : "height:" + elRect.height + "px;") +
        "box-sizing:border-box;" +
        "padding:" + cs.padding + ";border-width:0;" +
        "font:" + cs.fontStyle + " " + cs.fontWeight + " " + cs.fontSize + "/" + cs.lineHeight + " " + cs.fontFamily + ";" +
        "letter-spacing:" + cs.letterSpacing + ";text-indent:" + cs.textIndent + ";" +
        "white-space:" + (isTa ? "pre-wrap" : "pre") + ";" +
        "overflow-wrap:break-word;word-break:break-word;" +
        "pointer-events:none;visibility:hidden;");
      m.textContent = ent.text;
      document.body.appendChild(m);
      const tn = m.firstChild;
      const range = document.createRange();
      range.selectNodeContents(tn);
      const rects = [...range.getClientRects()].filter((r) => r.width > 0 || r.height > 0);
      const lines = rects.map((r) => ({
        rect: { x: round(r.x - elRect.x), y: round(r.y - elRect.y), width: round(r.width), height: round(r.height) },
        baseline_y: baselineOf(r.y - elRect.y, lh, fm),
      }));
      const u = { x: 1e9, y: 1e9, x2: -1e9, y2: -1e9 };
      for (const r of rects) {
        u.x = Math.min(u.x, r.x - elRect.x); u.y = Math.min(u.y, r.y - elRect.y);
        u.x2 = Math.max(u.x2, r.x - elRect.x + r.width); u.y2 = Math.max(u.y2, r.y - elRect.y + r.height);
      }
      runs.push({
        key: ent.kind, font_ascent: fm.ascent, font_descent: fm.descent,
        content: ent.text.slice(0, 200),
        source: ent.kind,
        rect: rects.length ? { x: round(u.x), y: round(u.y), width: round(u.x2 - u.x), height: round(u.y2 - u.y) } : null,
        lines,
        font_family: cs.fontFamily,
        font_size: round(parseFloat(cs.fontSize)),
        font_weight: parseInt(cs.fontWeight, 10) || 400,
        line_height: cs.lineHeight === "normal" ? round(fm.ascent + fm.descent) : round(parseFloat(cs.lineHeight)),
        color: ent.color,
        measure: "mirror-div effective font/text/padding geometry with control border width suppressed (input values + placeholders have no DOM text nodes; browser editing behavior is not authority)",
      });
      m.remove();
    }
    return runs;
  };

  // leaf DOM text runs: each non-whitespace text node = one run; the run's
  // effective font is its parent element's computed style. extraContainers
  // are data-part-owner elements outside el's subtree whose text still
  // belongs to el (owner attribution).
  const measureDomText = (el, elRect, extraContainers = []) => {
    const elId = el.getAttribute("data-automation-id");
    const runs = [];
    let idx = 0;
    for (const container of [el, ...extraContainers]) {
      const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);
      let n;
      while ((n = walker.nextNode())) {
        if (!n.nodeValue || !n.nodeValue.trim()) continue;
        const own = ownerOf(n);
        if (own.kind !== "owner" || own.id !== elId) continue; // nested/foreign element owns its text
        const parent = n.parentElement;
        const cs = getComputedStyle(parent);
        const fm = fontMetrics(cs);
        const lh = parseFloat(cs.lineHeight) || fm.ascent + fm.descent;
        // sr-only/visually-hidden text: laid out (Range gives rects) but not
        // painted - clipped to 1px. Recorded for the a11y contract, excluded
        // from pixel-ink validation.
        const clipped =
          cs.clipPath === "inset(50%)" || cs.clipPath === "inset(50% 50% 50% 50%)" ||
          (cs.clip && cs.clip !== "auto") ||
          (parent.offsetWidth <= 1 && parent.offsetHeight <= 1 && cs.overflow !== "visible");
        const range = document.createRange();
        range.selectNodeContents(n);
        const rects = [...range.getClientRects()].filter((r) => r.width > 0 || r.height > 0);
        if (!rects.length) continue;
        const u = { x: 1e9, y: 1e9, x2: -1e9, y2: -1e9 };
        for (const r of rects) {
          u.x = Math.min(u.x, r.x - elRect.x); u.y = Math.min(u.y, r.y - elRect.y);
          u.x2 = Math.max(u.x2, r.x - elRect.x + r.width); u.y2 = Math.max(u.y2, r.y - elRect.y + r.height);
        }
        const runPart = own.partEl
          ? (own.partEl.getAttribute("data-key")
              ? own.partEl.getAttribute("data-part") + "[" + own.partEl.getAttribute("data-key") + "]"
              : own.partEl.getAttribute("data-part"))
          : (() => { let cur = parent; while (cur && cur !== el) { if (cur.hasAttribute && cur.hasAttribute("data-part")) return partKey(cur, el); cur = cur.parentElement; } return null; })();
        runs.push({
          key: "text[" + idx + "]", part: runPart, font_ascent: fm.ascent, font_descent: fm.descent,
          content: n.nodeValue.trim().slice(0, 200),
          source: own.partEl ? "owner-part" : "dom",
          rect: { x: round(u.x), y: round(u.y), width: round(u.x2 - u.x), height: round(u.y2 - u.y) },
          lines: rects.map((r) => ({
            rect: { x: round(r.x - elRect.x), y: round(r.y - elRect.y), width: round(r.width), height: round(r.height) },
            baseline_y: baselineOf(r.y - elRect.y, lh, fm),
          })),
          font_family: cs.fontFamily,
          font_size: round(parseFloat(cs.fontSize)),
          font_weight: parseInt(cs.fontWeight, 10) || 400,
          line_height: cs.lineHeight === "normal" ? round(fm.ascent + fm.descent) : round(parseFloat(cs.lineHeight)),
          color: cs.color,
          visible: clipped ? false : true,
        });
        idx++;
      }
    }
    return runs;
  };

  const els = [...document.querySelectorAll("[data-automation-id]")];
  const out = [];
  for (const el of els) {
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    const rec = {
      automation_id: el.getAttribute("data-automation-id"),
      tag: el.tagName.toLowerCase(),
      slot: el.getAttribute("data-slot"),
      force_state: el.getAttribute("data-force-state") || null,
      rect: { x: round(r.x), y: round(r.y), width: round(r.width), height: round(r.height) },
      box: {
        border_radius: [
          round(parseFloat(cs.borderTopLeftRadius)),
          round(parseFloat(cs.borderTopRightRadius)),
          round(parseFloat(cs.borderBottomRightRadius)),
          round(parseFloat(cs.borderBottomLeftRadius)),
        ],
        border_width: [
          round(parseFloat(cs.borderTopWidth)),
          round(parseFloat(cs.borderRightWidth)),
          round(parseFloat(cs.borderBottomWidth)),
          round(parseFloat(cs.borderLeftWidth)),
        ],
        padding: [
          round(parseFloat(cs.paddingTop)),
          round(parseFloat(cs.paddingRight)),
          round(parseFloat(cs.paddingBottom)),
          round(parseFloat(cs.paddingLeft)),
        ],
        opacity: round(parseFloat(cs.opacity)),
        color: cs.color,
        background_color: cs.backgroundColor,
        border_color: cs.borderColor,
        outline: cs.outlineStyle !== "none" ? { width: round(parseFloat(cs.outlineWidth)), color: cs.outlineColor } : null,
        box_shadow: cs.boxShadow === "none" ? null : cs.boxShadow,
        line_height: cs.lineHeight,
        overflow_x: cs.overflowX,
        overflow_y: cs.overflowY,
      },
      parts: {},
      text: null,
      text_runs: [],
    };
    // parts: descendants with data-part, keyed; nested automation elements own
    // their own parts (skip to avoid double-count). Parts with no client
    // rects / display:none / zero size are recorded as hidden (never a
    // negative/zero off-document rect as geometry).
    const recPart = (key, p, extra) => {
      const pr = p.getBoundingClientRect();
      const pcs = getComputedStyle(p);
      if (rec.parts[key]) {
        errors.push(rec.automation_id + ": duplicate part key '" + key + "'");
        return;
      }
      if (!p.getClientRects().length || pcs.display === "none" || pr.width <= 0 || pr.height <= 0) {
        rec.parts[key] = { visible: false, reason: "not-rendered" };
        return;
      }
      rec.parts[key] = {
        x: round(pr.x - r.x), y: round(pr.y - r.y),
        width: round(pr.width), height: round(pr.height),
        background_color: pcs.backgroundColor,
        color: pcs.color,
        ...extra,
      };
    };
    for (const p of el.querySelectorAll("[data-part]")) {
      if (p.closest("[data-automation-id]") !== el) continue;
      recPart(partKey(p, el), p);
    }
    // owner-attributed parts (data-part-owner, outside the subtree): measured
    // into this owner's parts; label_gap = label.left - control.right
    const elId = el.getAttribute("data-automation-id");
    const owned = ownedPartEls[elId] || [];
    for (const p of owned) {
      const pk = p.getAttribute("data-part") + (p.getAttribute("data-key") ? "[" + p.getAttribute("data-key") + "]" : "");
      const pr = p.getBoundingClientRect();
      recPart(pk, p, { owner_attributed: true, label_gap: round(pr.x - (r.x + r.width)) });
    }
    // interaction state (C01): the contract records whether the element is
    // focused and, only when focused with a non-collapsed range, its
    // selection - a document has exactly one focus/selection.
    if (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable) {
      const focused = document.activeElement === el;
      let sel = null;
      if (focused) {
        const s = el.selectionStart, e2 = el.selectionEnd;
        if (typeof s === "number" && typeof e2 === "number" && s !== e2) sel = [s, e2];
      }
      rec.interaction = { focused, selection: sel };
    }
    if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") {
      rec.text_runs = measureControl(el, r);
    } else {
      rec.text_runs = measureDomText(el, r, owned);
    }
    // compatibility summary: first run's font on the element
    const tr = rec.text_runs[0];
    if (tr) {
      rec.text = {
        content: rec.text_runs.map((x) => x.content).join("\\n").slice(0, 200),
        rect: tr.rect,
        font_family: tr.font_family,
        font_size: tr.font_size,
        font_weight: tr.font_weight,
        line_height: tr.line_height,
        baseline_proxy_y: tr.lines[0] ? tr.lines[0].baseline_y : null,
      };
    }
    out.push(rec);
  }

  // C02 visible-text completeness: every visible non-whitespace text node
  // must resolve to an owner ([data-automation-id] or [data-part-owner])
  // or to a data-text-exempt ancestor from the closed vocabulary.
  const textInventory = [];
  const w2 = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  let tn;
  while ((tn = w2.nextNode())) {
    if (!tn.nodeValue || !tn.nodeValue.trim()) continue;
    const parent = tn.parentElement;
    if (!parent || !parent.getClientRects().length) continue;
    const pcs = getComputedStyle(parent);
    if (pcs.visibility === "hidden" || pcs.display === "none") continue;
    if (pcs.clipPath === "inset(50%)" || pcs.clipPath === "inset(50% 50% 50% 50%)" ||
        (parent.offsetWidth <= 1 && parent.offsetHeight <= 1 && pcs.overflow !== "visible")) continue;
    const own = ownerOf(tn);
    if (own.kind === "none") {
      errors.push("unowned visible text node: '" + tn.nodeValue.trim().slice(0, 40) + "'");
      continue;
    }
    if (own.kind === "exempt") {
      if (!TEXT_EXEMPT.has(own.cls))
        errors.push("unknown data-text-exempt class '" + own.cls + "'");
      continue;
    }
    let pk = null;
    if (own.partEl) {
      pk = own.partEl.getAttribute("data-key")
        ? own.partEl.getAttribute("data-part") + "[" + own.partEl.getAttribute("data-key") + "]"
        : own.partEl.getAttribute("data-part");
    } else {
      const ownerEl = document.querySelector("[data-automation-id='" + own.id + "']");
      let cur = parent;
      while (cur && cur !== ownerEl) {
        if (cur.hasAttribute && cur.hasAttribute("data-part")) { pk = partKey(cur, ownerEl); break; }
        cur = cur.parentElement;
      }
    }
    textInventory.push({ owner: own.id, part: pk, content: tn.nodeValue.trim().slice(0, 200) });
  }
  return { elements: out, errors, textInventory, doc: { width: round(document.documentElement.scrollWidth), height: round(document.documentElement.scrollHeight) } };
})()`;

// V04 capture-state assertions: the applied state table's focus/selection
// must still hold and the document must not be scrolled; each goto is a
// fresh document so nothing can carry across, and we assert it anyway.
const STATE_ASSERT = `(() => {
  const a = window.__captureStateApplied;
  const errs = [];
  if (window.scrollX !== 0 || window.scrollY !== 0)
    errs.push("scroll=" + window.scrollX + "," + window.scrollY);
  const ae = document.activeElement;
  const aId = ae && ae.getAttribute ? ae.getAttribute("data-automation-id") : null;
  const wantId = a ? a.focused : null;
  if (wantId) {
    if (aId !== wantId) errs.push("activeElement=" + (aId || (ae && ae.tagName)) + " != " + wantId);
  } else if (ae && ae !== document.body && ae !== document.documentElement) {
    errs.push("unexpected activeElement=" + (aId || ae.tagName));
  }
  if (a && a.range && ae && (ae.selectionStart !== a.range[0] || ae.selectionEnd !== a.range[1]))
    errs.push("selection " + ae.selectionStart + "-" + ae.selectionEnd + " != " + a.range[0] + "-" + a.range[1]);
  return errs;
})()`;

module.exports = { IN_PAGE_MEASURE, STATE_ASSERT };
