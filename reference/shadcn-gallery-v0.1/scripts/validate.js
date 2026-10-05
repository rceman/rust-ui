// Pure validators shared by scripts/check.js (real artifacts) and
// scripts/selftest.js (in-memory fault injection). Each returns an array of
// failure strings; empty = pass.

// ---------- version / schema ----------
const CANDIDATE_REVISION = 3;
function validateVersions({ reference, contract, coverage, bindings, tokens }) {
  const errs = [];
  const schemas = {
    reference: "rust-ui.shadcn-reference.reference/0.1",
    contract: "rust-ui.shadcn-reference.contract/0.2",
    coverage: "rust-ui.shadcn-reference.coverage/0.1",
    bindings: "rust-ui.shadcn-reference.token-bindings/0.1",
  };
  for (const [name, want] of Object.entries(schemas)) {
    const doc = { reference, contract, coverage, bindings }[name];
    if (!doc) { errs.push(`${name}: missing`); continue; }
    if (doc["$schema"] !== want) errs.push(`${name}: $schema=${doc["$schema"]} != ${want}`);
    if (doc.reference_version !== "0.1") errs.push(`${name}: reference_version=${doc.reference_version} != 0.1`);
    if (doc.candidate_revision !== CANDIDATE_REVISION) errs.push(`${name}: candidate_revision=${doc.candidate_revision} != ${CANDIDATE_REVISION}`);
  }
  if (tokens) for (const e of validateTokens(tokens)) errs.push(e);
  return errs;
}

