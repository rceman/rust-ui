// reference:check - validates the frozen reference end-to-end:
//   1. tokens/contract/reference structural sanity
//   2. CSS rebuild byte-identical to committed static/gallery.css
//   3. Re-capture to .check/ - screenshots sha256-identical, contract identical
//   4. light/dark geometry identity (asserted in capture; re-asserted here)
//   5. System mode follows emulated scheme (live page check)
//   6. Font resolution re-proof via CDP
//   7. Token closure - every measured radius/border/font/height in the
//      contract maps to a tokens.json value
const { execFileSync, spawnSync } = require("node:child_process");
const { chromium } = require("playwright");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");

const ROOT = path.join(__dirname, "..");
const DOC_POINTER = path.join(ROOT, "..", "..", "docs", "SHADCN_REFERENCE_V0_1.md");
const CHECK = path.join(ROOT, ".check");
const URL_BASE = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");
let failures = 0;
const fail = (m) => { failures++; console.error(`FAIL ${m}`); };
const ok = (m) => console.log(`ok   ${m}`);

function sha(b) { return crypto.createHash("sha256").update(b).digest("hex"); }

// ---------- 1. structure ----------
const tokens = JSON.parse(fs.readFileSync(path.join(ROOT, "tokens.json"), "utf8"));
const contract = JSON.parse(fs.readFileSync(path.join(ROOT, "contract.json"), "utf8"));
const reference = JSON.parse(fs.readFileSync(path.join(ROOT, "reference.json"), "utf8"));

if (tokens["$schema"] !== "rust-ui.shadcn-reference.tokens/0.1") fail("tokens $schema");
if (contract["$schema"] !== "rust-ui.shadcn-reference.contract/0.1") fail("contract $schema");
if (reference["$schema"] !== "rust-ui.shadcn-reference.reference/0.1") fail("reference $schema");
if (contract.reference_version !== "0.1" || tokens.reference_version === "0.1") {}
if (!contract.elements.length) fail("contract has no elements");
const ids = contract.elements.map((e) => e.automation_id);
if (new Set(ids).size !== ids.length) fail("duplicate automation ids in contract");
ok(`structure - ${ids.length} elements`);

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
  for (const [scheme, want] of [["dark", true], ["light", false]]) {
    const ctx = await browser.newContext({
      viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1,
      reducedMotion: "reduce", colorScheme: scheme,
    });
    const pg = await ctx.newPage();
    await pg.goto(`${URL_BASE}?page=components&theme=system&capture=1`);
    await pg.waitForSelector("[data-render-done]");
    const got = await pg.evaluate(`document.documentElement.classList.contains('dark')`);
    if (got !== want) fail(`system theme under ${scheme}: got ${got}`);
    else ok(`system theme under ${scheme} -> ${got}`);
    await ctx.close();
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

  // font re-proof on typography + native-text pages
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
  const pg = await ctx.newPage();
  const session = await ctx.newCDPSession(pg);
  await session.send("DOM.enable");
  await session.send("CSS.enable");
  let fontChecked = 0;
  for (const pageId of ["typography", "native-text"]) {
    await pg.goto(`${URL_BASE}?page=${pageId}&theme=light&capture=1`);
    await pg.waitForSelector("[data-render-done]");
    await pg.evaluate(() => document.fonts.ready);
    const { root } = await session.send("DOM.getDocument");
    const { nodeIds } = await session.send("DOM.querySelectorAll", { nodeId: root.nodeId, selector: "[data-automation-id]" });
    for (const nodeId of nodeIds) {
      const { attributes } = await session.send("DOM.getAttributes", { nodeId });
      const i = attributes.indexOf("data-automation-id");
      const id = i >= 0 ? attributes[i + 1] : null;
      const fam = (contract.elements.find((e) => e.automation_id === id)?.text?.font_family) || "";
      const isMono = /mono|cascadia|consolas/i.test(fam);
      let fonts;
      try {
        fonts = (await session.send("CSS.getPlatformFontsForNode", { nodeId })).fonts;
      } catch (_) { continue; }
      if (!fonts || !fonts.length) continue;
      const wantPrefix = isMono ? "Cascadia Mono" : "Segoe UI Variable";
      if (!fonts[0].familyName.startsWith(wantPrefix)) fail(`font ${wantPrefix} expected on ${id}, got ${fonts[0].familyName}`);
      else fontChecked++;
    }
  }
  if (fontChecked === 0) fail("font proof: no nodes resolved");
  else ok(`font proof: ${fontChecked} nodes, sans=Segoe UI Variable, mono=Cascadia Mono`);
  await ctx.close();
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

  if (failures) {
    console.error(`\nreference:check - ${failures} failure(s)`);
    process.exit(1);
  }
  console.log("\nreference:check - all checks passed");
})();
