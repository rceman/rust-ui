// Generates coverage.json - for every CORE family, the bounded
// upstream-selector-to-specimen table. Each entry maps a recipe/wrapper
// state or size selector to:
//   specimen: <automation_id>            - a specimen exists
//   specimens: [ids]                     - a matrix column/row covers it
//   equivalent_to: <automation_id>       - identical computed output (proof
//     = same element subtree produces same computed styles; documented)
//   n/a: "<reason>"                      - no desktop-relevant visual output
// reference:check fails if a mapping references a missing contract id, or a
// core family lacks bindings.
const fs = require("node:fs");
const path = require("node:path");

const V = ["default", "secondary", "outline", "ghost", "destructive", "link"];
const iv = (v, st) => `button.${v}.${st}`;
const matrix = (comp, variants, states) =>
  variants.flatMap((v) => states.map((st) => `${comp}.${v}.${st}`));

const families = {
  button: {
    states: {
      "cn-button-variant-{default,secondary,outline,ghost,destructive,link}": { specimens: V.map((v) => `button.${v}`) },
      "hover:": { specimens: V.map((v) => iv(v, "hover")) },
      "active:not-aria-[haspopup]:translate-y-px (pressed)": { specimens: V.map((v) => iv(v, "pressed")) },
      "focus-visible:": { specimens: V.map((v) => iv(v, "focus-visible")) },
      "disabled:": { specimens: V.map((v) => iv(v, "disabled")) },
      "aria-invalid:": { specimens: V.map((v) => iv(v, "invalid")) },
      "data-icon=inline-start": { specimen: "button.default.icon-inline-start" },
      "data-icon=inline-end": { specimen: "button.default.icon-inline-end" },
      "cn-button-size-{xs,sm,default,lg}": { specimens: ["button.default.size-xs", "button.default.size-sm", "button.default", "button.default.size-lg"] },
      "aria-expanded / data-popup-open (menu-trigger output)": { equivalent_to: "dropdown-menu.default", reason: "the open trigger is the dropdown-menu specimen's trigger part (aria-expanded recorded); button recipe carries no distinct open variant beyond translate suppression" },
      "cn-button-size-icon-*": { equivalent_to: "icon-button.default.size-icon", reason: "icon sizes are the icon-button family's specimens (component split)" },
    },
  },
  "icon-button": {
    states: {
      "variant x {normal,hover,pressed,focus-visible,disabled}": { specimens: matrix("icon-button", ["default", "secondary", "outline", "ghost", "destructive"], ["", "hover", "pressed", "focus-visible", "disabled"]).map((x) => x.replace(/\.$/, "")) },
      "cn-button-size-icon-{xs,sm,icon,lg}": { specimens: ["icon-button.default.size-icon-xs", "icon-button.default.size-icon-sm", "icon-button.default.size-icon", "icon-button.default.size-icon-lg"] },
    },
  },
  input: {
    states: {
      "placeholder": { specimen: "input.default.placeholder" },
      "filled": { specimen: "input.default.filled" },
      "focus-visible (border-ring + ring-ring/50)": { specimen: "input.default.focus-visible" },
      "disabled (bg-input/50)": { specimen: "input.default.disabled" },
      "aria-invalid (border-destructive + ring-destructive/20)": { specimen: "input.default.invalid" },
      "aria-invalid + focus-visible combined": { specimen: "input.default.invalid-focus-visible", note: "measured distinct (rev 2): invalid paints outline-width 3px, invalid+focus-visible 1px, both themes" },
      "readonly": { equivalent_to: "input.default.filled", reason: "upstream recipe has no readonly styling - visually identical to filled" },
      "file: (file input styling)": { "n/a": "file picker chrome is not a desktop rust-ui contract surface" },
    },
  },
  textarea: {
    states: {
      "placeholder": { specimen: "textarea.default.placeholder" },
      "filled": { specimen: "textarea.default.filled" },
      "focus-visible": { specimen: "textarea.default.focus-visible" },
      "disabled": { specimen: "textarea.default.disabled" },
      "aria-invalid": { specimen: "textarea.default.invalid" },
      "aria-invalid + focus-visible combined": { specimen: "textarea.default.invalid-focus-visible", note: "measured distinct (rev 2): outline-width 3px -> 1px, both themes" },
      "readonly": { equivalent_to: "textarea.default.filled", reason: "no upstream readonly styling" },
    },
  },
  "native-text": {
    states: {
      "single-line placeholder/filled/focused/selection/disabled/readonly/invalid": { specimens: ["native-text.single-line.placeholder", "native-text.single-line.filled", "native-text.single-line.focused", "native-text.single-line.selection", "native-text.single-line.disabled", "native-text.single-line.readonly", "native-text.single-line.invalid"] },
      "multiline same set incl. invalid": { specimens: ["native-text.multiline.placeholder", "native-text.multiline.filled", "native-text.multiline.focused", "native-text.multiline.selection", "native-text.multiline.disabled", "native-text.multiline.readonly", "native-text.multiline.invalid"] },
    },
    note: "native-text uses the upstream input/textarea recipes but documents the future RichEdit contract; resize grip suppressed (UA artifact)",
  },
  checkbox: {
    states: {
      "unchecked / checked / focus-visible / disabled": { specimens: ["checkbox.default.unchecked", "checkbox.default.checked", "checkbox.default.focus-visible", "checkbox.default.disabled"] },
      "disabled-checked": { specimen: "checkbox.default.disabled-checked" },
      "aria-invalid (border-destructive)": { specimen: "checkbox.default.invalid" },
      "aria-invalid + checked (aria-invalid:aria-checked:border-primary)": { specimen: "checkbox.default.invalid-checked" },
      "data-indeterminate": { "n/a": "upstream Nova recipe has no indeterminate styling; Base UI exposes data-indeterminate unstyled - deferred" },
    },
  },
  "radio-group": {
    states: {
      "checked + unchecked items": { specimens: ["radio-group.default.selected", "radio-group.default.unselected"] },
      "focus-visible": { specimen: "radio-group.default.focus-visible" },
      "disabled": { specimen: "radio-group.default.disabled" },
      "aria-invalid": { specimen: "radio-group.default.invalid" },
    },
    parts: "both items keyed item[option-a]/item[option-b]; checked indicator item[option-a].indicator (the dot)",
  },
  switch: {
    states: {
      "checked / unchecked / focus-visible / disabled": { specimens: ["switch.default.unchecked", "switch.default.checked", "switch.default.focus-visible", "switch.default.disabled"] },
      "disabled-checked": { specimen: "switch.default.disabled-checked" },
      "aria-invalid": { specimen: "switch.default.invalid" },
      "size sm + default": { specimens: ["switch.default.size-sm", "switch.default.size-sm-unchecked", "switch.default.checked"] },
    },
  },
  select: {
    states: {
      "trigger placeholder/filled/focus-visible/disabled/invalid": { specimens: ["select.default.placeholder", "select.default.filled", "select.default.focus-visible", "select.default.disabled", "select.default.invalid"] },
      "trigger size sm": { specimen: "select.default.size-sm" },
      "trigger open (data-popup-open + aria-expanded)": { specimen: "select.default.open.trigger" },
      "content: label + items (normal, highlighted, selected+indicator, disabled) + separator": { specimens: ["select.default.open.content", "select.default.open.item-highlighted"], parts: ["label[env]", "item[development]", "item[production]", "item[deprecated]", "item[eu-west]", "separator"], parts_on: "select.default.open.content" },
      "alignItemWithTrigger=true item-aligned mode": { "n/a": "frozen in alignItemWithTrigger=false (side=bottom) as in upstream select-example; deferred to a later reference version" },
    },
  },
  tabs: {
    states: {
      "default + line variants x active/inactive/hover/focus-visible/disabled": { specimens: matrix("tabs", ["default", "line"], ["active", "inactive", "hover", "focus-visible", "disabled"]) },
    },
  },
  "dropdown-menu": {
    states: {
      "label + item normal/highlighted/disabled + destructive + destructive-highlighted": { specimens: ["dropdown-menu.default.content", "dropdown-menu.default.item-highlighted", "dropdown-menu.default.item-destructive-highlighted"], parts: ["label", "item[profile]", "item[settings]", "item[billing]", "item[delete]"], parts_on: "dropdown-menu.default.content" },
      "checkbox item checked + unchecked": { specimen: "dropdown-menu.default.content", parts: ["item[toolbar]", "item[word-wrap]"] },
      "radio items checked + unchecked": { specimen: "dropdown-menu.default.content", parts: ["item[panel-left]", "item[panel-right]"] },
      "separator": { specimen: "dropdown-menu.default.content", parts: ["separator[1]", "separator[2]"] },
      "trigger aria-expanded + data-popup-open": { specimen: "dropdown-menu.default.trigger" },
      "submenu (SubTrigger/SubContent)": { "n/a": "single bounded stage; sub-menu surface reuses the same content recipe - no distinct visuals beyond slide-in direction" },
    },
  },
  "settings-nav": {
    states: {
      "expanded (collapsible=none, w=16rem)": { specimen: "settings-nav.default" },
      "collapsed rail (collapsible=icon, w=3rem)": { specimen: "settings-nav.default", capture_state: "settings-nav-collapsed" },
      "item states: default/hover/focus-visible/active/disabled": { specimens: ["settings-nav-item.default", "settings-nav-item.default.hover", "settings-nav-item.default.focus-visible", "settings-nav-item.default.active", "settings-nav-item.default.disabled"] },
      "mobile Sheet/offcanvas/useIsMobile": { "n/a": "rust-ui is desktop-only; upstream mobile path vendored verbatim, never transcribed" },
    },
    note: "uses --sidebar/--sidebar-accent neutral tokens only; upstream dark --sidebar-primary (blue) is never referenced",
  },
  tooltip: { states: { "open top + arrow": { specimens: ["tooltip.default.trigger", "tooltip.default.content"] } } },
  popover: { states: { "open anchored content": { specimens: ["popover.default.trigger", "popover.default.content"] } } },
  dialog: {
    states: {
      "open surface: title/description/body/footer/close + backdrop": { specimen: "dialog.default.content", parts: ["header.title", "header.description", "close", "footer"] },
      "close-button hover/pressed": { equivalent_to: "icon-button.ghost", reason: "close is a ghost icon-button at its own 24px size; colors identical, size recorded via part rect" },
    },
  },
  field: {
    states: {
      "vertical default / invalid / disabled": { specimens: ["field.default", "field.default.invalid", "field.default.disabled"] },
      "orientation=horizontal (label+description beside control)": { specimen: "field.horizontal" },
    },
  },
  kbd: { states: { "default + group": { specimens: ["kbd.default", "kbd.group"] } } },
  card: { states: { "default + size-sm": { specimens: ["card.default", "card.default.size-sm"] } } },
  separator: { states: { "horizontal + vertical": { specimens: ["separator.horizontal", "separator.vertical"] } } },
  badge: { states: { "all 6 variants": { specimens: ["badge.default", "badge.secondary", "badge.outline", "badge.destructive", "badge.ghost", "badge.link"] } } },
  alert: { states: { "default + destructive": { specimens: ["alert.default", "alert.destructive"] } } },
  skeleton: { states: { "default (avatar + lines)": { specimen: "skeleton.default" } } },
  label: { states: { "default": { specimen: "label.default" } } },
  typography: {
    states: {
      "h1/h2/h3/h4/p/lead/small/muted/inline-code": { specimens: ["typography.h1", "typography.h2", "typography.h3", "typography.h4", "typography.p", "typography.lead", "typography.small", "typography.muted", "typography.inline-code"] },
      "code-block (docs surface, Geist Mono, no highlighting)": { specimen: "typography.code-block" },
    },
  },
  "gallery-shell": {
    states: {
      "tabs nav + theme switch (system/light/dark)": { specimens: ["shell.nav.all", "shell.nav.components", "shell.theme.light", "shell.theme.dark", "shell.theme.system"] },
    },
  },
};

const coverage = {
  "$schema": "rust-ui.shadcn-reference.coverage/0.1",
  reference_version: "0.1",
  candidate_revision: 2,
  rule: "core = complete visually material desktop state/size coverage (every upstream recipe/wrapper state selector maps to a specimen, a measured equivalence, or an n/a reason); later = representative; reference-only = no completeness commitment",
  families,
};
fs.writeFileSync(path.join(__dirname, "..", "coverage.json"), JSON.stringify(coverage, null, 2));
console.log("coverage.json written:", Object.keys(families).length, "core families");