// ---------- tokens.json: schema + every leaf value domain-checked ---------
// fail-closed: an unparseable/non-finite/out-of-domain token must never be
// silently comparable.
function validateTokens(tokens) {
  const errs = [];
  if (!tokens || typeof tokens !== "object") return ["tokens: missing/not an object"];
  if (tokens["$schema"] !== "rust-ui.shadcn-reference.tokens/0.1")
    errs.push(`tokens: $schema=${tokens["$schema"]} != rust-ui.shadcn-reference.tokens/0.1`);
  if (tokens.reference_version !== "0.1")
    errs.push(`tokens: reference_version=${tokens.reference_version} != 0.1`);
  if (tokens.candidate_revision !== CANDIDATE_REVISION)
    errs.push(`tokens: candidate_revision=${tokens.candidate_revision} != ${CANDIDATE_REVISION}`);
  const walk = (node, path) => {
    if (node === null || node === undefined) return;
    if (typeof node === "number") {
      if (!Number.isFinite(node)) errs.push(`tokens: ${path} not finite (${node})`);
      return;
    }
    if (typeof node === "object") {
      // domain check leaf value slots
      for (const k of ["alpha"]) {
        if (typeof node[k] === "number" && (node[k] < 0 || node[k] > 1))
          errs.push(`tokens: ${path}.${k} alpha ${node[k]} outside [0,1]`);
      }
      // srgb leaves must be #rrggbb or a parseable CSS color; `color` keys
      // outside shadow layers are descriptive strings - layer colors are
      // domain-checked in the shadows walk below
      if (typeof node.srgb === "string" && !/^#[0-9a-f]{6}$/i.test(node.srgb) && !computedToSrgba(node.srgb))
        errs.push(`tokens: ${path}.srgb unparseable color '${node.srgb}'`);
      for (const k of ["px", "x", "y", "blur", "spread", "top", "bottom", "width", "height", "line_height"]) {
        if (k in node && node[k] !== null && typeof node[k] !== "number" && typeof node[k] !== "string")
          errs.push(`tokens: ${path}.${k} unexpected type ${typeof node[k]}`);
        if (typeof node[k] === "number" && !Number.isFinite(node[k]))
          errs.push(`tokens: ${path}.${k} not finite`);
      }
      for (const [k, v] of Object.entries(node)) {
        if (typeof v === "object" && v !== null) walk(v, path ? `${path}.${k}` : k);
      }
      return;
    }
  };
  for (const [k, v] of Object.entries(tokens)) {
    if (k.startsWith("$") || typeof v === "string") continue;
    if (typeof v === "object" && v !== null) walk(v, k);
  }
  // structured shadows: used entries carry validated layer data
  for (const [name, s] of Object.entries(tokens.shadows || {})) {
    if (s.status === "used") {
      if (!Array.isArray(s.layers) || !s.layers.length)
        errs.push(`tokens: shadows.${name} used but has no layers`);
      else for (const [i, l] of s.layers.entries()) {
        for (const k of ["x", "y", "blur", "spread"]) {
          if (typeof l[k] !== "number" || !Number.isFinite(l[k]))
            errs.push(`tokens: shadows.${name}.layers[${i}].${k} missing/not finite`);
        }
        if (typeof l.alpha !== "number" || l.alpha < 0 || l.alpha > 1)
          errs.push(`tokens: shadows.${name}.layers[${i}].alpha invalid`);
        if (!/^#[0-9a-f]{6}$/i.test(l.color || ""))
          errs.push(`tokens: shadows.${name}.layers[${i}].color '${l.color}' not #rrggbb`);
      }
      if (!Array.isArray(s.used_by) || !s.used_by.length)
        errs.push(`tokens: shadows.${name} used but used_by empty`);
    } else if (s.status !== "unused") {
      errs.push(`tokens: shadows.${name} status '${s.status}' != used/unused`);
    }
  }
  return errs;
}

// ---------- coverage: three-way (contract / coverage.json / authority) ----
// bounded upstream state-variant vocabulary for selector anchoring (C04)
const STATE_VOCAB = new Set([
  "hover", "active", "focus-visible", "disabled",
  "aria-invalid", "aria-expanded", "aria-checked", "aria-selected",
  "data-checked", "data-unchecked", "data-active", "data-disabled",
  "data-invalid", "data-highlighted", "data-popup-open", "data-open",
  "data-placeholder", "placeholder",
]);

// parse one @apply variant segment down to a bounded state prefix
function normStateSegment(seg) {
  let v = seg.trim();
  if (!v || v.startsWith("[")) return null;
  // strip non-state wrappers iteratively (dark:, not-, group-, peer-, has-)
  let changed = true;
  while (changed) {
    changed = false;
    for (const p of ["dark:", "not-", "group-", "peer-", "has-"]) {
      if (v.startsWith(p) && !STATE_VOCAB.has(v)) { v = v.slice(p.length); changed = true; }
    }
  }
  let m = v.match(/^(aria-[a-z-]+)/);
  if (m) return STATE_VOCAB.has(m[1]) ? m[1] : null;
  m = v.match(/data-\[([a-z-]+)/);
  if (m) return STATE_VOCAB.has("data-" + m[1]) ? "data-" + m[1] : null;
  m = v.match(/data-([a-z-]+)/);
  if (m) return STATE_VOCAB.has("data-" + m[1]) ? "data-" + m[1] : null;
  m = v.match(/^([a-z-]+)/);
  if (m && STATE_VOCAB.has(m[1])) return m[1];
  return null;
}

// derive the state-variant prefixes used in the hook classes of style-nova.css
function deriveNovaPrefixes(styleCss, hookSources) {
  const hookRes = (hookSources || []).map((s) => new RegExp(s));
  const found = new Set();
  const re = /\.(cn-[a-z0-9-]+)\s*\{([\s\S]*?)\}/g;
  let m;
  while ((m = re.exec(styleCss))) {
    if (!hookRes.some((r) => r.test(m[1]))) continue;
    const am = m[2].match(/@apply\s+([^;]+)/);
    if (!am) continue;
    for (const tok of am[1].split(/\s+/)) {
      const segs = tok.split(":");
      if (segs.length < 2) continue;
      for (const seg of segs.slice(0, -1)) {
        const n = normStateSegment(seg);
        if (n) found.add(n);
      }
    }
  }
  return found;
}

// contract.captures[<page>/<state>] part lookup: a part exists if present in
// the given capture (when the mapping declares capture_state) or visible in
// at least one capture otherwise.
function partPresent(el, part, captureState) {
  const inRec = (rec) => {
    if (!rec || !rec.parts) return false;
    let visible = false;
    for (const [k, v] of Object.entries(rec.parts)) {
      if ((k === part || k.startsWith(part)) && !(v && v.visible === false)) visible = true;
    }
    return visible;
  };
  const caps = el.captures || {};
  if (captureState) {
    for (const [k, rec] of Object.entries(caps)) {
      if (k.endsWith("/" + captureState) || (captureState === "default" && k.endsWith("/default"))) return inRec(rec);
    }
    return false;
  }
  return Object.values(caps).some(inRec);
}

function validateCoverage(contract, coverage, authority) {
  const errs = [];
  const ids = new Set(contract.elements.map((e) => e.automation_id));
  const elOf = (id) => contract.elements.find((e) => e.automation_id === id);

  // (a) required core families = contract core-tier families + gallery-shell
  const required = new Set(["gallery-shell"]);
  for (const e of contract.elements) {
    if (e.tier === "core" && e.family) {
      const fam = e.family === "settings-nav-item" ? "settings-nav" : e.family;
      required.add(fam);
    }
  }
  const covFams = new Set(Object.keys(coverage.families || {}));
  for (const f of required) if (!covFams.has(f)) errs.push(`coverage: missing family '${f}'`);
  for (const f of covFams) if (!required.has(f)) errs.push(`coverage: unknown family '${f}'`);

  // (b) per family: rule keys equal the authority's exactly + mappings resolve
  const auth = (authority && authority.families) || authority || {};
  for (const fam of covFams) {
    const cf = coverage.families[fam];
    const af = auth[fam];
    if (!af) { errs.push(`coverage ${fam}: no authority entry`); continue; }
    const covRules = Object.keys(cf.states || {}).sort();
    const authRules = Object.keys(af.states || {}).sort();
    const missing = authRules.filter((r) => !covRules.includes(r));
    const unknown = covRules.filter((r) => !authRules.includes(r));
    if (missing.length) errs.push(`coverage ${fam}: missing rule(s) ${missing.join(", ")}`);
    if (unknown.length) errs.push(`coverage ${fam}: unknown rule(s) ${unknown.join(", ")}`);
    for (const [sel, m] of Object.entries(cf.states || {})) {
      const spec = m.specimen;
      const specs = m.specimens || (spec ? [spec] : []);
      const eq = m.equivalent_to;
      const na = m["n/a"];
      if (!specs.length && !eq && !na) {
        errs.push(`coverage ${fam} '${sel}': no specimen/equivalent_to/n-a`);
        continue;
      }
      for (const id of specs) {
        if (!ids.has(id)) errs.push(`coverage ${fam} '${sel}': specimen '${id}' missing from contract`);
      }
      if (eq && !ids.has(eq)) errs.push(`coverage ${fam} '${sel}': equivalent_to '${eq}' missing from contract`);
      for (const part of m.parts || []) {
        const target = m.parts_on || spec || specs[0];
        const tel = target && elOf(target);
        if (tel && !partPresent(tel, part, m.capture_state))
          errs.push(`coverage ${fam} '${sel}': part '${part}' absent in ${target}`);
      }
    }
  }

  // (c) upstream anchoring: every state prefix derived from the family's
  // hook classes in vendored style-nova.css must be claimed by some rule
  const styleCss = validateCoverage._styleCss || "";
  for (const fam of covFams) {
    const af = auth[fam];
    if (!af || !af.hooks || !af.hooks.length) continue;
    const derived = deriveNovaPrefixes(styleCss, af.hooks);
    const claimed = new Set();
    for (const m of Object.values((coverage.families[fam] || {}).states || {}))
      for (const s of m.selectors || []) claimed.add(s);
    for (const p of derived) {
      if (!STATE_VOCAB.has(p)) continue;
      if (!claimed.has(p))
        errs.push(`coverage ${fam}: upstream prefix '${p}' derived from hook rules but unclaimed by any rule`);
    }
  }
  return errs;
}

// ---------- token path resolution ----------
// resolves colors./translucent./typography./control_/etc paths; {theme} is
// replaced by the bound theme before lookup.
function resolveBindingToken(tokens, tokenPath, theme) {
  const p = tokenPath.replaceAll("{theme}", theme);
  if (p === "special.transparent.srgb") return "#000000";
  if (p === "special.transparent.alpha") return 0;
  if (p === "special.transparent") return "#000000";
  const m = p.match(/^translucent\.([^.]+)\.(light|dark)\.(.+)$/);
  if (m) {
    const row = (tokens.translucent || []).find((t) => t.name === m[1]);
    if (!row) return undefined;
    return row[m[2]]?.[m[3]];
  }
  // longest-match-first: token keys can contain dots (e.g. "button.default"
  // under control_heights_px); at each level prefer the longest candidate
  const segs = p.split(".");
  let cur = tokens;
  let i = 0;
  while (i < segs.length && cur !== undefined && cur !== null) {
    if (Array.isArray(cur)) { cur = cur[Number(segs[i++])]; continue; }
    let key = null;
    for (let j = segs.length; j > i; j--) {
      const cand = segs.slice(i, j).join(".");
      if (typeof cur === "object" && cand in cur) { key = cand; break; }
    }
    if (key === null) return undefined;
    cur = cur[key];
    i += key.split(".").length;
  }
  if (i < segs.length) return undefined; // trailing path inside a scalar
  if (cur && typeof cur === "object") {
    if ("srgb" in cur) return cur.srgb;
    if ("px" in cur) return cur.px;
    if ("alpha" in cur) return cur.alpha;
  }
  return cur;
}

// ---------- color conversion ----------
function hexToRgb(hex) {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
}
function oklchToSrgb(L, C, H) {
  const hr = (H * Math.PI) / 180;
  const a = C * Math.cos(hr), b2 = C * Math.sin(hr);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b2) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541729 * b2) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b2) ** 3;
  const cl = (v) => Math.min(1, Math.max(0, v));
  const R = cl(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s);
  const G = cl(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s);
  const Bv = cl(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s);
  const g = (v) => (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
  return [R, G, Bv].map((v) => Math.round(g(v) * 255));
}
function oklabToSrgb(L, a, b2) {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b2) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541729 * b2) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b2) ** 3;
  const cl = (v) => Math.min(1, Math.max(0, v));
  const R = cl(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s);
  const G = cl(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s);
  const Bv = cl(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s);
  const g = (v) => (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
  return [R, G, Bv].map((v) => Math.round(g(v) * 255));
}
// computed CSS color string -> [r,g,b,alpha]
function computedToSrgba(str) {
  if (!str) return null;
  str = str.trim();
  let m = str.match(/^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/);
  if (m) return [+m[1], +m[2], +m[3], m[4] ? (m[4].endsWith("%") ? +m[4].slice(0, -1) / 100 : +m[4]) : 1];
  m = str.match(/^oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+|none)(?:\s*\/\s*([\d.]+%?))?\s*\)$/);
  if (m) {
    const [r, g, b2] = oklchToSrgb(+m[1], +m[2], m[3] === "none" ? 0 : +m[3]);
    return [r, g, b2, m[4] ? (m[4].endsWith("%") ? +m[4].slice(0, -1) / 100 : +m[4]) : 1];
  }
  m = str.match(/^oklab\(\s*([\d.]+)\s+([-\d.]+)\s+([-\d.]+|none)(?:\s*\/\s*([\d.]+%?))?\s*\)$/);
  if (m) {
    const [r, g, b2] = oklabToSrgb(+m[1], +m[2], m[3] === "none" ? 0 : +m[3]);
    return [r, g, b2, m[4] ? (m[4].endsWith("%") ? +m[4].slice(0, -1) / 100 : +m[4]) : 1];
  }
  m = str.match(/^color\(srgb\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)(?:\s*\/\s*([\d.]+))?\s*\)$/);
  if (m) return [Math.round(+m[1] * 255), Math.round(+m[2] * 255), Math.round(+m[3] * 255), m[4] ? +m[4] : 1];
  return null;
}

