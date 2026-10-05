// Deterministic capture: renders each page in light+dark under fixed
// viewport, applies forced pseudo states via CDP, takes full-page
// screenshots, measures geometry into contract.json, and proves font
// resolution via CSS.getPlatformFontsForNode. Emits reference.json.
const { chromium } = require("playwright");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");

const ROOT = path.join(__dirname, "..");
const URL_BASE = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");
const VIEWPORT = { width: 1440, height: 1000 };
const round = (n) => Math.round(n * 1000) / 1000;

const CAPTURE_STATES = {
  "native-text": ["default", "multiline-selection"],
};

const IN_PAGE_MEASURE = `(() => {
  const round = (n) => Math.round(n * 1000) / 1000;
  const px = (v) => (v.endsWith("px") ? parseFloat(v) : v === "none" || v === "normal" ? v : parseFloat(v) || 0);
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
      },
      parts: {},
      text: null,
    };
    for (const p of el.querySelectorAll("[data-part]")) {
      const pr = p.getBoundingClientRect();
      rec.parts[p.getAttribute("data-part")] = {
        x: round(pr.x - r.x), y: round(pr.y - r.y),
        width: round(pr.width), height: round(pr.height),
      };
    }
    // text metrics for text-bearing elements
    const textContent =
      el.tagName === "INPUT" || el.tagName === "TEXTAREA"
        ? el.value || el.placeholder
        : (() => { let s = ""; for (const n of el.childNodes) if (n.nodeType === 3) s += n.nodeValue; return s; })() || el.textContent;
    if (textContent && textContent.trim()) {
      const family = cs.fontFamily;
      const size = cs.fontSize;
      const weight = cs.fontWeight;
      const lh = cs.lineHeight;
      // text rect: Range over the element's text NODES only - never over
      // child elements (icons, arrows) which would pollute the bounds
      let textRect = null;
      try {
        const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
        const textNodes = [];
        let n;
        while ((n = walker.nextNode())) if (n.nodeValue.trim()) textNodes.push(n);
        if (textNodes.length) {
          const range = document.createRange();
          range.setStartBefore(textNodes[0]);
          range.setEndAfter(textNodes[textNodes.length - 1]);
          const rr = range.getBoundingClientRect();
          textRect = { x: round(rr.x - r.x), y: round(rr.y - r.y), width: round(rr.width), height: round(rr.height) };
        }
      } catch (_) {}
      // baseline proxy: canvas measureText ascent relative to the text rect top
      let ascent = null, descent = null;
      try {
        const c = document.createElement("canvas").getContext("2d");
        c.font = \`\${cs.fontStyle} \${cs.fontWeight} \${cs.fontSize} \${cs.fontFamily}\`;
        const m = c.measureText(textContent.trim().slice(0, 64));
        ascent = m.fontBoundingBoxAscent; descent = m.fontBoundingBoxDescent;
      } catch (_) {}
      rec.text = {
        content: textContent.trim().slice(0, 200),
        rect: textRect,
        font_family: family,
        font_size: round(parseFloat(size)),
        font_weight: parseInt(weight, 10),
        line_height: lh === "normal" ? "normal" : round(parseFloat(lh)),
        // baseline_proxy_y = text_rect.y + fontBoundingBoxAscent (canvas, same
        // font shorthand); approximates distance from element top to baseline.
        baseline_proxy_y:
          textRect && ascent != null ? round(textRect.y + ascent) : null,
      };
    }
    out.push(rec);
  }
  return { elements: out, doc: { width: round(document.documentElement.scrollWidth), height: round(document.documentElement.scrollHeight) } };
})()`;

async function forceStates(page) {
  const session = await page.context().newCDPSession(page);
  await session.send("DOM.enable");
  await session.send("CSS.enable");
  const { root } = await session.send("DOM.getDocument");
  const { nodeIds } = await session.send("DOM.querySelectorAll", {
    nodeId: root.nodeId,
    selector: "[data-force-state]",
  });
  const forced = [];
  for (const nodeId of nodeIds) {
    const { attributes } = await session.send("DOM.getAttributes", { nodeId });
    const i = attributes.indexOf("data-force-state");
    const val = attributes[i + 1];
    const classes = val.split(",").map((s) => s.trim());
    await session.send("CSS.forcePseudoState", {
      nodeId,
      forcedPseudoClasses: classes,
    });
    const aid = attributes.indexOf("data-automation-id");
    forced.push({ nodeId, classes, id: aid >= 0 ? attributes[aid + 1] : null });
  }
  return { session, forced };
}

