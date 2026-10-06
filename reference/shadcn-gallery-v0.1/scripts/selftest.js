// reference:selftest - injects deliberately wrong values in memory and
// proves each validator fails. The CI truth is: if a wrong artifact passes,
// the gate is broken; this makes that auditable.
// Every case is labeled "positive control" or "negative mutation"; the
// summary counts each kind separately.
const fs = require("node:fs");
const path = require("node:path");
const {
  validateVersions, validateCoverage, validateBindings, validateTokens,
  validateCaptureAuthority, validateSameStateConsistency, validateTextOwnership,
  validatePartKeys,
} = require("./validate");
const coverageAuthority = require("./coverage");

const ROOT = path.join(__dirname, "..");
const j = (f) => JSON.parse(fs.readFileSync(path.join(ROOT, f), "utf8"));
const tokens = j("tokens.json");
const contract = j("contract.json");
const reference = j("reference.json");
const coverage = j("coverage.json");
const bindingsDoc = j("token-bindings.json");
const styleCss = fs.readFileSync(path.join(ROOT, "vendor/shadcn/apps/v4/registry/styles/style-nova.css"), "utf8");

const clone = (o) => JSON.parse(JSON.stringify(o));
const covOk = (cov) => { validateCoverage._styleCss = styleCss; return validateCoverage(contract, cov, coverageAuthority).length === 0; };
const covErrs = (cov) => { validateCoverage._styleCss = styleCss; return validateCoverage(contract, cov, coverageAuthority); };

let pos = 0, neg = 0, failed = 0;
const expectPos = (name, cond) => {
  if (cond) { pos++; console.log(`ok   selftest positive control: ${name}`); }
  else { failed++; console.error(`FAIL selftest positive control: ${name}`); }
};
const expectNeg = (name, cond) => {
  if (cond) { neg++; console.log(`ok   selftest negative mutation: ${name}`); }
  else { failed++; console.error(`FAIL selftest negative mutation: ${name}`); }
};

// ---------- baseline positive controls ------------------------------------
expectPos("versions pass on committed artifacts",
  validateVersions({ reference, contract, coverage, bindings: bindingsDoc, tokens }).length === 0);
expectPos("tokens.json validates (schema + domain-walked leaves)",
  validateTokens(tokens).length === 0);
expectPos("coverage three-way passes on committed artifacts", covOk(coverage));

// ---------- versions ------------------------------------------------------
expectNeg("wrong reference_version detected", validateVersions({
  reference: { ...reference, reference_version: "0.2" },
  contract, coverage, bindings: bindingsDoc, tokens,
}).length > 0);
expectNeg("missing candidate_revision detected", validateVersions({
  reference: { ...reference, candidate_revision: undefined },
  contract, coverage, bindings: bindingsDoc, tokens,
}).length > 0);
expectNeg("tokens $schema wrong detected",
  validateTokens({ ...tokens, "$schema": "rust-ui.shadcn-reference.tokens/0.2" }).some((e) => /\$schema/.test(e)));
expectNeg("tokens reference_version wrong detected",
  validateTokens({ ...tokens, reference_version: "0.2" }).some((e) => /reference_version/.test(e)));
expectNeg("tokens candidate_revision wrong detected",
  validateTokens({ ...tokens, candidate_revision: 0 }).some((e) => /candidate_revision/.test(e)));

// ---------- C04 coverage --------------------------------------------------
{
  const cov2 = clone(coverage);
  delete cov2.families.button.states["hover:"];
  expectNeg("button 'hover:' rule deleted -> missing rule",
    covErrs(cov2).some((e) => /missing rule/.test(e) && /hover:/.test(e)));
}
{
  const cov3 = clone(coverage);
  delete cov3.families.button;
  expectNeg("button family deleted -> missing family",
    covErrs(cov3).some((e) => /missing family 'button'/.test(e)));
}
{
  // remove the `selectors` claim for one upstream-derived prefix while
  // keeping the rule itself -> derived hover: goes unclaimed
  const cov4 = clone(coverage);
  cov4.families.button.states["hover:"].selectors = [];
  expectNeg("button hover: selector claim removed -> unclaimed upstream prefix",
    covErrs(cov4).some((e) => /upstream prefix 'hover'.*unclaimed/.test(e)));
}
{
  const cov5 = clone(coverage);
  cov5.families["made-up-family"] = { hooks: [], states: {} };
  expectNeg("unknown family added -> unknown family",
    covErrs(cov5).some((e) => /unknown family 'made-up-family'/.test(e)));
}
{
  const cov6 = clone(coverage);
  cov6.families.button.states["madeup:"] = { specimen: "button.default" };
  expectNeg("unknown rule key added to button -> unknown rule",
    covErrs(cov6).some((e) => /coverage button: unknown rule\(s\) madeup:/.test(e)));
}