// ---------- bindings validation ----------
// live: map `${automation_id}|${part||""}|${theme}|${capture_state||""}` ->
//   { <css_property>: computedString }
// Compares each binding's live computed value against the token.
// parse a live box-shadow string into {ring, effects} layer lists.
// a ring layer is "0 0 0 <N>px" (zero offset/blur, spread only - Tailwind
// ring); everything else is an effect layer.
function parseShadowLayers(str) {
  if (str === "none") return { ring: [], effects: [] };
  const segs = [];
  const re = /(rgba?\([^)]*\)|oklch\([^)]*\)|oklab\([^)]*\)|color\([^)]*\)|#[0-9a-fA-F]+|transparent|currentcolor)?\s*(-?[\d.]+px)\s+(-?[\d.]+px)\s+(-?[\d.]+px)\s+(-?[\d.]+)px/g;
  let sm;
  let consumed = "";
  while ((sm = re.exec(String(str)))) {
    consumed += sm[0];
    segs.push({ color: sm[1] || "currentcolor", x: parseFloat(sm[2]), y: parseFloat(sm[3]), blur: parseFloat(sm[4]), spread: parseFloat(sm[5]) });
  }
  if (!segs.length && String(str).trim() !== "none") return null;
  const ring = [], effects = [];
  for (const s of segs) {
    if (s.x === 0 && s.y === 0 && s.blur === 0) ring.push(s);
    else effects.push(s);
  }
  return { ring, effects };
}

