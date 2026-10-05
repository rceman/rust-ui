// Deterministic capture: renders each page in light+dark under fixed
// viewport, applies forced pseudo states via CDP, settles, measures
// geometry/text/parts into contract.json, screenshots, validates text ink
// against the screenshot, and proves Geist font resolution exhaustively via
// CSS.getPlatformFontsForNode. Emits reference.json.
//
// CAPTURE PIPELINE ORDER (per page/state/theme, exactly as implemented):
//   goto ?page&theme&capture=1[&state] -> [data-render-done] -> fonts.ready
//   -> font proof (light+default only: data-fontproof attrs + mirror nodes,
//      fully reverted - post-proof DOM mutation drops CDP-forced states)
//   -> capture-state assertions (scroll/focus/selection)
//   -> CDP forcePseudoState on every [data-force-state]
//   -> settle (>=2 rAF + document.fonts.ready)
//   -> measure IN_PAGE_MEASURE (rect/box/text_runs/parts + text inventory)
//   -> token-binding eval + pressed parity IN THIS session
//   -> capture-state assertions again
//   -> screenshot -> pixel guard (forced-vs-twin crops) + ink validation
//   -> store under captures[<page>/<state>] -> session detach -> next goto.
// The screenshot, contract, live binding values and pixel guard all describe
// the same painted state; no "forced states can't change geometry"
// assumption (active:translate-y-px is real geometry).
const { launchCanonical, VIEWPORT } = require("./browser");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { decodePng } = require("./png");
const { forcedPairError } = require("./pixels");
const V = require("./validate");

const ROOT = path.join(__dirname, "..");
const URL_BASE = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");
const round = (n) => Math.round(n * 1000) / 1000;
const tokens = JSON.parse(fs.readFileSync(path.join(ROOT, "tokens.json"), "utf8"));
const bindingsDoc = JSON.parse(fs.readFileSync(path.join(ROOT, "token-bindings.json"), "utf8"));

const CAPTURE_STATES = {
  "native-text": ["default", "multiline-selection"],
  navigation: ["default", "settings-nav-collapsed"],
};