// ---------- R03 part keys --------------------------------------------------
expectNeg("duplicate part key detected",
  validatePartKeys({ "dialog.default": ["backdrop", "content", "content", "title"] }).length > 0);
expectPos("one-to-one part key enumeration passes on committed contract",
  validatePartKeys(Object.fromEntries(contract.elements.map((e) => [
    e.automation_id,
    Object.keys(e.captures["components/default"] ? e.captures["components/default"].parts || {} : (Object.values(e.captures)[0] || {}).parts || {}),
  ]))).length === 0);

// ---------- C03 bindings --------------------------------------------------
const only = (automation_id, prop, tp) =>
  bindingsDoc.bindings.find(
    (x) => x.automation_id === automation_id && x.css_property === prop && x.theme !== "dark" && x.token_path === tp,
  );
const bdBackdrop = only("dialog.default", "background-color", "translucent.backdrop-black-10.{theme}.alpha");
const bdHeight = only("button.default", "height", "control_heights_px.button.default");
const bdPad = only("button.default", "padding-left", "horizontal_padding_px.button.default.px");
const bdLh = only("button.default", "line-height", "typography.sizes_px.14.line_height");

expectNeg("backdrop alpha=1 detected", validateBindings(tokens, [bdBackdrop], {
  "dialog.default|backdrop|light|": { "background-color": "rgba(0, 0, 0, 1)" },
}).errs.length > 0);
expectPos("backdrop alpha=0.1 passes", validateBindings(tokens, [bdBackdrop], {
  "dialog.default|backdrop|light|": { "background-color": "rgba(0, 0, 0, 0.1)" },
}).errs.length === 0);
expectNeg("wrong control height (live 31px) detected", validateBindings(tokens, [bdHeight], {
  "button.default||light|": { height: "31px" },
}).errs.length > 0);
expectNeg("control height token = 'WRONG' detected (token domain)", validateBindings(
  { ...tokens, control_heights_px: { ...tokens.control_heights_px, "button.default": "WRONG" } },
  [bdHeight],
  { "button.default||light|": { height: "32px" } },
).errs.length > 0);
expectNeg("control height token = NaN detected", validateBindings(
  { ...tokens, control_heights_px: { ...tokens.control_heights_px, "button.default": NaN } },
  [bdHeight],
  { "button.default||light|": { height: "32px" } },
).errs.length > 0);
expectNeg("control height token = Infinity detected", validateBindings(
  { ...tokens, control_heights_px: { ...tokens.control_heights_px, "button.default": Infinity } },
  [bdHeight],
  { "button.default||light|": { height: "32px" } },
).errs.length > 0);
// Each mutation is isolated. Numeric JSON fields never accept numeric
// strings, including strings whose prefix happens to match the live value.
for (const bad of ["32garbage", " 32px ", "1foo", "NaN", "Infinity", NaN, Infinity, -Infinity, -1, null, undefined]) {
  const t = clone(tokens);
  if (bad === undefined) delete t.control_heights_px["button.default"];
  else t.control_heights_px["button.default"] = bad;
  expectNeg(`control height ${String(bad)} rejects before comparison`,
    validateBindings(t, [bdHeight], { "button.default||light|": { height: "32px" } }).errs
      .some((e) => /control_heights_px\.button\.default/.test(e) && /finite|unresolvable/.test(e)));
  if (bad !== undefined) expectNeg(`token leaf control height ${String(bad)} rejects`,
    validateTokens(t).some((e) => /control_heights_px\.button\.default.*invalid finite numeric/.test(e)));
}
expectNeg("wrong padding detected", validateBindings(tokens, [bdPad], {
  "button.default||light|": { "padding-left": "9px" },
}).errs.length > 0);
expectNeg("wrong line-height detected", validateBindings(tokens, [bdLh], {
  "button.default||light|": { "line-height": "19px" },
}).errs.length > 0);
// tolerance sanity: a sub-tolerance color difference must NOT fail
const bdBg = bindingsDoc.bindings.find((x) => x.automation_id === "button.default" && x.css_property === "background-color" && x.token_path.includes("srgb") && x.theme === "light");
expectPos("1/255 color drift within tolerance", validateBindings(tokens, [bdBg], {
  "button.default||light|": { "background-color": `rgba(24, 24, 24, 1)` },
}).errs.length === 0);
expectNeg("2/255 color drift detected", validateBindings(tokens, [bdBg], {
  "button.default||light|": { "background-color": `rgba(26, 26, 26, 1)` },
}).errs.length > 0);