function validateBindings(tokens, bindingsDoc, live) {
  const errs = [];
  let checked = 0;
  const rows = bindingsDoc.bindings || bindingsDoc;
  // expected-token domain checks (fail-closed BEFORE comparison)
  const tokenNumOk = (t, allowNegative) =>
    typeof t === "number" ? Number.isFinite(t) && (allowNegative || t >= 0)
      : (() => { const v = parseFloat(String(t)); return Number.isFinite(v) && (allowNegative || v >= 0); })();
  for (const bd of rows) {
    const themes = bd.theme === "both" ? ["light", "dark"] : [bd.theme];
    for (const theme of themes) {
      const token = resolveBindingToken(tokens, bd.token_path, theme);
      if (token === undefined) { errs.push(`${bd.automation_id}: token path '${bd.token_path}' unresolvable`); continue; }
      const key = `${bd.automation_id}|${bd.part || ""}|${theme}|${bd.capture_state || ""}`;
      const livev = live[key] ? live[key][bd.css_property] : undefined;
      if (livev === undefined || livev === null) {
        errs.push(`${bd.automation_id}${bd.part ? " part=" + bd.part : ""} ${theme}${bd.capture_state ? "/" + bd.capture_state : ""}: no live value for ${bd.css_property}`);
        continue;
      }
      const isColorProp = /color|background|border-color|box-shadow-(color|alpha)|outline/.test(bd.css_property);
      // expected token must be valid for its domain before any comparison
      if (bd.css_property === "box-shadow-layers") {
        if (!token || !Array.isArray(token.layers)) { errs.push(`${bd.automation_id}: token '${bd.token_path}' not a structured shadow entry`); continue; }
      } else if (bd.css_property === "box-shadow-alpha" || bd.token_path.endsWith(".alpha")) {
        if (!tokenNumOk(token, false) || token > 1) { errs.push(`${bd.automation_id}: token '${bd.token_path}' alpha ${token} invalid`); continue; }
      } else if (isColorProp || bd.css_property === "box-shadow-color") {
        const tokOk = typeof token === "string" && (/^#[0-9a-f]{6}$/i.test(token) || computedToSrgba(token));
        if (!tokOk) { errs.push(`${bd.automation_id}: token '${bd.token_path}' unparseable color '${token}'`); continue; }
      } else if (bd.css_property === "translate-x" || bd.css_property === "translate-y") {
        if (!tokenNumOk(token, true)) { errs.push(`${bd.automation_id}: token '${bd.token_path}' ${bd.css_property} '${token}' not finite`); continue; }
      } else {
        if (!tokenNumOk(token, false)) { errs.push(`${bd.automation_id}: token '${bd.token_path}' value '${token}' not a finite non-negative number`); continue; }
      }
      checked++;
      if (bd.css_property === "translate-y" || bd.css_property === "translate-x") {
        // live value is either the resolved component ("14px" - the in-page
        // resolver evaluates calc/percent) or the raw shorthand ("0px 1px")
        const s = String(livev).trim();
        const mm = s.match(/matrix\([^)]*,\s*([-\d.]+),\s*([-\d.]+)\)\s*$/);
        const px = s.match(/-?[\d.]+px/g) || [];
        const tv = s === "none" ? 0
          : mm ? +(bd.css_property === "translate-y" ? mm[2] : mm[1])
          : px.length === 1 ? parseFloat(px[0])
          : px.length >= 2 ? parseFloat(px[bd.css_property === "translate-y" ? 1 : 0])
          : NaN;
        if (Number.isNaN(tv))
          errs.push(`${bd.automation_id}: ${bd.css_property} unparseable live '${livev}'`);
        else if (Math.abs(tv - token) > 0.01)
          errs.push(`${bd.automation_id}: ${bd.css_property} ${livev} != ${token}`);
      } else if (bd.css_property === "box-shadow-layers") {
        // compare ALL effect layers of the live computed box-shadow, order
        // preserved, against the token layers; ring layers excluded
        const liveLayers = parseShadowLayers(livev);
        if (!liveLayers) { errs.push(`${bd.automation_id}: unparseable box-shadow '${livev}'`); continue; }
        const want = token.layers;
        if (liveLayers.effects.length !== want.length) {
          errs.push(`${bd.automation_id}: box-shadow-layers count ${liveLayers.effects.length} != ${want.length}`);
          continue;
        }
        for (let i = 0; i < want.length; i++) {
          const l = liveLayers.effects[i], w = want[i];
          for (const k of ["x", "y", "blur", "spread"]) {
            if (Math.abs(l[k] - w[k]) > 0.01)
              errs.push(`${bd.automation_id}: box-shadow-layers[${i}].${k} ${l[k]} != ${w[k]}`);
          }
          const cv = computedToSrgba(l.color);
          const wantRgb = /^#[0-9a-f]{6}$/i.test(w.color) ? hexToRgb(w.color) : null;
          if (!cv) errs.push(`${bd.automation_id}: box-shadow-layers[${i}] unparseable color '${l.color}'`);
          else {
            if (wantRgb && cv.slice(0, 3).some((v, ci) => Math.abs(v - wantRgb[ci]) > 1))
              errs.push(`${bd.automation_id}: box-shadow-layers[${i}].color ${l.color} != ${w.color}`);
            if (Math.abs(cv[3] - w.alpha) > 0.005)
              errs.push(`${bd.automation_id}: box-shadow-layers[${i}].alpha ${cv[3]} != ${w.alpha}`);
          }
        }
      } else if (bd.css_property.startsWith("box-shadow")) {
        // pick the shadow segment with the largest spread (the ring shadow)
        const segs = [];
        const re = /(rgba?\([^)]*\)|oklch\([^)]*\)|oklab\([^)]*\)|color\([^)]*\)|#[0-9a-fA-F]+|transparent|currentcolor)?\s*(-?[\d.]+px)\s+(-?[\d.]+px)\s+(-?[\d.]+px)\s+(-?[\d.]+)px/g;
        let sm;
        while ((sm = re.exec(String(livev)))) segs.push({ color: sm[1] || "currentcolor", spread: parseFloat(sm[5]) });
        const ring = segs.sort((a, b) => Math.abs(b.spread) - Math.abs(a.spread))[0];
        if (bd.css_property === "box-shadow-width") {
          if (!ring || Math.abs(ring.spread - token) > 0.01)
            errs.push(`${bd.automation_id}${bd.part ? " " + bd.part : ""}: ring width ${livev} != ${token}`);
        } else if (!ring) {
          errs.push(`${bd.automation_id}: unparseable shadow '${livev}'`);
        } else {
          const cv = computedToSrgba(ring.color);
          if (!cv) { errs.push(`${bd.automation_id}: unparseable shadow color '${ring.color}'`); continue; }
          if (bd.css_property === "box-shadow-color") {
            const want = String(token).startsWith("#") ? hexToRgb(token) : computedToSrgba(String(token))?.slice(0, 3);
            if (!want || cv.slice(0, 3).some((v, i) => Math.abs(v - want[i]) > 1))
              errs.push(`${bd.automation_id}: ring color ${ring.color} != ${token}`);
          } else if (Math.abs(cv[3] - token) > 0.005) {
            errs.push(`${bd.automation_id}: ring alpha ${cv[3]} != ${token}`);
          }
        }
      } else if (isColorProp) {
        const cv = computedToSrgba(livev);
        if (!cv) { errs.push(`${bd.automation_id}: unparseable color '${livev}'`); continue; }
        if (bd.css_property.endsWith("alpha") || bd.token_path.endsWith(".alpha")) {
          const want = typeof token === "number" ? token : (token.alpha ?? 1);
          if (Math.abs(cv[3] - want) > 0.005)
            errs.push(`${bd.automation_id} ${theme}: alpha ${cv[3]} != ${want}`);
        } else {
          const want = String(token).startsWith("#") ? hexToRgb(token) : computedToSrgba(String(token))?.slice(0, 3);
          if (!want || cv.slice(0, 3).some((v, i) => Math.abs(v - want[i]) > 1))
            errs.push(`${bd.automation_id} ${theme}: ${bd.css_property} ${livev} != ${token}`);
        }
      } else {
        let v = parseFloat(String(livev).replace("px", ""));
        const want = typeof token === "number" ? token : parseFloat(String(token));
        // rounded-full = calc(infinity * 1px) computes to ~3.3e7px; clamp to
        // the 9999 sentinel the token table uses
        if (want === 9999 && v >= 9999) v = 9999;
        if (Number.isNaN(v) || Math.abs(v - want) > 0.01)
          errs.push(`${bd.automation_id}${bd.part ? " part=" + bd.part : ""} ${theme}: ${bd.css_property} ${livev} != ${token}`);
      }
    }
  }
  return { errs, checked };
}