// Ink validation against the just-taken screenshot: inside each text run's
// rect (+/-1 px), find ink = pixels differing from the dominant local color
// (the run's painted background) by luminance > 40. Assert ink is inside the
// rect (+/-1px) and each line's baseline_y sits inside the line's ink band
// (descenders may reach descent px below baseline).
function validateInk(png, pageY0, elements) {
  const errs = [];
  const px = (x, y) => {
    x = Math.round(x); y = Math.round(y - pageY0);
    if (x < 0 || y < 0 || x >= png.width || y >= png.height) return null;
    const i = (y * png.width + x) * 4;
    return [png.data[i], png.data[i + 1], png.data[i + 2]];
  };
  for (const el of elements) {
    for (const run of el.rec.text_runs || []) {
      if (!run.rect || run.rect.width <= 0 || run.rect.height <= 0) {
        if (run.lines && run.lines.length === 0) continue;
        errs.push(`${el.rec.automation_id}: ${run.key} empty/invalid run rect`);
        continue;
      }
      const rx = el.rec.rect.x + run.rect.x;
      const ry = el.rec.rect.y + run.rect.y;
      const rw = run.rect.width, rh = run.rect.height;
      // dominant color = sampled mode inside the rect
      const freq = new Map();
      for (let yy = ry; yy < ry + rh; yy += 2)
        for (let xx = rx; xx < rx + rw; xx += 2) {
          const c = px(xx, yy);
          if (!c) continue;
          const key = c.join(",");
          freq.set(key, (freq.get(key) || 0) + 1);
        }
      // sr-only text is clipped/unpainted - skip pixel validation
      if (run.visible === false) continue;
      let bg = [255, 255, 255], best = -1;
      for (const [k, n] of freq) if (n > best) { best = n; bg = k.split(",").map(Number); }
      const bgl = 0.2126 * bg[0] + 0.7152 * bg[1] + 0.0722 * bg[2];
      // adaptive threshold: dimmed content (trigger behind bg-black/10
      // backdrop) keeps glyph structure but lower absolute contrast
      let lmin = 1e9, lmax = -1e9;
      for (let yy = ry; yy < ry + rh; yy += 1)
        for (let xx = rx; xx < rx + rw; xx += 1) {
          const c = px(xx, yy);
          if (!c) continue;
          const l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
          lmin = Math.min(lmin, l); lmax = Math.max(lmax, l);
        }
      const threshold = Math.max(12, (lmax - lmin) * 0.3);
      const isInk = (l) => Math.abs(l - bgl) > threshold;
      let top = 1e9, bot = -1e9, left = 1e9, right = -1e9, inkCount = 0;
      for (let yy = ry - 1; yy < ry + rh + 1; yy++)
        for (let xx = rx - 1; xx < rx + rw + 1; xx++) {
          const c = px(xx, yy);
          if (!c) continue;
          const l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
          if (isInk(l)) {
            inkCount++;
            top = Math.min(top, yy); bot = Math.max(bot, yy);
            left = Math.min(left, xx); right = Math.max(right, xx);
          }
        }
      if (!inkCount) {
        // empty ink = a text run that rendered nothing - must not happen for
        // non-empty visible content
        errs.push(`${el.rec.automation_id}: ${run.key} no ink inside rect`);
        continue;
      }
      if (left < rx - 1.5 || top < ry - 1.5 || right > rx + rw + 1.5 || bot > ry + rh + 1.5) {
        errs.push(`${el.rec.automation_id}: ${run.key} ink outside rect+/-1px (ink ${round(left - rx)},${round(top - ry)}..${round(right - rx)},${round(bot - ry)} of ${rw}x${rh})`);
        continue;
      }
      // Clip-edge check (R02 extension): when the element clips on an axis
      // (overflow hidden/auto/scroll/clip - e.g. the code-block figure), ink
      // must not reach the element's clip boundary. The rev-2 code-block
      // defect (missing px-4 py-3.5 on pre, "}" cut at the figure bottom)
      // passed ink-in-rect because the run rect hugged the text itself -
      // containment vs the run rect cannot see a missing inset.
      const CLIPS = new Set(["hidden", "auto", "scroll", "clip"]);
      const ebox = el.rec.box || {};
      const er = el.rec.rect;
      if (CLIPS.has(ebox.overflow_x) && (left <= er.x + 0.5 || right >= er.x + er.width - 0.5))
        errs.push(`${el.rec.automation_id}: ${run.key} ink touches clip edge (x)`);
      if (CLIPS.has(ebox.overflow_y) && (top <= er.y + 0.5 || bot >= er.y + er.height - 0.5))
        errs.push(`${el.rec.automation_id}: ${run.key} ink touches clip edge (y)`);
      // baseline validation: always inside the line box; for letter/digit
      // runs additionally inside the ink band extended by font descent below
      // (non-descender ink ends ~at the baseline but AA/optical rows can sit
      // 1-2px above it; descenders reach descent below). Punctuation-only
      // runs (ellipsis, separators) can paint entirely mid-line - line-box only.
      const punctOnly = !/[0-9A-Za-z]/.test(run.content);
      const descAllow = (run.font_descent || 4) + 1;
      for (const line of run.lines || []) {
        const ly = el.rec.rect.y + line.rect.y;
        const lx0 = el.rec.rect.x + line.rect.x;
        let ltop = 1e9, lbot = -1e9;
        for (let yy = ly; yy < ly + line.rect.height; yy++)
          for (let xx = lx0; xx < lx0 + line.rect.width; xx++) {
            const c = px(xx, yy);
            if (!c) continue;
            const l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
            if (isInk(l)) { ltop = Math.min(ltop, yy); lbot = Math.max(lbot, yy); }
          }
        if (lbot < ltop) continue; // whitespace line
        const baselinePage = el.rec.rect.y + line.baseline_y;
        const inBox = baselinePage >= ly - 1 && baselinePage <= ly + line.rect.height + 1;
        const inBand = baselinePage >= ltop - 1 && baselinePage <= lbot + descAllow;
        run.baseline_validated = punctOnly ? "line-box" : "ink-band";
        if (!inBox || (!punctOnly && !inBand)) {
          errs.push(`${el.rec.automation_id}: ${run.key} "${String(run.content).slice(0, 24)}" baseline ${round(baselinePage)} outside ${inBox ? `ink band [${round(ltop)},${round(lbot)}+desc]` : `line box [${round(ly)},${round(ly + line.rect.height)}]`}`);
        }
      }
    }
  }
  return errs;
}

// Caller owns the CDP session. MUST run after any DOM-mutating pass (the font
// proof): CDP-forced pseudo states are dropped when the DOM is mutated after
// forcing - that is what silently unstyled the light screenshots in rev 2.
async function forceStates(session) {
  const { root } = await session.send("DOM.getDocument");
  const { nodeIds } = await session.send("DOM.querySelectorAll", {
    nodeId: root.nodeId,
    selector: "[data-force-state]",
  });
  for (const nodeId of nodeIds) {
    const { attributes } = await session.send("DOM.getAttributes", { nodeId });
    const i = attributes.indexOf("data-force-state");
    await session.send("CSS.forcePseudoState", {
      nodeId,
      forcedPseudoClasses: attributes[i + 1].split(",").map((s) => s.trim()),
    });
  }
  return nodeIds.length;
}