// ---------- C03 structured shadows ------------------------------------------
const shadowEntry = Object.entries(tokens.shadows || {}).find(([, s]) => s.status === "used" && s.layers && s.layers.length);
if (!shadowEntry) {
  failed++;
  console.error("FAIL selftest setup: no used structured shadow in tokens.json");
} else {
  const [shName, shTok] = shadowEntry;
  const bdSh = bindingsDoc.bindings.find((x) => x.css_property === "box-shadow-layers" && x.token_path === `shadows.${shName}`);
  const liveShadow = (layers) =>
    layers.map((l) => `rgba(${[1, 3, 5].map((i) => parseInt(l.color.slice(i, i + 2), 16)).join(", ")}, ${l.alpha}) ${l.x}px ${l.y}px ${l.blur}px ${l.spread}px`).join(", ");
  if (!bdSh) {
    failed++;
    console.error(`FAIL selftest setup: no box-shadow-layers binding for shadows.${shName}`);
  } else {
    const key = `${bdSh.automation_id}|${bdSh.part || ""}|light|`;
    expectPos(`correct live shadow string passes (shadows.${shName})`, validateBindings(tokens, [bdSh], {
      [key]: { "box-shadow-layers": liveShadow(shTok.layers) },
    }).errs.length === 0);
    const mut = (fn) => {
      const t = clone(tokens);
      fn(t.shadows[shName].layers[0]);
      return t;
    };
    for (const [label, fn, re] of [
      ["offset x changed", (l) => { l.x += 1; }, /\.x/],
      ["offset y changed", (l) => { l.y += 1; }, /\.y/],
      ["blur changed", (l) => { l.blur += 1; }, /blur/],
      ["spread changed", (l) => { l.spread += 1; }, /spread/],
      ["alpha changed", (l) => { l.alpha += 0.05; }, /alpha/],
      ["color changed", (l) => { l.color = "#ff0000"; }, /color/],
    ]) {
      expectNeg(`used shadow ${label} detected`, validateBindings(mut(fn), [bdSh], {
        [key]: { "box-shadow-layers": liveShadow(shTok.layers) },
      }).errs.some((e) => re.test(e)));
    }
    // Astra's exact NaN alpha counterexample plus independent finite-domain
    // failures for every effect field. Validate before Math.abs comparisons.
    for (const field of ["x", "y", "blur", "spread", "alpha"]) {
      for (const bad of [NaN, Infinity, -Infinity, "1foo"]) {
        const t = mut((l) => { l[field] = bad; });
        const intended = (e) => e.includes(`shadows.${shName}`) && e.includes(`.${field}`) && /finite/.test(e);
        expectNeg(`shadows.${shName}.layers[0].${field}=${String(bad)} domain rejects`, validateTokens(t).some(intended));
        expectNeg(`shadows.${shName}.layers[0].${field}=${String(bad)} binding rejects before arithmetic`,
          validateBindings(t, [bdSh], { [key]: { "box-shadow-layers": liveShadow(shTok.layers) } }).errs.some(intended));
      }
    }
    for (const bad of [-0.1, 1.1]) {
      const t = mut((l) => { l.alpha = bad; });
      expectNeg(`used shadow alpha=${bad} outside domain rejects`,
        validateTokens(t).some((e) => /alpha/.test(e) && /invalid|outside/.test(e)));
    }
    const reversed = clone(tokens);
    reversed.shadows[shName].layers.reverse();
    expectNeg("used shadow layer order changed detected",
      validateBindings(reversed, [bdSh], { [key]: { "box-shadow-layers": liveShadow(shTok.layers) } }).errs.length > 0);
  }
}

// ---------- switch thumb position ------------------------------------------
const bdSwitchTx = bindingsDoc.bindings.find(
  (x) => x.automation_id === "switch.default.checked" && x.css_property === "translate-x" && x.part === "thumb",
);
expectNeg("switch thumb checked/unchecked mix-up detected",
  bdSwitchTx && validateBindings(tokens, [bdSwitchTx], {
    "switch.default.checked|thumb|light|": { "translate-x": "0px" },
    "switch.default.checked|thumb|dark|": { "translate-x": "0px" },
  }).errs.length > 0);