// ---------- C01 capture authority ----------------------------------------
// live = check.js's independent in-browser measurement:
//   { "<page>/<state>": { ids: [automation_id...], elements:
//     { <id>: { rect: {x,y,width,height}, parts: {key: {x,y,width,height}|{visible:false,reason}} } } } }
// Per page/state: rendered id set must equal the set of ids that have
// captures[<page>/<state>]; and for every id rendered in MORE THAN ONE
// capture, rect + part rects/visibility must equal the contract within 0.01.
function validateCaptureAuthority(contract, live) {
  const errs = [];
  let multiCount = 0;
  const byId = {};
  for (const el of contract.elements) byId[el.automation_id] = el;
  const multiIds = new Set(
    contract.elements.filter((e) => Object.keys(e.captures || {}).length > 1).map((e) => e.automation_id));
  multiCount = multiIds.size;
  for (const [capKey, p] of Object.entries(live || {})) {
    const rendered = new Set(p.ids || []);
    const want = new Set(
      contract.elements.filter((e) => e.captures && e.captures[capKey]).map((e) => e.automation_id));
    for (const id of want)
      if (!rendered.has(id)) errs.push(`capture-authority ${capKey}: contract id '${id}' not rendered live`);
    for (const id of rendered)
      if (!want.has(id)) errs.push(`capture-authority ${capKey}: rendered id '${id}' has no contract capture`);
    for (const id of multiIds) {
      const cap = byId[id].captures[capKey];
      if (!cap) continue;
      const lel = (p.elements || {})[id];
      if (!lel) { errs.push(`capture-authority ${capKey}: multi-capture id '${id}' has no live record`); continue; }
      for (const k of ["x", "y", "width", "height"]) {
        if (Math.abs((lel.rect ? lel.rect[k] : NaN) - cap.rect[k]) > 0.01)
          errs.push(`capture-authority ${id} ${capKey}: rect.${k} ${lel.rect && lel.rect[k]} != ${cap.rect[k]}`);
      }
      // paint: every box field - numeric arrays within 0.01, scalars exact
      for (const k of Object.keys({ ...(cap.box || {}), ...(lel.box || {}) })) {
        const av = cap.box && cap.box[k], bv = lel.box && lel.box[k];
        if (Array.isArray(av) || Array.isArray(bv)) {
          if (!Array.isArray(av) || !Array.isArray(bv) || av.length !== bv.length || av.some((v, i) => Math.abs(v - bv[i]) > 0.01))
            errs.push(`capture-authority ${id} ${capKey}: box.${k} ${JSON.stringify(bv)} != ${JSON.stringify(av)}`);
        } else if (av !== bv)
          errs.push(`capture-authority ${id} ${capKey}: box.${k} ${JSON.stringify(bv)} != ${JSON.stringify(av)}`);
      }
      // interaction state (focus/selection) - exact
      if (JSON.stringify(lel.interaction || null) !== JSON.stringify(cap.interaction || null))
        errs.push(`capture-authority ${id} ${capKey}: interaction ${JSON.stringify(lel.interaction)} != ${JSON.stringify(cap.interaction)}`);
      const lp = lel.parts || {};
      for (const [pk, cv] of Object.entries(cap.parts || {})) {
        const lv = lp[pk];
        if (!lv) { errs.push(`capture-authority ${id} ${capKey}: part '${pk}' missing live`); continue; }
        const cHidden = cv.visible === false;
        const lHidden = lv.visible === false;
        if (cHidden !== lHidden) {
          errs.push(`capture-authority ${id} ${capKey}: part '${pk}' visibility ${lHidden ? "hidden" : "visible"} live != ${cHidden ? "hidden" : "visible"} contract`);
          continue;
        }
        if (cHidden) continue;
        // every part field: rect fields within 0.01, paint/metadata exact
        for (const k of Object.keys({ ...cv, ...lv })) {
          const av = cv[k], bv = lv[k];
          if (["x", "y", "width", "height", "label_gap"].includes(k)) {
            if (av === undefined && bv === undefined) continue;
            if (av === undefined || bv === undefined || Math.abs(bv - av) > 0.01)
              errs.push(`capture-authority ${id} ${capKey}: part '${pk}'.${k} ${bv} != ${av}`);
          } else if (av !== bv)
            errs.push(`capture-authority ${id} ${capKey}: part '${pk}'.${k} ${JSON.stringify(bv)} != ${JSON.stringify(av)}`);
        }
      }
    }
  }
  return { errs, multiCount };
}

