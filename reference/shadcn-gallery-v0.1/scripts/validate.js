// Pure validators shared by scripts/check.js (real artifacts) and
// scripts/selftest.js (in-memory fault injection). Each returns an array of
// failure strings; empty = pass.

// ---------- version / schema ----------
function validateVersions({ reference, contract, coverage, bindings }) {
  const errs = [];
  const schemas = {
    reference: "rust-ui.shadcn-reference.reference/0.1",
    contract: "rust-ui.shadcn-reference.contract/0.1",
    coverage: "rust-ui.shadcn-reference.coverage/0.1",
    bindings: "rust-ui.shadcn-reference.token-bindings/0.1",
  };
  for (const [name, want] of Object.entries(schemas)) {
    const doc = { reference, contract, coverage, bindings }[name];
    if (!doc) { errs.push(`${name}: missing`); continue; }
    if (doc["$schema"] !== want) errs.push(`${name}: $schema=${doc["$schema"]} != ${want}`);
    if (doc.reference_version !== "0.1") errs.push(`${name}: reference_version=${doc.reference_version} != 0.1`);
    if (doc.candidate_revision !== 2) errs.push(`${name}: candidate_revision=${doc.candidate_revision} != 2`);
  }
  return errs;
}

// ---------- coverage: every core-family selector maps to contract ----------
function validateCoverage(contract, coverage) {
  const errs = [];
  const ids = new Set(contract.elements.map((e) => e.automation_id));
  const partsOf = (id) => {
    const el = contract.elements.find((e) => e.automation_id === id);
    return el ? Object.keys(el.parts || {}) : [];
  };
  for (const [family, f] of Object.entries(coverage.families || {})) {
    for (const [sel, m] of Object.entries(f.states || {})) {
      const spec = m.specimen;
      const specs = m.specimens || (spec ? [spec] : []);
      const eq = m.equivalent_to;
      const na = m["n/a"];
      if (!specs.length && !eq && !na) {
        errs.push(`coverage ${family} '${sel}': no specimen/equivalent_to/n-a`);
        continue;
      }
      for (const id of specs) {
        if (!ids.has(id)) errs.push(`coverage ${family} '${sel}': specimen '${id}' missing from contract`);
      }
      if (eq && !ids.has(eq)) errs.push(`coverage ${family} '${sel}': equivalent_to '${eq}' missing from contract`);
      for (const part of m.parts || []) {
        const target = m.parts_on || spec || specs[0];
        if (target && !partsOf(target).some((p) => p === part || p.startsWith(part)))
          errs.push(`coverage ${family} '${sel}': part '${part}' absent in ${target}`);
      }
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
function validateBindings(tokens, bindingsDoc, live) {
  const errs = [];
  let checked = 0;
  const rows = bindingsDoc.bindings || bindingsDoc;
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
      checked++;
      const isColorProp = /color|background|border-color|box-shadow-(color|alpha)|outline/.test(bd.css_property);
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
        if (Number.isNaN(tv) || Math.abs(tv - token) > 0.01)
          errs.push(`${bd.automation_id}: ${bd.css_property} ${livev} != ${token}`);
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
  validateVersions, validateCoverage, validateBindings,
  validatePartKeys, validateFontProof,
  computedToSrgba, resolveBindingToken,
};