// ---------- C01 capture authority -------------------------------------------
// live fabricated from the committed contract = positive control
const liveFromContract = (c) => {
  const live = {};
  for (const el of c.elements) {
    for (const [capKey, cap] of Object.entries(el.captures || {})) {
      live[capKey] = live[capKey] || { ids: [], elements: {} };
      live[capKey].ids.push(el.automation_id);
      live[capKey].elements[el.automation_id] = { rect: cap.rect, box: cap.box, interaction: cap.interaction, parts: cap.parts };
    }
  }
  return live;
};
{
  const r = validateCaptureAuthority(contract, liveFromContract(contract));
  expectPos("capture authority passes on live fabricated from committed contract", r.errs.length === 0);
  const c2 = clone(contract);
  const sn = c2.elements.find((e) => e.automation_id === "settings-nav.appearance");
  sn.captures["navigation/default"] = clone(sn.captures["navigation/settings-nav-collapsed"]);
  const r2 = validateCaptureAuthority(c2, liveFromContract(contract));
  expectNeg("settings-nav.appearance collapsed-state rect copied over default -> capture-authority fail",
    r2.errs.some((e) => /capture-authority settings-nav\.appearance navigation\/default/.test(e)));
  const c3 = clone(contract);
  const nt = c3.elements.find((e) => e.automation_id === "native-text.single-line.selection");
  // same page, different capture state: the multiline-selection record has
  // unfocused interaction - copying it over native-text/default must fail
  // (interaction/box differ even though the layout rect is identical)
  nt.captures["native-text/default"] = clone(nt.captures["native-text/multiline-selection"]);
  const r3 = validateCaptureAuthority(c3, liveFromContract(contract));
  expectNeg("native-text.single-line.selection multiline-selection capture copied over native-text/default -> capture-authority fail",
    r3.errs.some((e) => /capture-authority native-text\.single-line\.selection native-text\/default/.test(e)));
  const c4 = clone(contract);
  const nt4 = c4.elements.find((e) => e.automation_id === "native-text.single-line.selection");
  nt4.captures["all/default"] = clone(nt4.captures["native-text/multiline-selection"]);
  const r4 = validateCaptureAuthority(c4, liveFromContract(contract));
  expectNeg("native-text.single-line.selection wrong-state capture copied over all/default -> capture-authority fail",
    r4.errs.some((e) => /capture-authority native-text\.single-line\.selection/.test(e)));
}

// ---------- C01-d same-state consistency --------------------------------------
{
  const ss = validateSameStateConsistency(contract);
  expectPos(`same-state consistency passes on committed contract (${ss.checked.ids} ids)`, ss.errs.length === 0);
  const c5 = clone(contract);
  const nt5 = c5.elements.find((e) => e.automation_id === "native-text.single-line.selection");
  nt5.captures["all/default"].box.box_shadow = null;
  expectNeg("native-text.single-line.selection all/default box_shadow nulled -> same-state fail",
    validateSameStateConsistency(c5).errs.some((e) => /same-state native-text\.single-line\.selection.*box\.box_shadow/.test(e)));
  const c6 = clone(contract);
  const nt6 = c6.elements.find((e) => e.automation_id === "native-text.single-line.selection");
  nt6.captures["all/default"].interaction = { focused: false, selection: null };
  expectNeg("native-text.single-line.selection all/default interaction.focused flipped -> same-state fail",
    validateSameStateConsistency(c6).errs.some((e) => /same-state native-text\.single-line\.selection.*interaction/.test(e)));
}

// ---------- C02 text ownership ----------------------------------------------
const liveTextFromContract = (c) => {
  const out = {};
  for (const el of c.elements) {
    for (const [capKey, cap] of Object.entries(el.captures || {})) {
      out[capKey] = out[capKey] || [];
      for (const r of cap.text_runs || []) {
        out[capKey].push({ owner: el.automation_id, part: r.part || null, content: r.content });
      }
    }
  }
  return out;
};
{
  expectPos("text ownership passes on inventory built from committed contract",
    validateTextOwnership(contract, liveTextFromContract(contract)).errs.length === 0);
  const c2 = clone(contract);
  const cb = c2.elements.find((e) => e.automation_id === "checkbox.default.unchecked");
  // delete the owner-attributed label text run from every capture
  for (const cap of Object.values(cb.captures)) {
    cap.text_runs = (cap.text_runs || []).filter((r) => r.part !== "label");
  }
  const r2 = validateTextOwnership(c2, liveTextFromContract(contract));
  expectNeg("checkbox.default.unchecked label text_run deleted -> text-ownership fail",
    r2.errs.some((e) => /text-ownership/.test(e) && /checkbox\.default\.unchecked/.test(e)));
}

// ---------- pixel guard ------------------------------------------------------
const { decodePng } = require("./png");
const { forcedPairError } = require("./pixels");
const lightPng = decodePng(fs.readFileSync(path.join(ROOT, "screenshots/light/components.png")));
const rectOf = (id) => {
  const el = contract.elements.find((e) => e.automation_id === id);
  return el.captures["components/default"].rect;
};
const hoverRect = rectOf("button.default.hover");
const twinRect = rectOf("button.default");
expectNeg("skipped forcing detected by pixel guard (twin-vs-twin identical crops)",
  typeof forcedPairError(lightPng, twinRect, twinRect, "injected-skip") === "string");
expectPos("real forced specimen differs from twin in light capture",
  forcedPairError(lightPng, hoverRect, twinRect, "button.default.hover") === null);

console.log(`\nselftest: ${pos} positive controls, ${neg} negative mutations`);
if (failed) { console.error(`selftest: ${failed} failure(s)`); process.exit(1); }
console.log("selftest: all cases behaved");