// ---------- C01-d same-state consistency (formal V04 invariant) -----------
// Same automation id + same capture state on different pages => identical
// recorded appearance: element width/height, box, interaction, every part's
// visibility + paint fields + element-relative rect, text_runs
// content/font/element-relative rects. Exceptions only via the explicit
// map below (each entry a reason); non-exempt mismatches fail.
const SAME_STATE_EXCEPTIONS = {
  "shell.nav.all": "active nav tab follows the current page (gallery chrome)",
  "shell.nav.components": "active nav tab follows the current page (gallery chrome)",
  "shell.nav.forms": "active nav tab follows the current page (gallery chrome)",
  "shell.nav.navigation": "active nav tab follows the current page (gallery chrome)",
  "shell.nav.typography": "active nav tab follows the current page (gallery chrome)",
  "shell.nav.overlays": "active nav tab follows the current page (gallery chrome)",
  "shell.nav.native-text": "active nav tab follows the current page (gallery chrome)",
};

const _scalarFieldsEq = (a, b, k) => {
  const av = a ? a[k] : undefined, bv = b ? b[k] : undefined;
  if (Array.isArray(av) || Array.isArray(bv))
    return Array.isArray(av) && Array.isArray(bv) && av.length === bv.length && av.every((v, i) => Math.abs(v - bv[i]) <= 0.01);
  if (typeof av === "number" || typeof bv === "number")
    return typeof av === "number" && typeof bv === "number" && Math.abs(av - bv) <= 0.01;
  return av === bv;
};
const _rectFieldsEq = (a, b, fields) => fields.every((k) => {
  const av = a ? a[k] : undefined, bv = b ? b[k] : undefined;
  if (av === undefined && bv === undefined) return true;
  return av !== undefined && bv !== undefined && Math.abs(av - bv) <= 0.01;
});