async function proofFonts(session, page, ids) {
  const { root } = await session.send("DOM.getDocument");
  const out = {};
  for (const id of ids) {
    const sel = `[data-automation-id='${id}']`;
    const { nodeIds } = await session.send("DOM.querySelectorAll", {
      nodeId: root.nodeId,
      selector: sel,
    });
    if (!nodeIds.length) continue;
    let fonts = null;
    try {
      const res = await session.send("CSS.getPlatformFontsForNode", {
        nodeId: nodeIds[0],
      });
      fonts = res.fonts.map((f) => f.familyName);
    } catch (_) {}
    if (fonts && fonts.length) out[id] = fonts;
  }
  return out;
}

async function main() {
  const outDir = process.env.CAPTURE_OUT || ROOT;
  const shotsLight = path.join(outDir, "screenshots", "light");
  const shotsDark = path.join(outDir, "screenshots", "dark");
  fs.mkdirSync(shotsLight, { recursive: true });
  fs.mkdirSync(shotsDark, { recursive: true });

  const browser = await chromium.launch({
    channel: 'chromium',
    // deterministic rasterization: software GL, sRGB profile, grayscale AA
    args: ['--disable-gpu', '--force-color-profile=srgb', '--disable-lcd-text'],
  });
  const chromiumVersion = browser.version();

  const pages = [
    "all",
    "components",
    "forms",
    "navigation",
    "typography",
    "overlays",
    "native-text",
  ];
  const catalogById = {};
  const elements = {};
  const docSizes = {};
  let fontsReport = {};
  let geoDivergences = 0;
  const forcedVerified = [];
  const statesByPage = {};

  for (const theme of ["light", "dark"]) {
    const context = await browser.newContext({
      viewport: VIEWPORT,
      deviceScaleFactor: 1,
      reducedMotion: "reduce",
      colorScheme: theme,
    });
    const page = await context.newPage();
    page.on("pageerror", (err) => { console.error(`page render threw (${pg || "boot"}): ${err.message}`); process.exitCode = 1; });
    for (const pg of pages) {
      const states = CAPTURE_STATES[pg] || ["default"];
      statesByPage[pg] = states;
      for (const st of states) {
        const url = `${URL_BASE}?page=${pg}&theme=${theme}&capture=1${st === "default" ? "" : `&state=${st}`}`;
        await page.goto(url);
        await page.waitForSelector("[data-render-done]");
        await page.evaluate(() => document.fonts.ready);

        // every rendered <svg> must contain geometry - a missing icon in the
        // subset must fail loudly, not paint an empty square
        const badIcons = await page.evaluate(
          `[...document.querySelectorAll("svg")].filter(s => !s.querySelector("path,circle,rect,line,polygon,polyline,ellipse")).map(s => s.outerHTML.slice(0, 120))`,
        );
        if (badIcons.length) { console.error(`empty svg(s) on ${pg}/${st}/${theme}: ${badIcons.join(" | ")}`); process.exitCode = 1; }

        // measure + font proof BEFORE forcing pseudo states: forcing is the
        // last mutation before the screenshot so nothing can invalidate it.
        // (Forced pseudo states only change paint colors/rings - geometry is
        // identical, so measuring pre-force is sound.)
        const measured = await page.evaluate(IN_PAGE_MEASURE);
        docSizes[`${pg}/${st}/${theme}`] = measured.doc;

        const session = await page.context().newCDPSession(page);
        await session.send("DOM.enable");
        await session.send("CSS.enable");

        if (theme === "light") {
          // catalog metadata for each element
          const meta = await page.evaluate(
            `window.CATALOG.map(e => ({automation_id: e.automation_id, component: e.component, variant: e.variant, state: e.state, size: e.size || null, category: e.category, kind: e.kind || "control", meta: e.meta || null}))`,
          );
          for (const m of meta) catalogById[m.automation_id] = m;
          for (const mrec of measured.elements) {
            const id = mrec.automation_id;
            elements[id] = elements[id] || { rec: mrec, placements: [] };
            elements[id].rec = mrec;
            elements[id].placements.push({
              page: pg,
              capture_state: st,
              x: mrec.rect.x,
              y: mrec.rect.y,
              width: mrec.rect.width,
              height: mrec.rect.height,
            });
          }
          // font proof on this page's text-bearing elements
          const textIds = measured.elements.filter((e) => e.text).map((e) => e.automation_id);
          Object.assign(fontsReport, await proofFonts(session, page, textIds));
        } else {
          // dark pass: assert geometry identical to light
          for (const mrec of measured.elements) {
            const id = mrec.automation_id;
            const light = elements[id] && elements[id].placements.find((p) => p.page === pg && p.capture_state === st);
            if (!light) continue;
            const same =
              light.x === mrec.rect.x && light.y === mrec.rect.y &&
              light.width === mrec.rect.width && light.height === mrec.rect.height;
            if (!same) {
              geoDivergences++;
              console.error(`GEOMETRY DIVERGENCE ${id} ${pg}/${st}: light=${JSON.stringify(light)} dark=${JSON.stringify(mrec.rect)}`);
            }
          }
        }

        // force pseudo states LAST, immediately before the screenshot
        const { forced } = await forceStates(page);
        if (theme === "light") {
          // verify a forced hover actually changes style (upstream CSS authority)
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
        }
        const name = st === "default" ? pg : `${pg}--${st}`;
        await page.screenshot({
          path: path.join(theme === "light" ? shotsLight : shotsDark, `${name}.png`),
          fullPage: true,
          animations: "disabled",
          caret: "hide",
        });
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
    console.error(`SYSTEM THEME MISMATCH: ${JSON.stringify(sys)}`);
    process.exitCode = 1;
  }

  // font assertions
  let fontFail = 0;
  const resolved = { sans: new Set(), mono: new Set() };
  for (const [id, families] of Object.entries(fontsReport)) {
    const el = elements[id];
    const fam = el?.rec?.text?.font_family || "";
    const isMono = /mono|cascadia|consolas/i.test(fam);
    const want = isMono ? "Cascadia Mono" : "Segoe UI Variable";
    const first = families[0] || "";
    if (!first.startsWith(want)) {
      // getPlatformFonts returns fonts actually used for glyphs; the FIRST
      // family must be the resolved one.
      console.error(`FONT FAIL ${id}: want ${want}, got [${families.join(", ")}] (computed '${fam}')`);
      fontFail++;
    }
    resolved[isMono ? "mono" : "sans"].add(first);
  }
  if (fontFail) {
    console.error(`font proof: ${fontFail} failures`);
    process.exitCode = 1;
  }

  const hoverOk = forcedVerified.length && forcedVerified.every((v) => v.applies);
  if (!hoverOk) {
    console.error("forced-hover verification failed on some page");
    process.exitCode = 1;
  }

  // ---------- contract.json ----------
  const idVocab = {
    scheme: "<component>.<variant>[.<modifier>]* - lowercase kebab, dot-separated",
    states: ["hover","pressed","focus-visible","disabled","checked","unchecked","indeterminate","on","off","active","inactive","invalid","placeholder","filled","focused","selection","readonly","open","current"],
    sizes: ["size-xs","size-sm","size-lg","size-icon","size-icon-xs","size-icon-sm","size-icon-lg"],
    overlay_parts: ["trigger","content"],
  };
  const elsOut = [];
  for (const [id, v] of Object.entries(elements)) {
    const baseId = id.replace(/\.(trigger|content)$/, "");
    const meta = catalogById[id] || catalogById[baseId] || {};
    const kind =
      id.startsWith("shell.") ? "shell"
      : /\.(trigger)$/.test(id) ? "overlay-trigger"
      : /\.(content)$/.test(id) ? "overlay-content"
      : meta.kind === "overlay" ? "stage"
      : meta.kind || "control";
    const triggerMatch = id.match(/^([^.]+\.[^.]+)\.(trigger|content)$/);
    const entry = {
      automation_id: id,
      component: meta.component || id.split(".")[0],
      variant: meta.variant ?? null,
      state: meta.state ?? null,
      size: meta.size ?? null,
      category: meta.category || null,
      kind,
      data_slot: v.rec.slot,
      tag: v.rec.tag,
    };
    if (kind === "overlay-content") {
      entry.trigger = id.replace(/\.content$/, ".trigger");
      entry.anchor = meta.meta?.anchor || null;
    }
    if (kind === "stage" && meta.meta?.anchor) entry.anchor = meta.meta.anchor;
    entry.placements = v.placements;
    entry.box = v.rec.box;
    if (v.rec.text) entry.text = v.rec.text;
    if (Object.keys(v.rec.parts).length) entry.parts = v.rec.parts;
    if (v.rec.force_state) entry.forced_state = v.rec.force_state;
    elsOut.push(entry);
  }
  elsOut.sort((a, b) => a.automation_id.localeCompare(b.automation_id));

  const contract = {
    "$schema": "rust-ui.shadcn-reference.contract/0.1",
    reference_version: "0.1",
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
      baseline_proxy_formula:
        "baseline_proxy_y = text_rect.y (relative to element) + fontBoundingBoxAscent from canvas measureText with the element's computed font shorthand",
      geometry: "placements/box measured in light theme; capture asserts dark geometry identical",
    },
    elements: elsOut,
  };
  fs.writeFileSync(path.join(outDir, "contract.json"), JSON.stringify(contract, null, 2));

  // ---------- reference.json ----------
  const reference = {
    "$schema": "rust-ui.shadcn-reference.reference/0.1",
    reference_version: "0.1",
    authority: {
      site: "https://ui.shadcn.com/",
      urls_reviewed: [
        "/docs/theming",
        "/docs/components",
        "/docs/changelog/2025-12-shadcn-create",
        ...["button","badge","separator","card","alert","skeleton","kbd","label","input","textarea","checkbox","radio-group","switch","slider","select","field","tabs","breadcrumb","pagination","toggle","toggle-group","button-group","dialog","alert-dialog","popover","tooltip","dropdown-menu","typography"].map((c) => `/docs/components/base/${c}`),
      ],
      upstream_repo: "github.com/shadcn-ui/ui",
      upstream_commit: "295a1f114a138f23b5dfee0e0c6812394dfeb90c",
      upstream_files: [
        "apps/v4/registry/styles/style-nova.css",
        "packages/shadcn/src/tailwind.css",
        "apps/v4/registry/themes.ts (theme neutral)",
        "apps/v4/registry/bases/base/ui/*.tsx",
        "apps/v4/registry/bases/base/examples/*-example.tsx",
      ],
    },
    authority_review_date: "2026-10-05",
    style: {
      library: "base",
      style: "nova",
      base_color: "neutral",
      icon_library: "lucide",
      icon_package: "lucide-static@1.21.0",
      radius: "default",
      menu_accent: "subtle",
      menu_color: "default",
    },
    install_transforms: {
      note: "emulated upstream install-time transformers for this config (packages/registry/src/utils/transformers)",
      menu: "transform-menu.ts - menuColor=default removes cn-menu-target AND cn-menu-translucent (they are placeholders; only *-translucent inline the translucent classes, inverted* swaps cn-menu-target->'dark')",
      rtl: "transform-rtl.ts - rtl=false -> skipped (no direction/logical-side transform, cn-rtl-flip markers absent)",
      font: "transform-font.ts - cn-font-heading kept; --font-heading: var(--font-sans) present in gallery.css -> resolves to sans (no distinct heading font; Geist->Segoe deviation)",
      icons: "transform-icons.ts - iconLibrary=lucide -> no-op (source paths already lucide)",
      cleanup: "transform-cleanup.ts - strips transient cn-* markers only (cn-rtl-flip, cn-logical-sides); recipe cn-* classes retained",
      tw_prefix: "none configured -> no-op",
    },
    font: {
      canonical: "Segoe UI Variable (deviation from upstream Geist - deliberate rust-ui substitution)",
      css_stack: `"Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif`,
      mono_stack: `"Cascadia Mono", Consolas, monospace`,
      font_optical_sizing: "auto (DirectWrite must apply opsz by size)",
      resolved_platform_families: {
        sans: [...resolved.sans],
        mono: [...resolved.mono],
      },
      proof: "CDP CSS.getPlatformFontsForNode on every text-bearing contract element",
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

  // manifest of screenshot hashes for check.js determinism comparison
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

  if (geoDivergences) { console.error(`${geoDivergences} light/dark geometry divergences`); process.exitCode = 1; }
  console.log(`capture: ${elsOut.length} elements, ${Object.keys(hashes).length} screenshots, fonts sans=[${[...resolved.sans]}] mono=[${[...resolved.mono]}]`);
  console.log(`chromium ${chromiumVersion}`);
  if (process.exitCode) console.error("CAPTURE FAILED - see errors above");
  else console.log("capture ok");
  await browser.close();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
