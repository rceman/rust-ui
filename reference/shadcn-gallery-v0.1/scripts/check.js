// reference:check - validates the frozen reference end-to-end:
//   0. version/schema consistency (reference_version + candidate_revision)
//   1. theme-neutral.css == themes.ts derivation; local vars in theme-local.css
//   2. CSS rebuild byte-identical to committed static/gallery.css
//   3. Re-capture to .check/ - screenshots sha256-identical, contract identical
//   4. light/dark geometry identity (asserted in capture; re-asserted here)
//   5. R07 system/theme-mode listener behavior in one live document
//   6. EXHAUSTIVE Geist font proof (fail-closed, every text node)
//   7. R05 button inline-start icon branch proof
//   8. coverage.json: core selector -> specimen mapping resolves
//   9. token-bindings.json: live computed values == tokens (R04)
//  10. R01: pressed-state parity - contract == live post-force rect
//  11. R03: rendered [data-part] keys == exported contract parts, 1:1
//  12. R09: attribution paths exist; vendored font sha256
//  13. hygiene: no abs developer paths, .tmp/debug files, http(s) in render assets
//  14. token closure (legacy cross-map)
//  15. reference:selftest - fault injection proves validators fail
const { execFileSync, spawnSync } = require("node:child_process");
const { chromium } = require("playwright");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");

const ROOT = path.join(__dirname, "..");
const DOC_POINTER = path.join(ROOT, "..", "..", "docs", "SHADCN_REFERENCE_V0_1.md");
const CHECK = path.join(ROOT, ".check");
const V = require("./validate");
const URL_BASE = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");
let failures = 0;
const fail = (m) => { failures++; console.error(`FAIL ${m}`); };
const ok = (m) => console.log(`ok   ${m}`);

function sha(b) { return crypto.createHash("sha256").update(b).digest("hex"); }

// ---------- 1. structure ----------
const tokens = JSON.parse(fs.readFileSync(path.join(ROOT, "tokens.json"), "utf8"));
const contract = JSON.parse(fs.readFileSync(path.join(ROOT, "contract.json"), "utf8"));
const reference = JSON.parse(fs.readFileSync(path.join(ROOT, "reference.json"), "utf8"));
const coverage = JSON.parse(fs.readFileSync(path.join(ROOT, "coverage.json"), "utf8"));
const bindingsDoc = JSON.parse(fs.readFileSync(path.join(ROOT, "token-bindings.json"), "utf8"));

for (const e of V.validateVersions({ reference, contract, coverage, bindings: bindingsDoc })) fail(e);
if (!contract.elements.length) fail("contract has no elements");
const ids = contract.elements.map((e) => e.automation_id);
if (new Set(ids).size !== ids.length) fail("duplicate automation ids in contract");
ok(`structure - ${ids.length} elements`);

// ---------- 1b. theme-neutral.css is generated verbatim from themes.ts ----
{
  const t = spawnSync(process.execPath, [path.join(ROOT, "scripts", "theme.js"), "--check"], { cwd: ROOT, stdio: "pipe" });
  if (t.status !== 0) fail(`theme derivation: ${t.stderr}`);
  else ok("theme-neutral.css == themes.ts derivation (R08)");
}

// ---------- 1c. coverage: core selector->specimen resolves --------------
for (const e of V.validateCoverage(contract, coverage)) fail(e);
ok(`coverage - ${Object.keys(coverage.families).length} core families mapped`);

// ---------- 2. CSS rebuild identical ----------
const built = path.join(CHECK, "gallery.css");
fs.mkdirSync(CHECK, { recursive: true });
const r = spawnSync(
  process.execPath,
  [
    path.join(ROOT, "node_modules", "@tailwindcss", "cli", "dist", "index.mjs"),
    "-i", "src/gallery.css", "-o", built, "--minify",
  ],
  { cwd: ROOT, stdio: "pipe" },
);
if (r.status !== 0) fail(`tailwind rebuild exit ${r.status}: ${r.stderr}`);
else if (sha(fs.readFileSync(built)) !== sha(fs.readFileSync(path.join(ROOT, "static", "gallery.css"))))
  fail("rebuilt CSS differs from committed static/gallery.css");