function validateSameStateConsistency(contract, { exceptions = SAME_STATE_EXCEPTIONS } = {}) {
  const errs = [];
  const stateOf = (capKey) => capKey.split("/").slice(1).join("/");
  const checked = { ids: 0, pairs: 0 };
  for (const el of contract.elements || []) {
    const groups = {};
    for (const capKey of Object.keys(el.captures || {}))
      (groups[stateOf(capKey)] = groups[stateOf(capKey)] || []).push(capKey);
    const report = (first, other, m) => {
      if (exceptions[el.automation_id]) return;
      errs.push(`same-state ${el.automation_id} ${first} vs ${other}: ${m}`);
    };
    for (const keys of Object.values(groups)) {
      if (keys.length < 2) continue;
      checked.ids++;
      const [first, ...rest] = keys;
      for (const other of rest) {
        checked.pairs++;
        const a = el.captures[first], b = el.captures[other];
        // element width/height (x/y are page-dependent by design)
        if (!_rectFieldsEq(a.rect, b.rect, ["width", "height"]))
          report(first, other, `rect size ${JSON.stringify(b.rect)} != ${JSON.stringify(a.rect)}`);
        // box
        for (const k of Object.keys({ ...(a.box || {}), ...(b.box || {}) }))
          if (!_scalarFieldsEq(a.box, b.box, k))
            report(first, other, `box.${k} ${JSON.stringify(b.box && b.box[k])} != ${JSON.stringify(a.box && a.box[k])}`);
        // interaction
        if (JSON.stringify(a.interaction || null) !== JSON.stringify(b.interaction || null))
          report(first, other, `interaction ${JSON.stringify(b.interaction)} != ${JSON.stringify(a.interaction)}`);
        // parts: visibility + paint fields + element-relative rect
        for (const pk of Object.keys({ ...(a.parts || {}), ...(b.parts || {}) })) {
          const pa = a.parts && a.parts[pk], pb = b.parts && b.parts[pk];
          if (!pa || !pb) { report(first, other, `part '${pk}' missing in ${pa ? other : first}`); continue; }
          if ((pa.visible === false) !== (pb.visible === false)) { report(first, other, `part '${pk}' visibility differs`); continue; }
          if (pa.visible === false) continue;
          for (const k of Object.keys({ ...pa, ...pb }))
            if (!_scalarFieldsEq(pa, pb, k)) report(first, other, `part '${pk}'.${k} ${JSON.stringify(pb[k])} != ${JSON.stringify(pa[k])}`);
        }
        // text_runs: content + font fields + element-relative rects
        const ra = a.text_runs || [], rb = b.text_runs || [];
        if (ra.length !== rb.length) report(first, other, `text_runs count ${rb.length} != ${ra.length}`);
        for (let i = 0; i < Math.min(ra.length, rb.length); i++) {
          const ta = ra[i], tb = rb[i];
          for (const k of ["key", "part", "content", "source", "visible", "font_family", "font_size", "font_weight", "line_height", "color", "font_ascent", "font_descent"]) {
            const av = ta[k], bv = tb[k];
            if (av === undefined && bv === undefined) continue;
            if (typeof av === "number" || typeof bv === "number") {
              if (av === undefined || bv === undefined || Math.abs(av - bv) > 0.01) report(first, other, `text_runs[${i}].${k} ${bv} != ${av}`);
            } else if (av !== bv) report(first, other, `text_runs[${i}].${k} ${JSON.stringify(bv)} != ${JSON.stringify(av)}`);
          }
          if (!_rectFieldsEq(ta.rect, tb.rect, ["x", "y", "width", "height"]))
            report(first, other, `text_runs[${i}].rect differs`);
          for (let li = 0; li < Math.max((ta.lines || []).length, (tb.lines || []).length); li++) {
            const la = (ta.lines || [])[li], lb = (tb.lines || [])[li];
            if (!la || !lb) { report(first, other, `text_runs[${i}].lines[${li}] missing`); continue; }
            if (!_rectFieldsEq(la.rect, lb.rect, ["x", "y", "width", "height"]) || !_rectFieldsEq(la, lb, ["baseline_y"]))
              report(first, other, `text_runs[${i}].lines[${li}] differs`);
          }
        }
      }
    }
  }
  return { errs, checked };
}