// EXHAUSTIVE font proof (fail-closed): every element with a non-whitespace
// direct text node, and every input/textarea carrying value/placeholder text,
// must return a CSS.getPlatformFontsForNode result; every family must be the
// Geist custom font.
async function proofFonts(session, page) {
  // marks every rendered element carrying text + inserts a marked mirror div
  // for every input/textarea value/placeholder (control text is not a DOM
  // node - getPlatformFontsForNode returns nothing for it, so the mirror,
  // styled with the same font, is queried instead and then removed)
  const count = await page.evaluate(`
    (() => {
      let i = 0;
      const mirrors = [];
      for (const el of document.querySelectorAll("body *")) {
        const hasDomText = [...el.childNodes].some((n) => n.nodeType === 3 && n.nodeValue.trim());
        const ctrl = el.tagName === "INPUT" || el.tagName === "TEXTAREA";
        if (ctrl && (el.value || "").trim() + (el.placeholder || "")) {
          const cs = getComputedStyle(el);
          const m = document.createElement("div");
          m.style.cssText = "position:fixed;left:-9999px;top:0;visibility:hidden;font:" + cs.fontStyle + " " + cs.fontWeight + " " + cs.fontSize + "/" + cs.lineHeight + " " + cs.fontFamily;
          m.textContent = el.value || el.placeholder;
          m.setAttribute("data-fontproof", String(i++));
          document.body.appendChild(m);
          mirrors.push(m);
        } else if (hasDomText && el.getClientRects().length) {
          el.setAttribute("data-fontproof", String(i++));
        }
      }
      window.__fontMirrors = mirrors;
      return i;
    })()`);
  const { root } = await session.send("DOM.getDocument");
  const { nodeIds } = await session.send("DOM.querySelectorAll", {
    nodeId: root.nodeId,
    selector: "[data-fontproof]",
  });
  const failures = [];
  const resolved = { sans: new Set(), mono: new Set() };
  let proven = 0;
  for (const nodeId of nodeIds) {
    let fonts = null;
    try {
      fonts = (await session.send("CSS.getPlatformFontsForNode", { nodeId })).fonts;
    } catch (e) {
      failures.push(`getPlatformFontsForNode threw: ${e.message}`);
      continue;
    }
    if (!fonts || !fonts.length) {
      const { attributes } = await session.send("DOM.getAttributes", { nodeId });
      const k = attributes.indexOf("data-fontproof");
      failures.push(`no platform font result for text node #${attributes[k + 1]}`);
      continue;
    }
    for (const f of fonts) {
      if (f.familyName !== "Geist" && f.familyName !== "Geist Mono") {
        failures.push(`non-Geist font resolved: ${f.familyName}`);
      }
      if (!f.isCustomFont) failures.push(`font ${f.familyName} is not the custom (loaded) font`);
    }
    resolved[fonts[0].familyName === "Geist Mono" ? "mono" : "sans"].add(fonts[0].familyName);
    proven++;
  }
  // fully restore the DOM - the data-fontproof marks must not survive into
  // the capture pass (post-force DOM mutation drops the forced states)
  await page.evaluate(`(() => {
    (window.__fontMirrors || []).forEach((m) => m.remove());
    window.__fontMirrors = [];
    document.querySelectorAll("[data-fontproof]").forEach((e) => e.removeAttribute("data-fontproof"));
  })()`);
  return { marked: count, nodeCount: nodeIds.length, proven, failures, resolved };
}

async function settle(page) {
  await page.evaluate(
    `new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(r))))`,
  );
  await page.evaluate(() => document.fonts.ready);
}

// Binding eval: same keyed data-part chain resolution as the contract parts;
// runs on the live post-force DOM immediately before the screenshot.
const BINDING_EVAL = (bds) =>
  `(${JSON.stringify(
    bds.map((x) => [x.automation_id, x.part || "", x.css_property, x.capture_state || ""]),
  )}).map(([id, part, prop, cst]) => {
    const el = document.querySelector("[data-automation-id='" + id + "']");
    let target = el;
    if (el && part) {
      target = null;
      for (const pt of el.querySelectorAll("[data-part]")) {
        if (pt.closest("[data-automation-id]") !== el) continue;
        const chain = [];
        let cur = pt;
        while (cur && cur !== el && cur !== document.body) {
          if (cur.hasAttribute && cur.hasAttribute("data-part")) {
            const k = cur.getAttribute("data-key");
            chain.unshift(k ? cur.getAttribute("data-part") + "[" + k + "]" : cur.getAttribute("data-part"));
          }
          cur = cur.parentElement;
        }
        if (chain.join(".") === part) { target = pt; break; }
      }
    }
    if (!target) return [id, part, prop, cst, null];
    const cs = getComputedStyle(target);
    if (prop === "translate-x" || prop === "translate-y") {
      // Chromium serializes translate: calc(100% - 2px) 0px unresolved -
      // resolve % against the element's own box so the validator sees px
      const r = target.getBoundingClientRect();
      const resv = (s, size) => {
        const cm = /^calc\\(\\s*([\\d.]+)%\\s*([+-])\\s*([\\d.]+)px\\s*\\)$/.exec(String(s).trim());
        if (cm) return (parseFloat(cm[1]) / 100 * size + (cm[2] === "-" ? -1 : 1) * parseFloat(cm[3])) + "px";
        return s;
      };
      const t = String(cs.translate);
      const parts = t === "none" ? [] : (t.match(/calc\\([^)]*\\)|-?[\\d.]+px/g) || []);
      const comp = parts[prop === "translate-x" ? 0 : 1] || "0px";
      return [id, part, prop, cst, resv(comp, prop === "translate-x" ? r.width : r.height)];
    }
    const m = { "background-color": "backgroundColor", "color": "color", "border-color": "borderColor", "border-top-width": "borderTopWidth", "border-top-left-radius": "borderTopLeftRadius", "padding-left": "paddingLeft", "padding-right": "paddingRight", "padding-top": "paddingTop", "padding-bottom": "paddingBottom", "margin-top": "marginTop", "height": "height", "width": "width", "font-size": "fontSize", "font-weight": "fontWeight", "line-height": "lineHeight", "opacity": "opacity", "translate-x": "translate", "translate-y": "translate", "box-shadow-width": "boxShadow", "box-shadow-color": "boxShadow", "box-shadow-alpha": "boxShadow", "box-shadow-layers": "boxShadow" }[prop];
    return [id, part, prop, cst, cs[m]];
  })`;