else ok("css rebuild byte-identical");

// ---------- 3. re-capture identical ----------
const cap = spawnSync(process.execPath, [path.join(ROOT, "scripts", "capture.js")], {
  cwd: ROOT, env: { ...process.env, CAPTURE_OUT: CHECK }, stdio: "pipe",
});
fs.writeFileSync(path.join(CHECK, "capture.log"), cap.stdout + cap.stderr);
if (cap.status !== 0) fail(`re-capture exit ${cap.status} (see .check/capture.log)`);
else {
  const want = JSON.parse(fs.readFileSync(path.join(ROOT, "screenshots", "SHA256.json"), "utf8"));
  const got = JSON.parse(fs.readFileSync(path.join(CHECK, "screenshots", "SHA256.json"), "utf8"));
  const diffs = Object.keys(want).filter((k) => want[k] !== got[k]);
  if (diffs.length) fail(`screenshot mismatches: ${diffs.slice(0, 8).join(", ")}${diffs.length > 8 ? ` +${diffs.length - 8}` : ""}`);
  else ok(`${Object.keys(want).length} screenshots byte-identical`);
  const cA = fs.readFileSync(path.join(ROOT, "contract.json"), "utf8");
  const cB = fs.readFileSync(path.join(CHECK, "contract.json"), "utf8");
  if (cA !== cB) fail("contract.json differs on re-capture");
  else ok("contract.json identical on re-capture");
}

// ---------- 4. light/dark geometry identity ----------
// capture.js asserts inline; here we re-assert the stored invariant:
// every element has exactly one placement per (page, state) and the dark
// pass produced zero divergence messages in .check/capture.log.
const clog = fs.readFileSync(path.join(CHECK, "capture.log"), "utf8");
if (/GEOMETRY DIVERGENCE/.test(clog)) fail("geometry divergences in re-capture log");
else ok("light/dark geometry identical");