// ---------- C02 text ownership -------------------------------------------
// liveTextInventory = { "<page>/<state>": [{owner, part, content}] } — the
// same inventory IN_PAGE_MEASURE records in contract.text_inventory, but
// re-collected by check.js in an independent session. Every owned text must
// match a text_run in that owner's capture: same content, same part key,
// positive geometry, font fields present.
function validateTextOwnership(contract, liveTextInventory) {
  const errs = [];
  let checked = 0;
  const byId = {};
  for (const el of contract.elements) byId[el.automation_id] = el;
  for (const [capKey, items] of Object.entries(liveTextInventory || {})) {
    const usedByOwner = {};
    for (const it of items) {
      const cap = byId[it.owner] && byId[it.owner].captures && byId[it.owner].captures[capKey];
      if (!cap) { errs.push(`text-ownership ${capKey}: owner '${it.owner}' has no capture`); continue; }
      const used = usedByOwner[it.owner] = usedByOwner[it.owner] || new Set();
      const runs = cap.text_runs || [];
      let found = -1;
      for (let i = 0; i < runs.length; i++) {
        const r = runs[i];
        if (used.has(i)) continue;
        if (r.content === it.content && (it.part ? r.part === it.part : true)) { found = i; break; }
      }
      if (found < 0) {
        errs.push(`text-ownership ${capKey}: '${it.content.slice(0, 40)}' owner=${it.owner} part=${it.part || "-"} has no contract text_run`);
        continue;
      }
      used.add(found);
      const r = runs[found];
      checked++;
      if (!(r.rect && r.rect.width > 0 && r.rect.height > 0))
        errs.push(`text-ownership ${capKey}: run '${it.content.slice(0, 30)}' has non-positive rect`);
      for (const f of ["font_family", "font_size", "font_weight", "line_height"]) {
        if (r[f] === undefined || r[f] === null)
          errs.push(`text-ownership ${capKey}: run '${it.content.slice(0, 30)}' missing ${f}`);
      }
    }
  }
  return { errs, checked };
}

// ---------- part-key enumeration (R03) ----------
// renderedKeysPerElement: { automation_id: [key, ...] } - keys computed live
// from the DOM; each element's list must be dedup-free. check.js also asserts
// the rendered list equals the exported contract parts one-to-one.
function validatePartKeys(renderedKeysPerElement) {
  const errs = [];
  for (const [id, keys] of Object.entries(renderedKeysPerElement)) {
    const seen = new Set();
    for (const k of keys) {
      if (seen.has(k)) errs.push(`${id}: duplicate part key '${k}'`);
      seen.add(k);
    }
  }
  return errs;
}

// ---------- font proof ----------
function validateFontProof(report) {
  const errs = [];
  if (!report.marked || report.proven !== report.marked)
    errs.push(`font proof: ${report.proven}/${report.marked} text nodes proven`);
  if (report.failures && report.failures.length)
    errs.push(`font proof failures: ${report.failures.slice(0, 5).join("; ")}`);
  for (const k of ["sans", "mono"]) {
    const names = report.resolved?.[k] || [];
    for (const n of names)
      if (n !== "Geist" && n !== "Geist Mono") errs.push(`non-Geist resolved: ${n}`);
  }
  return errs;
}

module.exports = {
  validateVersions, validateCoverage, validateBindings, validateTokens,
  validateCaptureAuthority, validateSameStateConsistency, validateTextOwnership,
  validatePartKeys, validateFontProof,
  computedToSrgba, resolveBindingToken, deriveNovaPrefixes,
  parseShadowLayers, CANDIDATE_REVISION, STATE_VOCAB, SAME_STATE_EXCEPTIONS,
};