const { IN_PAGE_MEASURE, STATE_ASSERT } = require("./measure");

// R01 pressed parity: pressed vs normal specimen y-offset (translate-y-px),
// measured on the live post-force DOM.
const PRESSED_EVAL = `(() => {
  const out = [];
  for (const v of ["default", "secondary", "outline", "ghost", "destructive", "link"]) {
    for (const comp of ["button", "icon-button"]) {
      const n = document.querySelector("[data-automation-id='" + comp + "." + v + "']");
      const p = document.querySelector("[data-automation-id='" + comp + "." + v + ".pressed']");
      if (!n || !p) continue;
      const a = n.getBoundingClientRect(), b = p.getBoundingClientRect();
      out.push({ id: comp + "." + v, dy: b.y - a.y });
    }
  }
  return out;
})()`;

// Forced-state specimen -> same-size unforced twin. Specimens whose forcing
// was lost before the screenshot paint identically to the twin - the pixel
// guard fails on identical crops. Twins carry identical content where one
// exists. Excluded: *.invalid-focus-visible (the only computed delta vs
// .invalid is outline-width 3px->1px, which does not paint over the ring -
// verified identical crops; the diff is recorded in coverage.json) and the
// highlighted menu items (no unforced same-content twin).
function pixelTwin(id, byId) {
  const m = id.match(/\.(hover|pressed|focus-visible|focused)$/);
  if (!m) return null;
  const base = id.slice(0, m.index);
  for (const sfx of ["", ".off", ".active", ".checked", ".selected", ".filled", ".unchecked", ".placeholder", ".unselected", ".inactive"]) {
    const cand = base + sfx;
    if (cand !== id && byId[cand]) return cand;
  }
  return null;
}