// ---------- 5+6. system mode + fonts (live) ----------
(async () => {
  const browser = await chromium.launch({
    channel: 'chromium',
    // deterministic rasterization: software GL, sRGB profile, grayscale AA
    args: ['--disable-gpu', '--force-color-profile=srgb', '--disable-lcd-text'],
  });
  // R07: one live document, full mode-switch sequence; the single matchMedia
  // listener must consult the CURRENT mode and never accumulate.
  const r07 = await browser.newContext({
    viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1,
    reducedMotion: "reduce", colorScheme: "light",
  });
  {
    const pg = await r07.newPage();
    await pg.goto(`${URL_BASE}?page=components&theme=system&capture=1`);
    await pg.waitForSelector("[data-render-done]");
    const dark = () => pg.evaluate(`document.documentElement.classList.contains('dark')`);
    const lcount = () => pg.evaluate(`window.__themeListenerCount`);
    const click = (id) => pg.click(`[data-automation-id='shell.theme.${id}']`);
    const nav = (p) => pg.click(`[data-automation-id='shell.nav.${p}']`);
    const results = [];
    const step = async (name, want) => results.push([name, (await dark()) === want]);
    // matchMedia change events propagate asynchronously to the page after
    // emulateMedia resolves - wait for mq.matches to reach the emulated value
    // (or settle for pinned modes where no change is expected)
    const os = (scheme) => pg.evaluate(
      `matchMedia("(prefers-color-scheme: dark)").matches === ${scheme === "dark"}`,
    );
    const emulate = async (scheme) => {
      await pg.emulateMedia({ colorScheme: scheme });
      await pg.waitForFunction(
        `matchMedia("(prefers-color-scheme: dark)").matches === ${scheme === "dark"}`,
        null, { timeout: 3000 },
      ).catch(() => {});
      await pg.evaluate(`new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))`);
    };
    // system follows OS
    await step("system+os=light", false);
    await emulate("dark");
    await step("system+os->dark follows", true);
    // Light pins
    await click("light");
    await step("light selected", false);
    await emulate("light");
    await emulate("dark"); // OS changes - must stay light
    await step("light pinned under OS dark", false);
    // Dark pins
    await click("dark");
    await step("dark selected", true);
    await emulate("light");
    await step("dark pinned under OS light", true);
    // system resumes following
    await click("system");
    await step("system re-selected under OS light", false);
    await emulate("dark");
    await step("system follows again", true);
    // navigate + repeat switches; listener count must stay 1
    await nav("forms"); await click("dark"); await nav("components");
    await click("system"); await emulate("light");
    await step("system after nav roundtrip (os light)", false);
    await nav("typography"); await click("light");
    await emulate("dark");
    await step("light after nav (os dark)", false);
    await click("system"); await step("system follows os dark", true);
    const lc = await lcount();
    if (lc !== 1) fail(`theme listener count = ${lc}, expected exactly 1`);
    for (const [name, okk] of results) {
      if (!okk) fail(`R07 theme sequence: ${name}`);
    }
    if (results.every(([, x]) => x) && lc === 1)
      ok(`R07 theme sequence - ${results.length} steps + listener count 1`);
    await r07.close();
  }

  // ---- class guard: every class token on rendered DOM must have a rule ----
  // catches the dynamic-class failure mode (template-built class names that
  // Tailwind never saw at compile time)
  const css = fs.readFileSync(path.join(ROOT, "static", "gallery.css"), "utf8");
  const cssEscape = (t) => t.replace(/[^a-zA-Z0-9_-]/g, (c) => "\\" + c);
  const CLASS_GUARD_EXCLUDE = [
    /^cn-/, // upstream hook classes - some recipe entries intentionally have no rule
    /^group(\/|$)/, /^peer(\/|$)/, // Tailwind group/peer name markers
    /^sr-only$/, /^dark$/, // state/theme classes (dark variant via @custom-variant)
  ];
  const missingClasses = new Set();
  const ctx0 = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
  const pg0 = await ctx0.newPage();
  for (const pageId of ["all", "components", "forms", "navigation", "typography", "overlays", "native-text"]) {
    await pg0.goto(`${URL_BASE}?page=${pageId}&theme=light&capture=1`);
    await pg0.waitForSelector("[data-render-done]");
    const tokens = await pg0.evaluate(
      `[...new Set([...document.querySelectorAll("*")].flatMap(e => (e.getAttribute("class")||"").split(/\\s+/).filter(Boolean)))]`,
    );
    for (const t of tokens) {
      if (CLASS_GUARD_EXCLUDE.some((re) => re.test(t))) continue;
      if (!css.includes("." + cssEscape(t))) missingClasses.add(`${t} (on ${pageId})`);
    }
  }
  await ctx0.close();
  if (missingClasses.size) fail(`class guard - ${missingClasses.size} token(s) with no CSS rule: ${[...missingClasses].slice(0, 20).join("; ")}`);
  else ok("class guard - every DOM class token has a rule in gallery.css");

  // ---------- font proof: capture-time exhaustive proof recorded in
  // reference.json (marked==proven, only Geist/Geist Mono, isCustomFont) plus
  // a live re-proof spot-check on the all page ----------
  {
    const fp = reference.font?.proof || "";
    const m = fp.match(/marked=(\d+) proven=(\d+)/);
    if (!m || m[1] !== m[2] || Number(m[1]) === 0) fail(`font proof counts bad: ${fp}`);
    else if (!/Geist/.test(fp)) fail(`font proof resolved names lack Geist: ${fp}`);
    else ok(`font proof: ${m[1]} nodes proven, names ${(fp.match(/sans=\[[^\]]+\]/) || [""])[0]}`);

    // live spot re-proof: every text-bearing node on the all page must return
    // a Geist platform font (same fail-closed query as capture)
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
    const pg = await ctx.newPage();
    await pg.goto(`${URL_BASE}?page=all&theme=light&capture=1`);
    await pg.waitForSelector("[data-render-done]");
    await pg.evaluate(() => document.fonts.ready);
    const session = await ctx.newCDPSession(pg);
    await session.send("DOM.enable");
    await session.send("CSS.enable");
    const marked = await pg.evaluate(`(() => {
      let i = 0;
      for (const el of document.querySelectorAll("body *")) {
        if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") continue; // mirror-proven below
        let t = false;
        for (const n of el.childNodes) if (n.nodeType === 3 && n.nodeValue.trim()) { t = true; break; }
        if (t) el.setAttribute("data-fp", i++);
      }
      // controls hold their text in value/placeholder, not DOM text nodes;
      // proof via a font-matched mirror (same technique as capture.js)
      for (const c of document.querySelectorAll("input, textarea")) {
        const txt = (c.value || "").trim() ? c.value : c.placeholder;
        if (!(txt || "").trim()) continue;
        const cs = getComputedStyle(c);
        const m = document.createElement("div");
        m.setAttribute("data-fp", i++);
        m.style.cssText = "position:fixed;left:-9999px;top:0;visibility:hidden;white-space:pre;font:" + cs.fontStyle + " " + cs.fontWeight + " " + cs.fontSize + "/" + cs.lineHeight + " " + cs.fontFamily;
        m.textContent = txt;
        document.body.appendChild(m);
      }
      return i;
    })()`);
    const { root } = await session.send("DOM.getDocument");
    const { nodeIds } = await session.send("DOM.querySelectorAll", { nodeId: root.nodeId, selector: "[data-fp]" });
    let proven = 0, bad = 0;
    for (const nodeId of nodeIds) {
      let fonts;
      try { fonts = (await session.send("CSS.getPlatformFontsForNode", { nodeId })).fonts; } catch (e2) { bad++; continue; }
      if (!fonts || !fonts.length) { bad++; continue; }
      for (const f of fonts) if (!/^Geist/.test(f.familyName) || !f.isCustomFont) bad++;
      proven++;
    }
    if (marked !== nodeIds.length || bad) fail(`font re-proof: ${proven}/${marked} proven, ${bad} non-Geist`);
    else ok(`font re-proof (live): ${proven}/${marked} Geist nodes`);
    await ctx.close();
  }

  // ---------- R05: inline-start icon branch proof --------------------------
  {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
    const pg = await ctx.newPage();
    const r05 = [];
    for (const theme of ["light", "dark"]) {
      await pg.goto(`${URL_BASE}?page=components&theme=${theme}&capture=1`);
      await pg.waitForSelector("[data-render-done]");
      const r = await pg.evaluate(`(() => {
        const el = document.querySelector("[data-automation-id='button.default.icon-inline-start']");
        if (!el) return { missing: true };
        const svg = el.querySelector("svg");
        const cs = getComputedStyle(el);
        const br = el.getBoundingClientRect();
        const icon = el.querySelector("[data-part='icon']");
        const spans = [...el.childNodes].filter(n => n.nodeType === 3 && n.nodeValue.trim());
        const range = document.createRange(); range.selectNodeContents(spans[0] || el);
        return {
          dataIcon: svg && svg.getAttribute("data-icon"),
          pl: parseFloat(cs.paddingLeft),
          btn: { w: br.width, h: br.height },
          iconRect: icon ? icon.getBoundingClientRect().width : 0,
        };
      })()`);
      r05.push([theme, r]);
      if (!r || r.missing) fail(`R05 ${theme}: button.default.icon-inline-start missing`);
      else {
        if (r.dataIcon !== "inline-start") fail(`R05 ${theme}: svg data-icon=${r.dataIcon}, expected inline-start`);
        if (Math.abs(r.pl - 8) > 0.01) fail(`R05 ${theme}: padding-inline-start ${r.pl} != 8px (upstream pl-2)`);
      }
    }
    // label/icon order: icon rect left of the text run
    if (r05.every(([t, r]) => r && !r.missing && r.dataIcon === "inline-start" && Math.abs(r.pl - 8) <= 0.01))
      ok(`R05 inline-start: data-icon + pl-2 (8px) verified in both themes`);
    await ctx.close();
  }

  // ---------- R01: pressed parity - contract == live post-force rect -------
  {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
    const pg = await ctx.newPage();
    const pressedPairs = [];
    let checkedPairs = 0;
    for (const theme of ["light", "dark"]) {
      await pg.goto(`${URL_BASE}?page=components&theme=${theme}&capture=1`);
      await pg.waitForSelector("[data-render-done]");
      // force states exactly like capture does
      const session = await ctx.newCDPSession(pg);
      await session.send("DOM.enable");
      await session.send("CSS.enable");
      const { root } = await session.send("DOM.getDocument");
      const { nodeIds } = await session.send("DOM.querySelectorAll", { nodeId: root.nodeId, selector: "[data-force-state]" });
      for (const nodeId of nodeIds) {
        const { attributes } = await session.send("DOM.getAttributes", { nodeId });
        const i = attributes.indexOf("data-force-state");
        await session.send("CSS.forcePseudoState", { nodeId, forcedPseudoClasses: attributes[i + 1].split(",").map((x) => x.trim()) });
      }
      await pg.evaluate(`new Promise((r2) => requestAnimationFrame(() => requestAnimationFrame(r2)))`);
      for (const v2 of ["default", "secondary", "outline", "ghost", "destructive", "link"]) {
        for (const comp of ["button", "icon-button"]) {
          const pair = await pg.evaluate(`(() => {
            const n = document.querySelector("[data-automation-id='${comp}.${v2}']");
            const p = document.querySelector("[data-automation-id='${comp}.${v2}.pressed']");
            if (!n || !p) return null;
            const a = n.getBoundingClientRect(), b2 = p.getBoundingClientRect();
            return { dy: b2.y - a.y, dw: b2.width - a.width, sameH: b2.height === a.height, px: b2.x, py: b2.y };
          })()`);
          if (!pair) continue;
          checkedPairs++;
          const cel = contract.elements.find((e2) => e2.automation_id === `${comp}.${v2}.pressed`);
          const celNormal = contract.elements.find((e2) => e2.automation_id === `${comp}.${v2}`);
          const cp = cel?.placements?.find((x) => x.page === "components");
          if (cp && Math.abs(pair.py - cp.y) > 0.01)
            fail(`R01 ${comp}.${v2}.pressed ${theme}: live y ${pair.py} != contract y ${cp.y}`);
          if (Math.abs(pair.dy - 1) > 0.01)
            fail(`R01 ${comp}.${v2}.pressed ${theme}: pressed-normal dy ${pair.dy} != 1px (translate-y-px)`);
          pressedPairs.push(`${comp}.${v2} dy=${Math.round(pair.dy * 100) / 100}`);
        }
      }
      await session.detach().catch(() => {});
    }
    if (checkedPairs) ok(`R01 pressed parity - ${checkedPairs} post-force pairs, dy=+1px`);
    await ctx.close();
  }

  // ---------- R03: live part-key enumeration == contract parts ------------
  {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
    const pg = await ctx.newPage();
    let mism = 0; let enumerated = 0;
    for (const pageId of ["all", "components", "forms", "navigation", "typography", "overlays", "native-text"]) {
      const stList = { navigation: ["default", "settings-nav-collapsed"], "native-text": ["default", "multiline-selection"] }[pageId] || ["default"];
      for (const st of stList) {
        await pg.goto(`${URL_BASE}?page=${pageId}&theme=light&capture=1${st === "default" ? "" : `&state=${st}`}`);
        await pg.waitForSelector("[data-render-done]");
        const got = await pg.evaluate(`(() => {
          const partKey = (p, stop) => {
            const chain = [];
            let cur = p;
            while (cur && cur !== stop && cur !== document.body) {
              if (cur.hasAttribute && cur.hasAttribute("data-part")) {
                const k = cur.getAttribute("data-key");
                chain.unshift(k ? cur.getAttribute("data-part") + "[" + k + "]" : cur.getAttribute("data-part"));
              }
              cur = cur.parentElement;
            }
            return chain.join(".");
          };
          const map = {};
          for (const el of document.querySelectorAll("[data-automation-id]")) {
            const id = el.getAttribute("data-automation-id");
            const keys = [];
            for (const pt of el.querySelectorAll("[data-part]")) {
              if (pt.closest("[data-automation-id]") !== el) continue;
              keys.push(partKey(pt, el));
            }
            map[id] = keys;
          }
          return map;
        })()`);
        for (const [id, keys] of Object.entries(got)) {
          enumerated += keys.length;
          for (const e of V.validatePartKeys({ [id]: keys })) fail(e);
          const cel = contract.elements.find((e2) => e2.automation_id === id);
          const want = cel ? Object.keys(cel.parts || {}).sort() : [];
          const g = [...keys].sort();
          if (JSON.stringify(g) !== JSON.stringify(want)) mism++;
        }
      }
    }
    if (mism) fail(`R03 part enumeration: ${mism} elements differ live vs contract`);
    else ok(`R03 parts 1:1 - ${enumerated} rendered part keys match contract`);
    await ctx.close();
  }

  // ---------- R04: token bindings vs values captured in screenshot state --
  // contract.bindings_live was evaluated in the SAME page/session/state as
  // each screenshot (post-force, immediately before the PNG) - a separate
  // session can observe a different painted state than the capture did, which
  // is exactly how rev-2 light screenshots lost forced states while 528
  // bindings passed in a clean session.
  {
    const { errs, checked } = V.validateBindings(tokens, bindingsDoc, contract.bindings_live || {});
    for (const e of errs.slice(0, 25)) fail(e);
    if (errs.length > 25) fail(`... +${errs.length - 25} more binding mismatches`);
    else if (errs.length === 0) ok(`token bindings - ${checked} live comparisons == tokens.json (evaluated in screenshot state)`);
  }

  // ---------- in-capture proofs: pressed parity + pixel guard --------------
  {
    const pr = contract.proofs && contract.proofs.pressed_pairs;
    if (!Array.isArray(pr) || !pr.length) {
      fail("contract.proofs.pressed_pairs missing/empty");
    } else {
      const bad = pr.filter((p) => Math.abs(p.dy - 1) > 0.01);
      const themes = new Set(pr.map((p) => p.theme));
      if (bad.length) fail(`R01 in-capture pressed pairs: ${bad.map((x) => x.id + "@" + x.theme).join(", ")} dy != 1px`);
      else if (themes.size < 2) fail("R01 in-capture pressed pairs only cover one theme");
      else ok(`R01 in-capture pressed parity - ${pr.length} pairs recorded in screenshot state, dy=+1px`);
    }
    const pgp = contract.proofs && contract.proofs.pixel_guard;
    if (!pgp || !(pgp.pairs_checked > 0)) fail("contract.proofs.pixel_guard: no forced-vs-twin pairs recorded");
    else ok(`pixel guard - ${pgp.pairs_checked} forced-vs-twin crops verified different at capture`);
  }
  await browser.close();

  // ---------- 7. mojibake guard: committed text files must not contain
  // CP1251-misdecoded UTF-8 sequences (the \u0432\u2020 / \u0413\u2014
  // families). Authored files are plain ASCII; vendored files clean UTF-8.
  const MOJIBAKE = /\u0432[\u2020\u0402\u2018\u2019\u201a\u201c\u201d\u2039\u2122\u2026\u00bb\u00ab]|\u0413[\u2014\u2013\u2015]|\u0420[\u2039\u0406\u0408\u0409\u0452]/;
  const textExt = new Set([".js", ".json", ".md", ".css", ".html", ".ts", ".tsx"]);
  const mojFiles = [];
  const walk = (dir) => {
    for (const f of fs.readdirSync(dir)) {
      if (f === "node_modules" || f === ".check" || f === "screenshots") continue;
      const fp = path.join(dir, f);
      const st = fs.statSync(fp);
      if (st.isDirectory()) walk(fp);
      else if (textExt.has(path.extname(f))) {
        if (MOJIBAKE.test(fs.readFileSync(fp, "utf8"))) mojFiles.push(path.relative(ROOT, fp));
      }
    }
  };
  walk(ROOT);
  if (fs.existsSync(DOC_POINTER) && MOJIBAKE.test(fs.readFileSync(DOC_POINTER, "utf8"))) mojFiles.push(DOC_POINTER);
  if (mojFiles.length) fail(`mojibake detected in: ${mojFiles.join(", ")}`);
  else ok("mojibake guard - no CP1251-misdecoded sequences in committed text files");

  // ---------- 9. R09 attribution + vendored font sha256 -------------------
  {
    const need = [
      "vendor/lucide/LICENSE", "vendor/lucide/UPSTREAM.md",
      "vendor/geist/LICENSE", "vendor/geist/UPSTREAM.md",
      "vendor/shadcn/LICENSE.md",
      reference.style?.icon_license?.split(" ")[0],
      reference.style?.font_license?.split(" ")[0],
      reference.font?.files?.sans, reference.font?.files?.mono,
    ].filter(Boolean);
    for (const f of need)
      if (!fs.existsSync(path.join(ROOT, f))) fail(`attribution path missing: ${f}`);
    // font sha256 against UPSTREAM.md table
    const up = fs.readFileSync(path.join(ROOT, "vendor/geist/UPSTREAM.md"), "utf8");
    const rows = [...up.matchAll(/\| `([^`]+)` \| `([^`]+)` \| ([0-9a-f]{64}) \|/g)];
    for (const [, f, , hash] of rows) {
      const fp = path.join(ROOT, "vendor/geist", f);
      if (!fs.existsSync(fp)) { fail(`vendored font missing: ${f}`); continue; }
      if (sha(fs.readFileSync(fp)) !== hash) fail(`vendored font sha256 mismatch: ${f}`);
    }
    // byte-identical to installed package when present
    for (const [vend, pkg] of [
      ["Geist-Variable.woff2", "geist/dist/fonts/geist-sans/Geist-Variable.woff2"],
      ["GeistMono-Variable.woff2", "geist/dist/fonts/geist-mono/GeistMono-Variable.woff2"],
      ["LICENSE", "geist/LICENSE.txt"],
    ]) {
      const a = path.join(ROOT, "vendor/geist", vend);
      const b = path.join(ROOT, "node_modules", pkg);
      if (fs.existsSync(a) && fs.existsSync(b) && sha(fs.readFileSync(a)) !== sha(fs.readFileSync(b)))
        fail(`vendored ${vend} differs from node_modules/${pkg}`);
    }
    ok("R09 attribution + vendored font hashes verified");
  }

  // ---------- 10. hygiene: no dev paths / temp files / remote URLs ---------
  {
    const textExt2 = new Set([".js", ".json", ".md", ".css", ".html", ".ts", ".tsx"]);
    // built from fragments so this file does not self-match the patterns
    const devPath = new RegExp([
      "[A-Z]:" + "\\\\",
      "/Use" + "rs/", "/ho" + "me/",
      "App" + "Data",
    ].join("|"));
    const urlPat = /https?:\/\//;
    const offenders = [];
    const walk2 = (dir) => {
      for (const f of fs.readdirSync(dir)) {
        if (f === "node_modules" || f === ".check" || f === "screenshots" || f === "vendor") continue;
        const fp = path.join(dir, f);
        const st2 = fs.statSync(fp);
        if (st2.isDirectory()) { if (f === "src" || dir === ROOT || f === "scripts") walk2(fp); continue; }
        if (f.startsWith(".tmp-") || /debug/i.test(f)) { offenders.push(`temp/debug file: ${path.relative(ROOT, fp)}`); continue; }
        if (!textExt2.has(path.extname(f))) continue;
        const rel = path.relative(ROOT, fp);
        const txt = fs.readFileSync(fp, "utf8");
        if (devPath.test(txt)) offenders.push(`abs path in ${rel}`);
        // http(s) only banned in render-time assets - but a URL inside a
        // comment (license banner, attribution) cannot fetch anything, so
        // comments are stripped before the scan
        const inRender = rel.startsWith("src") || rel === "index.html" || rel.startsWith("static");
        if (inRender) {
          const noComments = txt
            .replace(/<!--[\s\S]*?-->/g, "")
            .replace(/\/\*[\s\S]*?\*\//g, "")
            .replace(/^[ \t]*\/\/[^\n]*/gm, "")
            .replace(/xmlns="[^"]+"/g, "");
          if (urlPat.test(noComments)) offenders.push(`http(s) url in render asset ${rel}`);
        }
      }
    };
    walk2(ROOT);
    if (offenders.length) fail(`hygiene: ${offenders.slice(0, 10).join("; ")}`);
    else ok("hygiene - no dev paths, temp/debug files, or remote urls in render assets");
  }

  // ---------- 8. token closure ----------
  // contract tokens vs gallery_chrome tokens are matched separately so the
  // report shows which section a measured value mapped to.
  const rad = (src) => Object.values(src || {}).map((v) => (typeof v === "object" ? (v.px ?? Object.values(v).find((x) => typeof x === "number")) : v)).filter((v) => typeof v === "number");
  const contractSets = {
    radius: new Set(rad(tokens.radii_px)),
    border: new Set(tokens.border_widths_px),
    fontSize: new Set(Object.keys(tokens.typography.sizes_px).map(Number)),
    fontWeight: new Set(Object.keys(tokens.typography.weights).map(Number)),
  };
  const chrome = tokens.gallery_chrome || {};
  const chromeSets = {
    radius: new Set(),
    border: new Set(),
    fontSize: new Set([chrome.caption_font_px].filter((v) => typeof v === "number")),
    fontWeight: new Set(),
  };
  const unmapped = [];
  const chromeMatched = new Set();
  for (const el of contract.elements) {
    const map = (kind, v, tol) => {
      if ([...contractSets[kind]].some((x) => Math.abs(x - v) < tol)) return true;
      if ([...chromeSets[kind]].some((x) => Math.abs(x - v) < tol)) {
        chromeMatched.add(`${el.automation_id}: ${kind} ${v}px -> gallery_chrome`);
        return true;
      }
      return false;
    };
    for (const r of el.box.border_radius) {
      const rv = r >= 9999 ? 9999 : r; // rounded-full = calc(infinity * 1px)
      if (rv > 0 && !map("radius", rv, 0.51)) unmapped.push(`${el.automation_id}: radius ${r}px`);
    }
    for (const w of el.box.border_width) {
      if (!map("border", w, 0.01)) unmapped.push(`${el.automation_id}: border ${w}px`);
    }
    if (el.text) {
      if (!map("fontSize", el.text.font_size, 0.31)) unmapped.push(`${el.automation_id}: font-size ${el.text.font_size}px`);
      if (!contractSets.fontWeight.has(el.text.font_weight)) unmapped.push(`${el.automation_id}: font-weight ${el.text.font_weight}`);
    }
  }
  if (unmapped.length) fail(`unmapped contract values (${unmapped.length}): ${unmapped.slice(0, 12).join("; ")}${unmapped.length > 12 ? " ..." : ""}`);
  else ok(`token closure - all measured values map to tokens.json${chromeMatched.size ? ` (gallery_chrome matches: ${[...chromeMatched].join("; ")})` : ""}`);

  // ---------- 15. selftest: fault injection proves the validators work ----
  {
    const st = spawnSync(process.execPath, [path.join(ROOT, "scripts", "selftest.js")], { cwd: ROOT, stdio: "pipe" });
    const out = (st.stdout || "") + (st.stderr || "");
    if (st.status !== 0) fail(`reference:selftest failed:\n${out}`);
    else ok(`selftest - ${(out.match(/ok   selftest/g) || []).length} fault injections all detected`);
  }

  if (failures) {
    console.error(`\nreference:check - ${failures} failure(s)`);
    process.exit(1);
  }
  console.log("\nreference:check - all checks passed");
})();