async function main() {
  const outDir = process.env.CAPTURE_OUT || ROOT;
  const shotsLight = path.join(outDir, "screenshots", "light");
  const shotsDark = path.join(outDir, "screenshots", "dark");
  fs.mkdirSync(shotsLight, { recursive: true });
  fs.mkdirSync(shotsDark, { recursive: true });

  const browser = await launchCanonical();
  const chromiumVersion = browser.version();

  const pages = [
    "all", "components", "forms", "navigation", "typography", "overlays", "native-text",
  ];
  const catalogById = {};
  const elements = {};
  const docSizes = {};
  const fontsReport = { marked: 0, proven: 0 };
  let geoDivergences = 0;
  const forcedVerified = [];
  const statesByPage = {};
  let fontResolved = { sans: new Set(), mono: new Set() };
  const allErrors = [];
  const live = {};          // same-session binding values (contract.bindings_live)
  const textInventories = {}; // per page/state owned-text inventory (C02)
  const pressedPairs = [];  // same-session R01 evidence (contract.proofs)
  let pixelGuardPairs = 0;
  let pixelGuardSkipped = 0;

  for (const theme of ["light", "dark"]) {
    const context = await browser.newContext({
      viewport: VIEWPORT,
      deviceScaleFactor: 1,
      reducedMotion: "reduce",
      colorScheme: theme,
    });
    const page = await context.newPage();
    let curPage = "boot";
    page.on("pageerror", (err) => {
      allErrors.push(`page render threw (${curPage}): ${err.message}`);
    });
    for (const pg of pages) {
      curPage = pg;
      const states = CAPTURE_STATES[pg] || ["default"];
      statesByPage[pg] = states;
      for (const st of states) {
        const url = `${URL_BASE}?page=${pg}&theme=${theme}&capture=1${st === "default" ? "" : `&state=${st}`}`;
        await page.goto(url);
        await page.waitForSelector("[data-render-done]");
        await page.evaluate(() => document.fonts.ready);

        const session = await page.context().newCDPSession(page);
        await session.send("DOM.enable");
        await session.send("CSS.enable");

        if (theme === "light") {
          const meta = await page.evaluate(
            `window.CATALOG.map(e => ({automation_id: e.automation_id, component: e.component, variant: e.variant, state: e.state, size: e.size || null, category: e.category, kind: e.kind || "control", tier: e.tier || null, family: e.family || null, meta: e.meta || null}))`,
          );
          for (const m of meta) catalogById[m.automation_id] = m;
          // exhaustive font proof once per page, BEFORE forcing - it mutates
          // the DOM (data-fontproof attrs + mirror nodes) and CDP-forced
          // pseudo states do not survive DOM mutation; proofFonts removes
          // every mutation before returning so forcing starts on a pristine
          // document.
          if (st === "default") {
            const pf = await proofFonts(session, page);
            fontsReport.marked += pf.marked;
            fontsReport.proven += pf.proven;
            for (const f of pf.failures) allErrors.push(`font proof ${pg}: ${f}`);
            for (const k of ["sans", "mono"]) for (const v of pf.resolved[k]) fontResolved[k].add(v);
          }
        }

        // R01: force the pseudo states, settle, THEN measure and screenshot -
        // the exported contract describes the painted state exactly.
        const nForced = await forceStates(session);
        await settle(page);

        // every rendered <svg> must contain geometry
        const badIcons = await page.evaluate(
          `[...document.querySelectorAll("svg")].filter(s => !s.querySelector("path,circle,rect,line,polygon,polyline,ellipse")).map(s => s.outerHTML.slice(0, 120))`,
        );
        if (badIcons.length) allErrors.push(`empty svg(s) on ${pg}/${st}/${theme}: ${badIcons.join(" | ")}`);

        // V04: capture-state sanity before measuring - a contaminated
        // document (scrolled, wrong focus/selection) must never reach the
        // contract or the screenshot
        const stateErrsPre = await page.evaluate(STATE_ASSERT);
        for (const e of stateErrsPre) allErrors.push(`capture-state ${pg}/${st}/${theme} pre-measure: ${e}`);

        const measured = await page.evaluate(IN_PAGE_MEASURE);
        for (const e of measured.errors) allErrors.push(`${pg}/${st}/${theme}: ${e}`);
        docSizes[`${pg}/${st}/${theme}`] = measured.doc;

        if (theme === "light") {
          for (const mrec of measured.elements) {
            const id = mrec.automation_id;
            if (!elements[id]) elements[id] = { captures: {}, slot: mrec.slot, tag: mrec.tag, force_state: mrec.force_state };
            const acc = elements[id];
            // identity across captures is required: data-slot/tag must not
            // differ between the places the same automation id renders
            if (acc.slot !== mrec.slot || acc.tag !== mrec.tag)
              allErrors.push(`identity divergence ${id}: ${pg}/${st} slot=${mrec.slot} tag=${mrec.tag} vs earlier slot=${acc.slot} tag=${acc.tag}`);
            acc.captures[`${pg}/${st}`] = {
              rect: mrec.rect, box: mrec.box, parts: mrec.parts,
            };
            if (mrec.interaction) acc.captures[`${pg}/${st}`].interaction = mrec.interaction;
            if (mrec.text) acc.captures[`${pg}/${st}`].text = mrec.text;
            if (mrec.text_runs && mrec.text_runs.length) acc.captures[`${pg}/${st}`].text_runs = mrec.text_runs;
          }
          textInventories[`${pg}/${st}`] = measured.textInventory;
          const hov = await page.evaluate(() => {
            const a = document.querySelector("[data-automation-id='button.default']");
            const b = document.querySelector("[data-automation-id='button.default.hover']");
            if (!a || !b) return null;
            return {
              normal: getComputedStyle(a).backgroundColor,
              forced: getComputedStyle(b).backgroundColor,
            };
          });
          if (hov) forcedVerified.push({ page: pg, state: st, ...hov, applies: hov.normal !== hov.forced });
        } else {
          // dark pass: assert geometry identical to light, per capture
          for (const mrec of measured.elements) {
            const id = mrec.automation_id;
            const light = elements[id] && elements[id].captures[`${pg}/${st}`];
            if (!light) continue;
            const lr = light.rect;
            const same =
              lr.x === mrec.rect.x && lr.y === mrec.rect.y &&
              lr.width === mrec.rect.width && lr.height === mrec.rect.height;
            if (!same) {
              geoDivergences++;
              allErrors.push(`GEOMETRY DIVERGENCE ${id} ${pg}/${st}: light=${JSON.stringify(lr)} dark=${JSON.stringify(mrec.rect)}`);
            }
          }
        }

        // R04 + R01 evidence evaluated in THIS page/session/state, after
        // measurement and immediately before the screenshot - the recorded
        // live values describe exactly the painted frame (both themes).
        const bdsHere = bindingsDoc.bindings.filter((bd) => (bd.capture_state || "default") === st);
        if (bdsHere.length) {
          const vals = await page.evaluate(BINDING_EVAL(bdsHere));
          for (const [id, part, prop, cst, v] of vals) {
            if (v === null || v === undefined) continue;
            const k2 = `${id}|${part}|${theme}|${cst === "default" ? "" : cst}`;
            // same id + same part + same state must evaluate identically on
            // every page that renders it (the key has no page on purpose)
            if (live[k2] && prop in live[k2] && live[k2][prop] !== v) {
              allErrors.push(`bindings divergence ${k2} ${prop}: '${live[k2][prop]}' vs '${v}'`);
            } else {
              (live[k2] = live[k2] || {})[prop] = v;
            }
          }
        }
        for (const pr of await page.evaluate(PRESSED_EVAL)) {
          pressedPairs.push({ page: pg, state: st, theme, id: pr.id, dy: round(pr.dy) });
          if (Math.abs(pr.dy - 1) > 0.01)
            allErrors.push(`R01 ${pg}/${st}/${theme} ${pr.id}: pressed dy ${round(pr.dy)}px != 1px (translate-y-px)`);
        }

        // V04: the applied capture state must still hold for the screenshot
        const stateErrsShot = await page.evaluate(STATE_ASSERT);
        for (const e of stateErrsShot) allErrors.push(`capture-state ${pg}/${st}/${theme} pre-shot: ${e}`);

        const name = st === "default" ? pg : `${pg}--${st}`;
        const shotBuf = await page.screenshot({
          path: path.join(theme === "light" ? shotsLight : shotsDark, `${name}.png`),
          fullPage: true,
          animations: "disabled",
          caret: "hide",
        });
        // R02: ink validation against the screenshot just taken (post-force state)
        const png = decodePng(shotBuf);
        const pageEls = measured.elements.map((r) => ({ rec: r }));
        for (const e of validateInk(png, 0, pageEls)) allErrors.push(`ink ${pg}/${st}/${theme}: ${e}`);

        // Pixel guard: a forced-state specimen must NOT paint identically to
        // its same-size normal twin in this screenshot. Catches the class of
        // bug where the forced state is lost after measurement (rev-2 light
        // font-proof mutation) while every non-pixel check still passes.
        const byId = {};
        for (const mrec of measured.elements) byId[mrec.automation_id] = mrec;
        for (const el of measured.elements) {
          const twinId = pixelTwin(el.automation_id, byId);
          if (!twinId) continue;
          const twin = byId[twinId];
          if (Math.abs(twin.rect.width - el.rect.width) > 0.5 || Math.abs(twin.rect.height - el.rect.height) > 0.5) {
            pixelGuardSkipped++;
            continue;
          }
          // pressed specimens paint 1px lower via translate-y-px - the rect
          // follows the shift, so compare at the UN-translated box and let
          // the shifted content itself produce the difference
          const dy = el.automation_id.endsWith(".pressed") ? el.rect.y - twin.rect.y : 0;
          const fRect = { x: el.rect.x, y: el.rect.y - dy, width: el.rect.width, height: el.rect.height };
          const err = forcedPairError(png, fRect, twin.rect, `${el.automation_id} vs ${twinId}`);
          if (err) allErrors.push(`PIXEL GUARD ${pg}/${st}/${theme}: ${err}`);
          else pixelGuardPairs++;
        }
        await session.detach().catch(() => {});
      }
    }
    await context.close();
  }

  // System-mode proof: emulated dark -> .dark; emulated light -> not.
  const sys = {};
  for (const scheme of ["dark", "light"]) {
    const context = await browser.newContext({
      viewport: VIEWPORT, deviceScaleFactor: 1, reducedMotion: "reduce",
      colorScheme: scheme,
    });
    const page = await context.newPage();
    await page.goto(`${URL_BASE}?page=components&theme=system&capture=1`);
    await page.waitForSelector("[data-render-done]");
    sys[scheme] = await page.evaluate(
      `document.documentElement.classList.contains('dark')`,
    );
    await context.close();
  }
  if (!(sys.dark === true && sys.light === false)) {
    allErrors.push(`SYSTEM THEME MISMATCH: ${JSON.stringify(sys)}`);
  }

  const hoverOk = forcedVerified.length && forcedVerified.every((v) => v.applies);
  if (!hoverOk) allErrors.push("forced-hover verification failed on some page");

  // The binding values were recorded in the same session/state as each
  // screenshot - validate them against tokens.json here (R04 authority).
  {
    const { errs, checked } = V.validateBindings(tokens, bindingsDoc, live);
    for (const e of errs) allErrors.push(`binding ${e}`);
    console.log(`bindings: ${checked} live comparisons evaluated in screenshot state`);
  }
  console.log(`pixel guard: ${pixelGuardPairs} forced-vs-twin pairs verified, ${pixelGuardSkipped} size-mismatched/skipped`);
  if (!pixelGuardPairs) allErrors.push("PIXEL GUARD ran zero pairs - forced-state coverage missing");

  // ---------- contract.json ----------
  const idVocab = {
    scheme: "<component>.<variant>[.<modifier>]* - lowercase kebab, dot-separated",
    states: ["hover","pressed","focus-visible","disabled","checked","unchecked","on","off","active","inactive","invalid","invalid-focus-visible","disabled-checked","invalid-checked","placeholder","filled","focused","selection","readonly","open","current"],
    sizes: ["size-xs","size-sm","size-lg","size-icon","size-icon-xs","size-icon-sm","size-icon-lg"],
    overlay_parts: ["trigger","content"],
    part_keys: "parts are keyed: <part>[<data-key>] chained by ancestor parts, e.g. item[production].indicator, row[0].cell[1]; generation fails on duplicate keys",
  };
  const elsOut = [];
  for (const [id, v] of Object.entries(elements)) {
    const baseId = id.replace(/\.(trigger|content)$/, "");
    // sub-elements (e.g. *.item-highlighted) carry their own automation id
    // but share the parent specimen's tier/family - walk dot prefixes
    let meta = catalogById[id] || catalogById[baseId];
    if (!meta) {
      let prefix = id;
      while (prefix.includes(".")) {
        prefix = prefix.slice(0, prefix.lastIndexOf("."));
        if (catalogById[prefix]) { meta = catalogById[prefix]; break; }
      }
    }
    meta = { ...(meta || {}) };
    // settings-nav.<item> ids inside the stage are nav items, not catalog
    // specimens - assign settings-nav-item family/parent explicitly
    if (/^settings-nav\.(?!default)/.test(id)) {
      meta.component = "settings-nav-item";
      meta.family = "settings-nav";
      meta.tier = "core";
      meta.meta = { ...(meta.meta || {}), parent: "settings-nav.default" };
    }
    const kind =
      id.startsWith("shell.") ? "shell"
      : /\.(trigger)$/.test(id) ? "overlay-trigger"
      : /\.(content)$/.test(id) ? "overlay-content"
      : meta.kind === "overlay" ? "stage"
      : meta.kind || "control";
    const entry = {
      automation_id: id,
      component: id.startsWith("shell.") ? "gallery-shell" : (meta.component || id.split(".")[0]),
      family: id.startsWith("shell.") ? "gallery-shell" : (meta.family || meta.component || id.split(".")[0]),
      tier: id.startsWith("shell.") ? "core" : (meta.tier || null),
      variant: meta.variant ?? null,
      state: meta.state ?? null,
      size: meta.size ?? null,
      category: meta.category || null,
      kind,
      data_slot: v.slot,
      tag: v.tag,
    };
    if (kind === "overlay-content") {
      entry.trigger = id.replace(/\.content$/, ".trigger");
      entry.anchor = meta.meta?.anchor || null;
    }
    if (kind === "stage" && meta.meta?.anchor) entry.anchor = meta.meta.anchor;
    if (meta.meta?.parent) entry.parent = meta.meta.parent;
    if (meta.meta?.presentation) entry.presentation = meta.meta.presentation;
    entry.captures = {};
    for (const k of Object.keys(v.captures).sort()) entry.captures[k] = v.captures[k];
    if (v.force_state) entry.forced_state = v.force_state;
    elsOut.push(entry);
  }
  elsOut.sort((a, b) => a.automation_id.localeCompare(b.automation_id));

  // C01-d same-state consistency (formal V04 invariant): same automation id
  // + same capture state on different pages must record identical
  // appearance (box, interaction, part paint, text_runs, element size) -
  // exceptions are only the explicit SAME_STATE_EXCEPTIONS list.
  {
    const { errs, checked } = V.validateSameStateConsistency({ elements: elsOut });
    for (const e of errs) allErrors.push(e);
    console.log(`same-state consistency: ${checked.ids} multi-page ids, ${checked.pairs} same-state pairs compared`);
  }

  const contract = {
    "$schema": "rust-ui.shadcn-reference.contract/0.2",
    reference_version: "0.1",
    candidate_revision: 3,
    coordinate_space: {
      unit: "css-px",
      rust_ui_equivalent: "Dp (1 css-px = 1 rust-ui logical Dp)",
      origin: "document top-left",
      viewport: VIEWPORT,
      device_scale_factor: 1,
      zoom: 1,
    },
    id_vocabulary: idVocab,
    pages: pages.map((p) => ({
      id: p,
      capture_states: statesByPage[p] || ["default"],
      document: docSizes[`${p}/default/light`],
    })),
    notes: {
      measurement_order:
        "goto -> [data-render-done] -> fonts.ready -> font proof (light+default only; data-fontproof mutation fully reverted) -> capture-state assertions -> CDP forcePseudoState -> settle (3x rAF + fonts.ready) -> measure -> token-binding eval + pressed parity in the same session -> capture-state assertions -> screenshot -> pixel guard + ink validation -> store under captures[<page>/<state>] -> session detach: the contract records the PAINTED post-force state (R01); bindings_live values were read immediately before each screenshot",
      baseline_formula:
        "baseline_y = line rect top + fontBoundingBoxAscent of the run's effective font (canvas measureText on that font); each line validated against rendered ink: inside the line box always, and inside the ink band (+descent) for letter/digit runs",
      control_text:
        "input/textarea text_runs are measured via a mirror div at the control's position: same font/letter-spacing/text-indent/whitespace and padding, but border-width suppressed (the control's border would shift the text origin); browser editing behavior is not authority",
      geometry:
        "captures[<page>/<state>] measured post-force in light theme; dark geometry asserted identical per capture; hidden parts record {visible:false, reason:'not-rendered'}",
      text_ownership:
        "data-part-owner attributes parts/text outside an element's subtree to it (owner parts carry label_gap); every visible text node must resolve to an owner or a data-text-exempt gallery-chrome class (contract.text_inventory)",
      pixel_guard:
        "every forced-state specimen with a same-size unforced twin must produce a different screenshot crop (4px pad absorbs the pressed +1px translate)",
    },
    text_inventory: textInventories,
    proofs: {
      pressed_pairs: pressedPairs,
      pixel_guard: { pairs_checked: pixelGuardPairs, size_mismatch_or_no_twin: pixelGuardSkipped },
    },
    bindings_live: live,
    elements: elsOut,
  };
  fs.writeFileSync(path.join(outDir, "contract.json"), JSON.stringify(contract, null, 2));

  // ---------- reference.json ----------
  const reference = {
    "$schema": "rust-ui.shadcn-reference.reference/0.1",
    reference_version: "0.1",
    candidate_revision: 3,
    status: "candidate - pre-freeze review revision; freeze policy applies after split review approval",
    authority: {
      site: "https://ui.shadcn.com/",
      urls_reviewed: [
        "/docs/theming",
        "/docs/components",
        "/docs/changelog/2025-12-shadcn-create",
        ...["button","badge","separator","card","alert","skeleton","kbd","label","input","textarea","checkbox","radio-group","switch","slider","select","field","tabs","breadcrumb","pagination","toggle","toggle-group","button-group","dialog","alert-dialog","popover","tooltip","dropdown-menu","typography","sidebar"].map((c) => `/docs/components/base/${c}`),
      ],
      upstream_repo: "github.com/shadcn-ui/ui",
      upstream_commit: "295a1f114a138f23b5dfee0e0c6812394dfeb90c",
      upstream_files: [
        "apps/v4/registry/styles/style-nova.css",
        "packages/shadcn/src/tailwind.css",
        "apps/v4/registry/themes.ts (theme neutral - theme-neutral.css generated by scripts/theme.js)",
        "apps/v4/app/globals.css (surface/code/selection vars - see src/theme-local.css)",
        "apps/v4/mdx-components.tsx (code-block surface styling)",
        "apps/v4/registry/bases/base/ui/*.tsx",
        "apps/v4/registry/bases/base/examples/*-example.tsx",
        "apps/v4/registry/bases/base/blocks/sidebar-13/ (settings-nav authority)",
      ],
    },
    authority_review_date: "2026-10-05",
    scope: "rust-ui is desktop-only + fully desktop-responsive; mobile is out of scope (responsive != mobile). The 1440x1000 DPR1 viewport is the deterministic CAPTURE viewport, not a fixed-size layout contract. Upstream mobile paths (Sidebar Sheet/offcanvas/useIsMobile) stay in vendored files untouched and are never transcribed.",
    tiers: {
      core: "required for the first native rust-ui Gallery/component milestone - complete visually material desktop state/size coverage",
      later: "planned desktop component, not in the first native milestone - representative coverage",
      "reference-only": "retained visual coverage, no native promise",
    },
    style: {
      library: "base",
      style: "nova",
      base_color: "neutral",
      icon_library: "lucide",
      icon_package: "lucide-static@1.21.0",
      icon_license: "vendor/lucide/LICENSE (ISC + retained Feather MIT notices)",
      font_package: "geist@1.7.2",
      font_license: "vendor/geist/LICENSE (SIL OFL 1.1)",
      radius: "default",
      menu_accent: "subtle",
      menu_color: "default",
    },
    install_transforms: {
      note: "emulated upstream install-time transformers for this config (packages/registry/src/utils/transformers)",
      menu: "transform-menu.ts - menuColor=default removes cn-menu-target AND cn-menu-translucent (they are placeholders; only *-translucent inline the translucent classes, inverted* swaps cn-menu-target->'dark')",
      rtl: "transform-rtl.ts - rtl=false -> skipped (no direction/logical-side transform, cn-rtl-flip markers absent)",
      font: "transform-font.ts - cn-font-heading retained; --font-heading: var(--font-sans) in gallery.css -> resolves to Geist",
      icons: "transform-icons.ts - iconLibrary=lucide -> no-op (source paths already lucide)",
      cleanup: "transform-cleanup.ts - strips transient cn-* markers only (cn-rtl-flip, cn-logical-sides); recipe cn-* classes retained",
      tw_prefix: "none configured -> no-op",
    },
    font: {
      canonical: "Geist / Geist Mono - upstream Nova canonical fonts, vendored OFL woff2",
      css_stack: `"Geist", ui-sans-serif, system-ui, sans-serif`,
      mono_stack: `"Geist Mono", ui-monospace, monospace`,
      files: {
        sans: "vendor/geist/Geist-Variable.woff2",
        mono: "vendor/geist/GeistMono-Variable.woff2",
      },
      proof: `EXHAUSTIVE fail-closed: CSS.getPlatformFontsForNode on every element with a non-whitespace direct text node + every input/textarea value/placeholder; empty/error/non-Geist/non-custom result fails capture. marked=${fontsReport.marked} proven=${fontsReport.proven} resolved sans=[${[...fontResolved.sans]}] mono=[${[...fontResolved.mono]}]`,
    },
    viewport: VIEWPORT,
    device_scale_factor: 1,
    zoom: 1,
    capture_platform: {
      os: process.platform,
      chromium: chromiumVersion,
      playwright: "1.63.0",
      deterministic: "fixed viewport/dpr/zoom; animations+transitions disabled; caret hidden; no timestamps/randomness/network",
    },
    theme_modes: ["light", "dark", "system"],
    canonical_pages: pages,
    capture_states: statesByPage,
    git_base: {
      foundation_review: "e827f3dfb5d29e50bcc64f90e5d735624641d878",
      frozen_foundation_production: "00dc29acfeb686d6a190d91624de7ab9a48e1e92",
    },
    devctl_bridge:
      "automation ids == future rust-ui .automation_id(...) == UIA AutomationId; future rust-ui-devctl rect/hover/click/focus/type/snapshot-layout/compare consume contract.json",
  };
  fs.writeFileSync(path.join(outDir, "reference.json"), JSON.stringify(reference, null, 2));

  const hashes = {};
  for (const theme of ["light", "dark"]) {
    const dir = path.join(outDir, "screenshots", theme);
    for (const f of fs.readdirSync(dir).sort()) {
      const b = fs.readFileSync(path.join(dir, f));
      hashes[`screenshots/${theme}/${f}`] = crypto.createHash("sha256").update(b).digest("hex");
    }
  }
  fs.writeFileSync(
    path.join(outDir, "screenshots", "SHA256.json"),
    JSON.stringify(hashes, null, 2),
  );

  if (geoDivergences) allErrors.push(`${geoDivergences} light/dark geometry divergences`);
  for (const e of allErrors) console.error(e);
  if (allErrors.length) process.exitCode = 1;
  console.log(`capture: ${elsOut.length} elements, ${Object.keys(hashes).length} screenshots, fonts sans=[${[...fontResolved.sans]}] mono=[${[...fontResolved.mono]}] (${fontsReport.proven}/${fontsReport.marked} proven)`);
  console.log(`chromium ${chromiumVersion}`);
  if (process.exitCode) console.error(`CAPTURE FAILED - ${allErrors.length} error(s)`);
  else console.log("capture ok");
  await browser.close();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
