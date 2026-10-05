// The ONE catalog - every specimen. DOM is transcribed verbatim from
// upstream wrappers: apps/v4/registry/bases/base/ui/*.tsx (element types,
// data-slot, cn-* hook classes, utility classes) plus the attributes Base
// UI 1.6.0 renders (data-checked/data-unchecked/data-indeterminate,
// data-disabled, data-active, data-open, data-side/data-align,
// aria-checked/aria-selected/aria-pressed/aria-invalid, roles).
// Forced-state cells carry data-force-state (applied via CDP at capture).

(() => {
  const h = (tag, attrs, inner) => {
    const a = Object.entries(attrs || {})
      .filter(([, v]) => v !== undefined && v !== null && v !== false)
      .map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${v}"`))
      .join("");
    const void_ = tag === "input";
    return `<${tag}${a}${void_ ? ">" : `>${inner || ""}</${tag}>`}`;
  };
  const ic = (name, cls) => window.icon(name, cls);

  // ---------- transcribed component helpers ----------
  const BTN_BASE =
    "cn-button group/button inline-flex shrink-0 items-center justify-center whitespace-nowrap transition-all outline-none select-none disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0";
  const btn = (variant, size, opts = {}) =>
    h(
      "button",
      {
        "data-slot": opts.dataSlot || "button",
        class: `${BTN_BASE} cn-button-variant-${variant} cn-button-size-${size} ${opts.cls || ""}`,
        type: "button",
        disabled: opts.disabled ? "" : undefined,
        "data-force-state": opts.force,
        "aria-invalid": opts.invalid || undefined,
        "data-automation-id": opts.id,
      },
      (opts.icon ? ic(opts.icon, opts.iconCls) : "") + (opts.label ?? (opts.icon ? "" : "Button")),
    );

  const BADGE_BASE =
    "cn-badge group/badge inline-flex w-fit shrink-0 items-center justify-center overflow-hidden whitespace-nowrap focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 [&>svg]:pointer-events-none";
  const badge = (variant, id) =>
    h(
      "span",
      {
        "data-slot": "badge",
        class: `${BADGE_BASE} cn-badge-variant-${variant}`,
        "data-automation-id": id,
      },
      "Badge",
    );

  const sep = (orientation, id) =>
    h("div", {
      "data-slot": "separator",
      "data-automation-id": id,
      "data-orientation": orientation,
      role: "separator",
      class:
        "shrink-0 bg-border data-horizontal:h-px data-horizontal:w-full data-vertical:w-px data-vertical:self-stretch",
    });

  const INPUT_BASE =
    "cn-input w-full min-w-0 outline-none file:inline-flex file:border-0 file:bg-transparent file:text-foreground placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50";
  const input = (opts = {}) =>
    h("input", {
      type: "text",
      "data-slot": "input",
      class: INPUT_BASE + " w-64",
      placeholder: opts.placeholder,
      value: opts.value,
      readonly: opts.readonly ? "" : undefined,
      disabled: opts.disabled ? "" : undefined,
      "aria-invalid": opts.invalid ? "true" : undefined,
      "data-force-state": opts.force,
      "data-automation-id": opts.id,
    });
  const TA_BASE =
    "cn-textarea flex field-sizing-content min-h-16 w-full outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50";
  const textarea = (opts = {}) =>
    h(
      "textarea",
      {
        "data-slot": "textarea",
        class: TA_BASE + " w-72" + (opts.cls ? " " + opts.cls : ""),
        placeholder: opts.placeholder,
        readonly: opts.readonly ? "" : undefined,
        disabled: opts.disabled ? "" : undefined,
        "aria-invalid": opts.invalid ? "true" : undefined,
        "data-force-state": opts.force,
        "data-automation-id": opts.id,
        rows: opts.rows,
      },
      opts.value,
    );

  const checkbox = (state, opts = {}) => {
    const checked =
      state === "checked"
        ? { "aria-checked": "true", "data-checked": true }
        : state === "indeterminate"
          ? { "aria-checked": "mixed", "data-indeterminate": true }
          : { "aria-checked": "false", "data-unchecked": true };
    const hasIndicator = state !== "unchecked";
    return h(
      "button",
      {
        type: "button",
        role: "checkbox",
        "data-slot": "checkbox",
        class:
          "cn-checkbox peer relative shrink-0 outline-none after:absolute after:-inset-x-3 after:-inset-y-2 disabled:cursor-not-allowed disabled:opacity-50",
        disabled: opts.disabled ? "" : undefined,
        "data-disabled": opts.disabled || undefined,
        "aria-invalid": opts.invalid ? "true" : undefined,
        "data-force-state": opts.force,
        "data-automation-id": opts.id,
        ...checked,
      },
      hasIndicator
        ? h(
            "span",
            {
              "data-slot": "checkbox-indicator",
              "data-part": "indicator",
              class:
                "cn-checkbox-indicator grid place-content-center text-current transition-none",
            },
            ic("check"),
          )
        : "",
    );
  };

  const radioItem = (checked, opts = {}) =>
    h(
      "button",
      {
        type: "button",
        role: "radio",
        "aria-checked": checked ? "true" : "false",
        "data-slot": "radio-group-item",
        class:
          "cn-radio-group-item group/radio-group-item peer relative aspect-square shrink-0 border outline-none after:absolute after:-inset-x-3 after:-inset-y-2 disabled:cursor-not-allowed disabled:opacity-50",
        disabled: opts.disabled ? "" : undefined,
        "data-disabled": opts.disabled || undefined,
        "data-checked": checked || undefined,
        "data-unchecked": checked ? undefined : true,
        "data-force-state": opts.force,
      },
      checked
        ? h(
            "span",
            {
              "data-slot": "radio-group-indicator",
              "data-part": "indicator",
              class: "cn-radio-group-indicator",
            },
            h("span", { class: "cn-radio-group-indicator-icon" }),
          )
        : "",
    );

  const radioGroup = (opts = {}) =>
    h(
      "div",
      {
        "data-slot": "radio-group",
        role: "radiogroup",
        class: "cn-radio-group w-full",
        "data-automation-id": opts.id,
      },
      opts.items
        .map((it) =>
          h(
            "div",
            { class: "flex items-center gap-2" },
            radioItem(it.checked, it) +
              h(
                "label",
                {
                  "data-slot": "label",
                  class:
                    "cn-label flex items-center select-none gap-2" +
                    (it.disabled ? " opacity-50" : ""),
                },
                it.label,
              ),
          ),
        )
        .join(""),
    );

  const switch_ = (checked, opts = {}) =>
    h(
      "button",
      {
        type: "button",
        role: "switch",
        "aria-checked": checked ? "true" : "false",
        "data-slot": "switch",
        "data-size": opts.size || "default",
        class:
          "cn-switch peer group/switch relative inline-flex items-center transition-all outline-none after:absolute after:-inset-x-3 after:-inset-y-2 data-disabled:cursor-not-allowed data-disabled:opacity-50",
        disabled: opts.disabled ? "" : undefined,
        "data-disabled": opts.disabled || undefined,
        "data-checked": checked || undefined,
        "data-unchecked": checked ? undefined : true,
        "data-force-state": opts.force,
        "data-automation-id": opts.id,
      },
      h("span", {
        "data-slot": "switch-thumb",
        "data-part": "thumb",
        class:
          "cn-switch-thumb pointer-events-none block ring-0 transition-transform",
      }),
    );

  // thumbAlignment="edge" (upstream slider.tsx): thumb center stays inside
  // the track - pos(v) = 6px + (trackW - 12px) * v/100, thumb = size-3 (12px)
  const sliderPos = (v) => `calc(6px + (100% - 12px) * ${v / 100})`;
  const slider = (opts = {}) => {
    const thumbs = opts.thumbs || [50];
    const lo = Math.min(...thumbs);
    const hi = Math.max(...thumbs);
    const rangeStyle =
      thumbs.length > 1
        ? `inset-inline-start:${sliderPos(lo)};width:calc((100% - 12px) * ${(hi - lo) / 100})`
        : `inset-inline-start:0;width:${sliderPos(hi)}`;
    const t = thumbs
      .map(
        (v, i) =>
          h("div", {
            "data-slot": "slider-thumb",
            "data-part": thumbs.length > 1 ? `thumb-${i}` : "thumb",
            "data-index": i,
            "data-orientation": "horizontal",
            role: "slider",
            tabindex: "0",
            "aria-valuenow": v,
            "aria-valuemin": "0",
            "aria-valuemax": "100",
            "aria-orientation": "horizontal",
            class:
              "cn-slider-thumb block shrink-0 select-none disabled:pointer-events-none disabled:opacity-50",
            style: `position:absolute;inset-inline-start:${sliderPos(v)};top:50%;translate:-50% -50%`,
          }),
      )
      .join("");
    return h(
      "div",
      {
        "data-slot": "slider",
        "data-orientation": "horizontal",
        class: "cn-slider data-horizontal:w-full w-48",
        "data-disabled": opts.disabled || undefined,
        "data-automation-id": opts.id,
      },
      h(
        "div",
        {
          class:
            "cn-slider relative flex w-full touch-none items-center select-none data-disabled:opacity-50 data-vertical:h-full data-vertical:w-auto data-vertical:flex-col",
          "data-disabled": opts.disabled || undefined,
        },
        h(
          "div",
          {
            "data-slot": "slider-track",
            "data-part": "track",
            "data-orientation": "horizontal",
            class:
              "cn-slider-track relative grow overflow-hidden select-none",
          },
          h("div", {
            "data-slot": "slider-range",
            "data-part": "range",
            class:
              "cn-slider-range select-none data-horizontal:h-full data-vertical:w-full",
            style: `position:relative;height:inherit;${rangeStyle}`,
          }),
        ) + t,
      ),
    );
  };

  const label = (text, opts = {}) =>
    h(
      "label",
      {
        "data-slot": "label",
        class:
          "cn-label flex items-center select-none group-data-[disabled=true]:pointer-events-none peer-disabled:cursor-not-allowed" +
          (opts.cls || ""),
        for: opts.for,
      },
      text,
    );

  const selectTrigger = (opts = {}) =>
    h(
      "button",
      {
        type: "button",
        "data-slot": "select-trigger",
        "data-size": opts.size || "default",
        role: "combobox",
        class:
          "cn-select-trigger flex w-fit items-center justify-between whitespace-nowrap outline-none disabled:cursor-not-allowed disabled:opacity-50 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center [&_svg]:pointer-events-none [&_svg]:shrink-0",
        "data-placeholder": opts.placeholder || undefined,
        "data-popup-open": opts.open || undefined,
        "aria-expanded": opts.open ? "true" : undefined,
        disabled: opts.disabled ? "" : undefined,
        "aria-invalid": opts.invalid ? "true" : undefined,
        "data-force-state": opts.force,
        "data-automation-id": opts.id,
      },
      h(
        "span",
        {
          "data-slot": "select-value",
          "data-part": "value",
          class: "cn-select-value flex flex-1 text-left",
        },
        opts.text,
      ) + ic("chevron-down", "cn-select-trigger-icon pointer-events-none"),
    );

  const TOGGLE_BASE =
    "cn-toggle group/toggle inline-flex items-center justify-center whitespace-nowrap outline-none hover:bg-muted focus-visible:ring-[3px] disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0";
  const toggle = (variant, on, opts = {}) =>
    h(
      "button",
      {
        type: "button",
        "data-slot": "toggle",
        "data-variant": variant,
        class: `${TOGGLE_BASE} cn-toggle-variant-${variant} cn-toggle-size-${opts.size || "default"}`,
        "aria-pressed": on ? "true" : "false",
        disabled: opts.disabled ? "" : undefined,
        "data-force-state": opts.force,
        "data-automation-id": opts.id,
      },
      ic(opts.icon || "bold") + (opts.label || ""),
    );

  const tabsList = (variant, opts = {}) =>
    h(
      "div",
      {
        "data-slot": "tabs",
        "data-orientation": "horizontal",
        class: "cn-tabs group/tabs flex data-horizontal:flex-col",
        "data-automation-id": opts.id,
      },
      h(
        "div",
        {
          "data-slot": "tabs-list",
          role: "tablist",
          "data-variant": variant,
          class: `cn-tabs-list cn-tabs-list-variant-${variant} group/tabs-list inline-flex w-fit items-center justify-center text-muted-foreground ${variant === "default" ? "bg-muted" : "gap-1 bg-transparent"}`,
        },
        opts.tabs
          .map((t, i) =>
            h(
              "button",
              {
                type: "button",
                role: "tab",
                "aria-selected": t.active ? "true" : "false",
                "data-slot": "tabs-trigger",
                class:
                  "cn-tabs-trigger relative inline-flex h-[calc(100%-1px)] flex-1 items-center justify-center whitespace-nowrap text-foreground/60 transition-all group-data-vertical/tabs:w-full group-data-vertical/tabs:justify-start hover:text-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 dark:text-muted-foreground dark:hover:text-foreground [&_svg]:pointer-events-none [&_svg]:shrink-0 group-data-[variant=line]/tabs-list:bg-transparent group-data-[variant=line]/tabs-list:data-active:bg-transparent dark:group-data-[variant=line]/tabs-list:data-active:border-transparent dark:group-data-[variant=line]/tabs-list:data-active:bg-transparent data-active:bg-background data-active:text-foreground dark:data-active:border-input dark:data-active:bg-input/30 dark:data-active:text-foreground after:absolute after:bg-foreground after:opacity-0 after:transition-opacity group-data-horizontal/tabs:after:inset-x-0 group-data-horizontal/tabs:after:bottom-[-5px] group-data-horizontal/tabs:after:h-0.5 group-data-vertical/tabs:after:inset-y-0 group-data-vertical/tabs:after:-right-1 group-data-vertical/tabs:after:w-0.5 group-data-[variant=line]/tabs-list:data-active:after:opacity-100",
                "data-active": t.active || undefined,
                disabled: t.disabled ? "" : undefined,
                "data-force-state": t.force,
                "data-part": `tab-${i}`,
              },
              t.label,
            ),
          )
          .join(""),
      ),
    );

  const field = (opts = {}) => {
    const desc = opts.error
      ? h(
          "div",
          {
            role: "alert",
            "data-slot": "field-error",
            "data-part": "error",
            class: "cn-field-error font-normal",
          },
          opts.error,
        )
      : h(
          "p",
          {
            "data-slot": "field-description",
            "data-part": "description",
            class:
              "cn-field-description leading-normal font-normal group-has-data-horizontal/field:text-balance last:mt-0 nth-last-2:-mt-1 [&>a]:underline [&>a]:underline-offset-4 [&>a:hover]:text-primary",
          },
          opts.description || "",
        );
    return h(
      "div",
      {
        role: "group",
        "data-slot": "field",
        "data-orientation": "vertical",
        "data-disabled": opts.disabled || undefined,
        class:
          "cn-field cn-field-orientation-vertical group/field flex w-72 flex-col *:w-full [&>.sr-only]:w-auto",
        "data-automation-id": opts.id,
      },
      label(opts.label, { cls: " cn-field-label group/field-label peer/field-label flex w-fit" }) +
        (opts.control || "") +
        desc,
    );
  };

  // ---------- catalog ----------
  const C = [];
  const e = (entry) => C.push(entry);

  // components - Button matrix: variants x states
  for (const v of ["default", "secondary", "outline", "ghost", "destructive", "link"]) {
    e({ automation_id: `button.${v}`, component: "button", variant: v, state: "normal", category: "components", render: () => btn(v, "default", { label: v === "link" ? "Link" : v[0].toUpperCase() + v.slice(1), id: `button.${v}` }) });
    for (const [st, force] of [["hover", "hover"], ["pressed", "active"], ["focus-visible", "focus,focus-visible"]]) {
      e({ automation_id: `button.${v}.${st}`, component: "button", variant: v, state: st, category: "components", render: () => btn(v, "default", { label: v[0].toUpperCase() + v.slice(1), force, id: `button.${v}.${st}` }) });
    }
    e({ automation_id: `button.${v}.disabled`, component: "button", variant: v, state: "disabled", category: "components", render: () => btn(v, "default", { label: "Disabled", disabled: true, id: `button.${v}.disabled` }) });
  }
  // Button sizes (default variant)
  for (const s of ["xs", "sm", "lg"]) {
    e({ automation_id: `button.default.size-${s}`, component: "button", variant: "default", state: "normal", size: s, category: "components", render: () => btn("default", s, { label: `Size ${s}`, id: `button.default.size-${s}` }) });
  }
  for (const s of ["icon-xs", "icon-sm", "icon", "icon-lg"]) {
    e({ automation_id: `button.default.size-${s}`, component: "button", variant: "default", state: "normal", size: s, category: "components", render: () => btn("default", s, { icon: "arrow-right", id: `button.default.size-${s}` }) });
  }
  e({ automation_id: "button.default.icon-inline-start", component: "button", variant: "default", state: "normal", category: "components", render: () => btn("default", "default", { icon: "plus", iconCls: "", label: "Add item", id: "button.default.icon-inline-start" }) });

  // Badge variants
  for (const v of ["default", "secondary", "outline", "destructive", "ghost", "link"]) {
    e({ automation_id: `badge.${v}`, component: "badge", variant: v, state: "normal", category: "components", render: () => badge(v, `badge.${v}`) });
  }

  // Separator
  e({ automation_id: "separator.horizontal", component: "separator", variant: "horizontal", state: "normal", category: "components", render: () => h("div", { class: "w-48" }, sep("horizontal", "separator.horizontal")) });
  e({ automation_id: "separator.vertical", component: "separator", variant: "vertical", state: "normal", category: "components", render: () => h("div", { class: "flex h-16 items-stretch" }, sep("vertical", "separator.vertical")) });

  // Card
  const card = (size, id) =>
    h(
      "div",
      {
        "data-slot": "card",
        "data-size": size,
        "data-automation-id": id,
        class: "cn-card group/card flex w-72 flex-col",
      },
      h("div", { "data-slot": "card-header", "data-part": "header", class: "cn-card-header group/card-header @container/card-header grid auto-rows-min items-start has-data-[slot=card-action]:grid-cols-[1fr_auto] has-data-[slot=card-description]:grid-rows-[auto_auto]" },
        h("div", { "data-slot": "card-title", "data-part": "title", class: "cn-card-title cn-font-heading" }, "Card Title") +
        h("div", { "data-slot": "card-description", "data-part": "description", class: "cn-card-description" }, "A short supporting description.")) +
      h("div", { "data-slot": "card-content", "data-part": "content", class: "cn-card-content" }, h("p", { class: "text-sm" }, "Card content region.")) +
      h("div", { "data-slot": "card-footer", "data-part": "footer", class: "cn-card-footer flex items-center justify-end gap-2" },
        btn("outline", "sm", { label: "Cancel" }) + btn("default", "sm", { label: "Save" })),
    );
  e({ automation_id: "card.default", component: "card", variant: "default", state: "normal", category: "components", render: () => card("default", "card.default") });
  e({ automation_id: "card.default.size-sm", component: "card", variant: "default", state: "normal", size: "sm", category: "components", render: () => card("sm", "card.default.size-sm") });

  // Alert (icon + title + description)
  for (const v of ["default", "destructive"]) {
    e({
      automation_id: `alert.${v}`, component: "alert", variant: v, state: "normal", category: "components",
      render: () =>
        h("div", {
          "data-slot": "alert", role: "alert", "data-automation-id": `alert.${v}`,
          class: `cn-alert cn-alert-variant-${v} group/alert relative w-80`,
        },
          ic("circle-alert") +
          h("div", { "data-slot": "alert-title", "data-part": "title", class: "cn-alert-title" }, v === "destructive" ? "Something went wrong" : "Heads up") +
          h("div", { "data-slot": "alert-description", "data-part": "description", class: "cn-alert-description" }, v === "destructive" ? "Your session has expired. Please sign in again." : "Your workspace was updated successfully.")),
    });
  }

  // Skeleton - typical upstream card placeholder shape
  e({
    automation_id: "skeleton.default", component: "skeleton", variant: "default", state: "normal", category: "components",
    render: () =>
      h("div", { "data-automation-id": "skeleton.default", class: "flex w-56 items-center gap-4" },
        h("div", { "data-slot": "skeleton", "data-part": "avatar", class: "cn-skeleton size-10 shrink-0 rounded-full" }) +
        h("div", { class: "flex-1 space-y-2" },
          h("div", { "data-slot": "skeleton", "data-part": "line", class: "cn-skeleton h-4 w-full" }) +
          h("div", { "data-slot": "skeleton", "data-part": "line", class: "cn-skeleton h-4 w-2/3" }))),
  });

  // Kbd
  e({ automation_id: "kbd.default", component: "kbd", variant: "default", state: "normal", category: "components", render: () => h("kbd", { "data-slot": "kbd", "data-automation-id": "kbd.default", class: "cn-kbd pointer-events-none inline-flex items-center justify-center select-none" }, "Ctrl K") });
  e({ automation_id: "kbd.group", component: "kbd", variant: "group", state: "normal", category: "components", render: () => h("kbd", { "data-slot": "kbd-group", "data-automation-id": "kbd.group", class: "cn-kbd-group inline-flex items-center" }, h("kbd", { "data-slot": "kbd", class: "cn-kbd pointer-events-none inline-flex items-center justify-center select-none" }, "Ctrl") + h("kbd", { "data-slot": "kbd", class: "cn-kbd pointer-events-none inline-flex items-center justify-center select-none" }, "Shift") + h("kbd", { "data-slot": "kbd", class: "cn-kbd pointer-events-none inline-flex items-center justify-center select-none" }, "P")) });

  // ---------- forms ----------
  e({ automation_id: "label.default", component: "label", variant: "default", state: "normal", category: "forms", render: () => h("div", { "data-automation-id": "label.default", class: "flex flex-col gap-2" }, label("Label") + input({ placeholder: "With a label" })) });

  for (const [st, opts] of [
    ["placeholder", { placeholder: "Placeholder text" }],
    ["filled", { value: "Filled value" }],
    ["focus-visible", { value: "Focused input", force: "focus,focus-visible" }],
    ["disabled", { placeholder: "Disabled input", disabled: true }],
    ["invalid", { value: "bad@input", invalid: true }],
  ]) {
    e({ automation_id: `input.default.${st}`, component: "input", variant: "default", state: st, category: "forms", render: () => input({ ...opts, id: `input.default.${st}` }) });
  }
  for (const [st, opts] of [
    ["placeholder", { placeholder: "Write something..." }],
    ["filled", { value: "A longer message spanning\ntwo lines of text." }],
    ["focus-visible", { value: "Focused textarea", force: "focus,focus-visible" }],
    ["disabled", { placeholder: "Disabled textarea", disabled: true }],
    ["invalid", { value: "invalid", invalid: true }],
  ]) {
    e({ automation_id: `textarea.default.${st}`, component: "textarea", variant: "default", state: st, category: "forms", render: () => textarea({ ...opts, id: `textarea.default.${st}` }) });
  }

  const cbRow = (cbEl, labelText) =>
    h("div", { class: "flex items-center gap-2" }, cbEl + h("span", { class: "cn-label text-sm font-medium" }, labelText));
  for (const st of ["unchecked", "checked", "disabled", "invalid"]) {
    const opts = { id: `checkbox.default.${st}` };
    if (st === "disabled") opts.disabled = true;
    if (st === "invalid") opts.invalid = true;
    e({ automation_id: opts.id, component: "checkbox", variant: "default", state: st, category: "forms", render: () => cbRow(checkbox(st === "disabled" || st === "invalid" ? "unchecked" : st, opts), st[0].toUpperCase() + st.slice(1)) });
  }
  e({ automation_id: "checkbox.default.focus-visible", component: "checkbox", variant: "default", state: "focus-visible", category: "forms", render: () => cbRow(checkbox("checked", { force: "focus,focus-visible", id: "checkbox.default.focus-visible" }), "Focus visible") });

  // Radio group: one group per state cell
  e({ automation_id: "radio-group.default.selected", component: "radio-group", variant: "default", state: "selected", category: "forms", render: () => radioGroup({ id: "radio-group.default.selected", items: [{ checked: true, label: "Option A" }, { checked: false, label: "Option B" }] }) });
  e({ automation_id: "radio-group.default.unselected", component: "radio-group", variant: "default", state: "unselected", category: "forms", render: () => radioGroup({ id: "radio-group.default.unselected", items: [{ checked: false, label: "Option A" }, { checked: false, label: "Option B" }] }) });
  e({ automation_id: "radio-group.default.focus-visible", component: "radio-group", variant: "default", state: "focus-visible", category: "forms", render: () => radioGroup({ id: "radio-group.default.focus-visible", items: [{ checked: true, label: "Option A" }, { checked: false, label: "Option B", force: "focus,focus-visible" }] }) });
  e({ automation_id: "radio-group.default.disabled", component: "radio-group", variant: "default", state: "disabled", category: "forms", render: () => radioGroup({ id: "radio-group.default.disabled", items: [{ checked: true, label: "Option A", disabled: true }, { checked: false, label: "Option B", disabled: true }] }) });

  // Switch
  e({ automation_id: "switch.default.unchecked", component: "switch", variant: "default", state: "unchecked", category: "forms", render: () => switch_(false, { id: "switch.default.unchecked" }) });
  e({ automation_id: "switch.default.checked", component: "switch", variant: "default", state: "checked", category: "forms", render: () => switch_(true, { id: "switch.default.checked" }) });
  e({ automation_id: "switch.default.focus-visible", component: "switch", variant: "default", state: "focus-visible", category: "forms", render: () => switch_(true, { force: "focus,focus-visible", id: "switch.default.focus-visible" }) });
  e({ automation_id: "switch.default.disabled", component: "switch", variant: "default", state: "disabled", category: "forms", render: () => switch_(true, { disabled: true, id: "switch.default.disabled" }) });
  e({ automation_id: "switch.default.size-sm", component: "switch", variant: "default", state: "checked", size: "sm", category: "forms", render: () => switch_(true, { size: "sm", id: "switch.default.size-sm" }) });

  // Slider
  e({ automation_id: "slider.default", component: "slider", variant: "default", state: "normal", category: "forms", render: () => slider({ id: "slider.default", thumbs: [50] }) });
  e({ automation_id: "slider.default.range", component: "slider", variant: "default", state: "normal", modifier: "range", category: "forms", render: () => slider({ id: "slider.default.range", thumbs: [25, 50] }) });
  e({ automation_id: "slider.default.disabled", component: "slider", variant: "default", state: "disabled", category: "forms", render: () => slider({ id: "slider.default.disabled", thumbs: [50], disabled: true }) });

  // Select trigger states
  e({ automation_id: "select.default.placeholder", component: "select", variant: "default", state: "placeholder", category: "forms", render: () => selectTrigger({ text: "Choose an option", placeholder: true, id: "select.default.placeholder" }) });
  e({ automation_id: "select.default.filled", component: "select", variant: "default", state: "filled", category: "forms", render: () => selectTrigger({ text: "Production", id: "select.default.filled" }) });
  e({ automation_id: "select.default.focus-visible", component: "select", variant: "default", state: "focus-visible", category: "forms", render: () => selectTrigger({ text: "Production", force: "focus,focus-visible", id: "select.default.focus-visible" }) });
  e({ automation_id: "select.default.disabled", component: "select", variant: "default", state: "disabled", category: "forms", render: () => selectTrigger({ text: "Production", disabled: true, id: "select.default.disabled" }) });
  e({ automation_id: "select.default.invalid", component: "select", variant: "default", state: "invalid", category: "forms", render: () => selectTrigger({ text: "Production", invalid: true, id: "select.default.invalid" }) });
  e({ automation_id: "select.default.size-sm", component: "select", variant: "default", state: "filled", size: "sm", category: "forms", render: () => selectTrigger({ text: "Production", size: "sm", id: "select.default.size-sm" }) });

  // Field - label + description, error, disabled
  e({ automation_id: "field.default", component: "field", variant: "default", state: "normal", category: "forms", render: () => field({ id: "field.default", label: "Email", description: "We'll only use this for notifications.", control: input({ placeholder: "you@example.com" }) }) });
  e({ automation_id: "field.default.invalid", component: "field", variant: "default", state: "invalid", category: "forms", render: () => field({ id: "field.default.invalid", label: "Email", error: "Enter a valid email address.", control: input({ value: "not-an-email", invalid: true }) }) });
  e({ automation_id: "field.default.disabled", component: "field", variant: "default", state: "disabled", category: "forms", render: () => field({ id: "field.default.disabled", label: "Email", description: "Locked for this account.", control: input({ placeholder: "you@example.com", disabled: true }), disabled: true }) });

  // ---------- navigation ----------
  for (const variant of ["default", "line"]) {
    e({ automation_id: `tabs.${variant}.active`, component: "tabs", variant, state: "active", category: "navigation", render: () => tabsList(variant, { id: `tabs.${variant}.active`, tabs: [{ label: "Account", active: true }, { label: "Password" }, { label: "Settings" }] }) });
    e({ automation_id: `tabs.${variant}.inactive`, component: "tabs", variant, state: "inactive", category: "navigation", render: () => tabsList(variant, { id: `tabs.${variant}.inactive`, tabs: [{ label: "Account" }, { label: "Password" }, { label: "Settings" }] }) });
    e({ automation_id: `tabs.${variant}.hover`, component: "tabs", variant, state: "hover", category: "navigation", render: () => tabsList(variant, { id: `tabs.${variant}.hover`, tabs: [{ label: "Account", active: true }, { label: "Password", force: "hover" }, { label: "Settings" }] }) });
    e({ automation_id: `tabs.${variant}.focus-visible`, component: "tabs", variant, state: "focus-visible", category: "navigation", render: () => tabsList(variant, { id: `tabs.${variant}.focus-visible`, tabs: [{ label: "Account", active: true }, { label: "Password", force: "focus,focus-visible" }, { label: "Settings" }] }) });
    e({ automation_id: `tabs.${variant}.disabled`, component: "tabs", variant, state: "disabled", category: "navigation", render: () => tabsList(variant, { id: `tabs.${variant}.disabled`, tabs: [{ label: "Account", active: true }, { label: "Password", disabled: true }, { label: "Settings" }] }) });
  }

  // Breadcrumb with ellipsis + current page
  e({
    automation_id: "breadcrumb.default", component: "breadcrumb", variant: "default", state: "current", category: "navigation",
    render: () =>
      h("nav", { "aria-label": "breadcrumb", "data-slot": "breadcrumb", "data-automation-id": "breadcrumb.default", class: "cn-breadcrumb" },
        h("ol", { "data-slot": "breadcrumb-list", class: "cn-breadcrumb-list flex flex-wrap items-center wrap-break-word" },
          h("li", { "data-slot": "breadcrumb-item", class: "cn-breadcrumb-item inline-flex items-center" },
            h("a", { "data-slot": "breadcrumb-link", "data-part": "link", class: "cn-breadcrumb-link", href: "#" }, "Home")) +
          h("li", { "data-slot": "breadcrumb-separator", "data-part": "separator", role: "presentation", "aria-hidden": "true", class: "cn-breadcrumb-separator" }, ic("chevron-right", "cn-rtl-flip")) +
          h("li", { "data-slot": "breadcrumb-item", class: "cn-breadcrumb-item inline-flex items-center" },
            h("span", { "data-slot": "breadcrumb-ellipsis", "data-part": "ellipsis", role: "presentation", "aria-hidden": "true", class: "cn-breadcrumb-ellipsis flex items-center justify-center" }, ic("ellipsis") + h("span", { class: "sr-only" }, "More"))) +
          h("li", { "data-slot": "breadcrumb-separator", "data-part": "separator", role: "presentation", "aria-hidden": "true", class: "cn-breadcrumb-separator" }, ic("chevron-right", "cn-rtl-flip")) +
          h("li", { "data-slot": "breadcrumb-item", class: "cn-breadcrumb-item inline-flex items-center" },
            h("a", { "data-slot": "breadcrumb-link", "data-part": "link", class: "cn-breadcrumb-link", href: "#" }, "Components")) +
          h("li", { "data-slot": "breadcrumb-separator", "data-part": "separator", role: "presentation", "aria-hidden": "true", class: "cn-breadcrumb-separator" }, ic("chevron-right", "cn-rtl-flip")) +
          h("li", { "data-slot": "breadcrumb-item", class: "cn-breadcrumb-item inline-flex items-center" },
            h("span", { "data-slot": "breadcrumb-page", "data-part": "current", role: "link", "aria-disabled": "true", "aria-current": "page", class: "cn-breadcrumb-page" }, "Breadcrumb")))),
  });

  // Pagination
  const pagLink = (opts) =>
    h("li", { "data-slot": "pagination-item" },
      h("a", {
        "data-slot": "pagination-link",
        "data-active": opts.active || undefined,
        "aria-current": opts.active ? "page" : undefined,
        "aria-label": opts.aria,
        href: "#",
        "data-part": opts.part || "link",
        class: `${BTN_BASE} cn-button-variant-${opts.active ? "outline" : "ghost"} cn-button-size-${opts.size || "icon"} cn-pagination-link` + (opts.cls || ""),
      }, opts.inner));
  e({
    automation_id: "pagination.default", component: "pagination", variant: "default", state: "current", category: "navigation",
    render: () =>
      h("nav", { role: "navigation", "aria-label": "pagination", "data-slot": "pagination", "data-automation-id": "pagination.default", class: "cn-pagination mx-auto flex w-full justify-center" },
        h("ul", { "data-slot": "pagination-content", class: "cn-pagination-content flex items-center" },
          pagLink({ aria: "Go to previous page", size: "default", cls: " cn-pagination-previous pl-1.5!", inner: ic("chevron-left", "cn-rtl-flip") + h("span", { class: "cn-pagination-previous-text hidden sm:block" }, "Previous"), part: "previous" }) +
          pagLink({ inner: "1" }) +
          pagLink({ inner: "2", active: true, part: "active" }) +
          pagLink({ inner: "3" }) +
          h("li", { "data-slot": "pagination-item" }, h("span", { "aria-hidden": "true", "data-slot": "pagination-ellipsis", "data-part": "ellipsis", class: "cn-pagination-ellipsis flex items-center justify-center" }, ic("ellipsis") + h("span", { class: "sr-only" }, "More pages"))) +
          pagLink({ aria: "Go to next page", size: "default", cls: " cn-pagination-next pr-1.5!", inner: h("span", { class: "cn-pagination-next-text hidden sm:block" }, "Next") + ic("chevron-right", "cn-rtl-flip"), part: "next" }))),
  });

  // Toggle - default/outline x states
  for (const v of ["default", "outline"]) {
    e({ automation_id: `toggle.${v}.off`, component: "toggle", variant: v, state: "off", category: "navigation", render: () => toggle(v, false, { icon: "bold", id: `toggle.${v}.off` }) });
    e({ automation_id: `toggle.${v}.on`, component: "toggle", variant: v, state: "on", category: "navigation", render: () => toggle(v, true, { icon: "bold", id: `toggle.${v}.on` }) });
    e({ automation_id: `toggle.${v}.hover`, component: "toggle", variant: v, state: "hover", category: "navigation", render: () => toggle(v, false, { icon: "bold", force: "hover", id: `toggle.${v}.hover` }) });
    e({ automation_id: `toggle.${v}.focus-visible`, component: "toggle", variant: v, state: "focus-visible", category: "navigation", render: () => toggle(v, false, { icon: "bold", force: "focus,focus-visible", id: `toggle.${v}.focus-visible` }) });
    e({ automation_id: `toggle.${v}.disabled`, component: "toggle", variant: v, state: "disabled", category: "navigation", render: () => toggle(v, false, { icon: "bold", disabled: true, id: `toggle.${v}.disabled` }) });
  }

  // Toggle Group (outline, sm is used in shell - here default size)
  e({
    automation_id: "toggle-group.outline", component: "toggle-group", variant: "outline", state: "on", category: "navigation",
    render: () =>
      h("div", {
        "data-slot": "toggle-group", "data-variant": "outline", "data-size": "default", "data-spacing": "2", "data-orientation": "horizontal",
        style: "--gap:2", "data-automation-id": "toggle-group.outline",
        class: "cn-toggle-group group/toggle-group flex w-fit flex-row items-center gap-[--spacing(var(--gap))] data-vertical:flex-col data-vertical:items-stretch",
      },
        ["bold", "italic", "underline"].map((a, i) =>
          h("button", {
            type: "button", "data-slot": "toggle-group-item", "data-variant": "outline", "data-size": "default", "data-spacing": "2",
            "aria-pressed": i === 0 ? "true" : "false",
            class: `${TOGGLE_BASE} cn-toggle-group-item cn-toggle-variant-outline cn-toggle-size-default shrink-0 focus:z-10 focus-visible:z-10`,
            "data-part": `item-${i}`,
          }, ic(a)),
        ).join("")),
  });

  // Button Group - toolbar pattern
  e({
    automation_id: "button-group.toolbar", component: "button-group", variant: "default", state: "normal", category: "navigation",
    render: () =>
      h("div", {
        role: "group", "data-slot": "button-group", "data-orientation": "horizontal", "data-automation-id": "button-group.toolbar",
        class: "cn-button-group cn-button-group-orientation-horizontal flex w-fit items-stretch *:focus-visible:relative *:focus-visible:z-10 [&>[data-slot=select-trigger]:not([class*='w-'])]:w-fit [&>input]:flex-1 *:data-slot:rounded-r-none [&>[data-slot]~[data-slot]]:rounded-l-none [&>[data-slot]~[data-slot]]:border-l-0",
      },
        btn("outline", "default", { icon: "bold" }) +
        btn("outline", "default", { icon: "italic" }) +
        btn("outline", "default", { icon: "underline" })),
  });

  // ---------- typography (upstream docs examples) ----------
  const typo = (id, tag, cls, text, part) =>
    h("div", { "data-automation-id": id, class: "flex flex-col items-start" },
      h(tag, { class: cls, "data-part": part || "text", "data-automation-elem": true }, text));
  const TYPOS = [
    ["typography.h1", "h1", "scroll-m-20 text-4xl font-extrabold tracking-tight text-balance lg:text-5xl", "Taxing Laughter: The Joke Tax Chronicles"],
    ["typography.h2", "h2", "scroll-m-20 border-b pb-2 text-3xl font-semibold tracking-tight first:mt-0", "The People of the Kingdom"],
    ["typography.h3", "h3", "scroll-m-20 text-2xl font-semibold tracking-tight", "The Joke Tax"],
    ["typography.h4", "h4", "scroll-m-20 text-xl font-semibold tracking-tight", "People stopped telling jokes"],
    ["typography.p", "p", "leading-7 [&:not(:first-child)]:mt-6", "The king, seeing how much happier his subjects were, realized the error of his ways and repealed the joke tax."],
    ["typography.blockquote", "blockquote", "mt-6 border-l-2 pl-6 italic", "After all, he said, everyone enjoys a good joke, so it's only fair that they should pay for the privilege."],
    ["typography.lead", "p", "text-muted-foreground text-xl", "A modal dialog that interrupts the user with important content and expects a response."],
    ["typography.large", "div", "text-lg font-semibold", "Are you absolutely sure?"],
    ["typography.small", "small", "text-sm leading-none font-medium", "Email address"],
    ["typography.muted", "p", "text-muted-foreground text-sm", "Enter your email address."],
    ["typography.inline-code", "code", "bg-muted relative rounded-sm px-[0.3rem] py-[0.2rem] font-mono text-sm font-semibold", "npx shadcn@latest init"],
  ];
  for (const [id, tag, cls, text] of TYPOS) {
    e({ automation_id: id, component: id.split(".")[1], variant: "default", state: "normal", category: "typography", kind: "text", render: () => h(tag, { class: cls, "data-automation-id": id, "data-part": "text" }, text) });
  }
  e({
    automation_id: "typography.list", component: "list", variant: "default", state: "normal", category: "typography", kind: "text",
    render: () =>
      h("ul", { class: "my-6 ml-6 list-disc [&>li]:mt-2", "data-automation-id": "typography.list" },
        h("li", { "data-part": "item" }, "1st level of puns: 5 gold coins") +
        h("li", { "data-part": "item" }, "2nd level of jokes: 10 gold coins") +
        h("li", { "data-part": "item" }, "3rd level of one-liners: 20 gold coins")),
  });
  e({
    automation_id: "typography.table", component: "table", variant: "default", state: "normal", category: "typography", kind: "text",
    render: () =>
      h("div", { class: "my-6 w-full overflow-y-auto", "data-automation-id": "typography.table" },
        h("table", { class: "w-full" },
          h("thead", {}, h("tr", { class: "even:bg-muted m-0 border-t p-0", "data-part": "header-row" },
            h("th", { class: "border px-4 py-2 text-left font-bold [&[align=center]]:text-center [&[align=right]]:text-right", "data-part": "header" }, "King's Treasury") +
            h("th", { class: "border px-4 py-2 text-left font-bold [&[align=center]]:text-center [&[align=right]]:text-right", "data-part": "header" }, "People's happiness"))) +
          h("tbody", {},
            h("tr", { class: "even:bg-muted m-0 border-t p-0", "data-part": "row" }, h("td", { class: "border px-4 py-2 text-left [&[align=center]]:text-center [&[align=right]]:text-right", "data-part": "cell" }, "Empty") + h("td", { class: "border px-4 py-2 text-left [&[align=center]]:text-center [&[align=right]]:text-right", "data-part": "cell" }, "Overflowing")) +
            h("tr", { class: "even:bg-muted m-0 border-t p-0", "data-part": "row" }, h("td", { class: "border px-4 py-2 text-left [&[align=center]]:text-center [&[align=right]]:text-right", "data-part": "cell" }, "Modest") + h("td", { class: "border px-4 py-2 text-left [&[align=center]]:text-center [&[align=right]]:text-right", "data-part": "cell" }, "Satisfied"))))),
  });

  // ---------- overlays (statically open, stage-clipped) ----------
  const stage = (id, inner, height) =>
    h("div", {
      "data-automation-id": `${id}`,
      class: `overlay-stage w-full ${height || "h-72"}`,
      "data-part": "stage",
    }, inner);

  const ovPositioner = (id, cls, inner) =>
    h("div", { class: `stage-clipped isolate z-50 ${cls}` }, inner);

  // Dialog
  e({
    automation_id: "dialog.default", component: "dialog", variant: "default", state: "open", category: "overlays", kind: "overlay",
    render: () =>
      stage("dialog.default",
        h("div", { class: "absolute left-4 top-4" },
          btn("outline", "default", { label: "Open dialog", id: "dialog.default.trigger", cls: " ", dataSlot: "dialog-trigger" })) +
        h("div", { "data-slot": "dialog-overlay", "data-part": "backdrop", "data-open": true, class: "cn-dialog-overlay absolute inset-0 isolate z-10" }) +
        h("div", {
          "data-slot": "dialog-content", role: "dialog", "aria-modal": "true",
          "data-automation-id": "dialog.default.content", "data-part": "content",
          "data-open": true, "data-side": "bottom", "data-align": "center",
          class: "cn-dialog-content absolute top-1/2 left-1/2 z-50 w-full max-w-[calc(100%-2rem)] -translate-x-1/2 -translate-y-1/2 outline-none sm:max-w-sm",
        },
          h("div", { "data-slot": "dialog-header", "data-part": "header", class: "cn-dialog-header flex flex-col" },
            h("div", { "data-slot": "dialog-title", "data-part": "title", class: "cn-dialog-title cn-font-heading" }, "Edit profile") +
            h("div", { "data-slot": "dialog-description", "data-part": "description", class: "cn-dialog-description" }, "Make changes to your profile here. Click save when you're done.")) +
          h("div", { class: "flex flex-col gap-2" }, input({ value: "rust-ui", placeholder: "Name" }) + input({ value: "@rustui", placeholder: "Username" })) +
          h("div", { "data-slot": "dialog-footer", "data-part": "footer", class: "cn-dialog-footer flex flex-col-reverse gap-2 sm:flex-row sm:justify-end" }, btn("default", "default", { label: "Save changes" })) +
          h("button", {
            type: "button", "data-slot": "dialog-close", "data-part": "close",
            class: `${BTN_BASE} cn-button-variant-ghost cn-button-size-icon-sm cn-dialog-close`,
          }, ic("x") + h("span", { class: "sr-only" }, "Close"))),
        "h-80"),
  });

  // Alert Dialog
  e({
    automation_id: "alert-dialog.default", component: "alert-dialog", variant: "default", state: "open", category: "overlays", kind: "overlay",
    render: () =>
      stage("alert-dialog.default",
        h("div", { class: "absolute left-4 top-4" },
          btn("outline", "default", { label: "Delete account", id: "alert-dialog.default.trigger", dataSlot: "alert-dialog-trigger" })) +
        h("div", { "data-slot": "alert-dialog-overlay", "data-part": "backdrop", "data-open": true, class: "cn-alert-dialog-overlay absolute inset-0 isolate z-10" }) +
        h("div", {
          "data-slot": "alert-dialog-content", role: "alertdialog", "aria-modal": "true",
          "data-size": "default", "data-automation-id": "alert-dialog.default.content", "data-part": "content",
          "data-open": true,
          class: "cn-alert-dialog-content group/alert-dialog-content absolute top-1/2 left-1/2 z-50 grid w-full max-w-[calc(100%-2rem)] -translate-x-1/2 -translate-y-1/2 outline-none sm:max-w-sm",
        },
          h("div", { "data-slot": "alert-dialog-header", "data-part": "header", class: "cn-alert-dialog-header" },
            h("div", { "data-slot": "alert-dialog-title", "data-part": "title", class: "cn-alert-dialog-title cn-font-heading" }, "Are you absolutely sure?") +
            h("div", { "data-slot": "alert-dialog-description", "data-part": "description", class: "cn-alert-dialog-description" }, "This action cannot be undone. This will permanently delete your account.")) +
          h("div", { "data-slot": "alert-dialog-footer", "data-part": "footer", class: "cn-alert-dialog-footer flex flex-col-reverse gap-2 group-data-[size=sm]/alert-dialog-content:grid group-data-[size=sm]/alert-dialog-content:grid-cols-2 sm:flex-row sm:justify-end" },
            btn("outline", "default", { label: "Cancel", cls: " cn-alert-dialog-cancel" }) +
            btn("destructive", "default", { label: "Continue", cls: " cn-alert-dialog-action" }))),
        "h-72"),
  });

  // Popover (anchored below trigger, centered - upstream side=bottom, align=center, sideOffset=4)
  e({
    automation_id: "popover.default", component: "popover", variant: "default", state: "open", category: "overlays", kind: "overlay",
    meta: { anchor: { side: "bottom", align: "center", sideOffset: 4, alignOffset: 0 } },
    render: () =>
      stage("popover.default",
        h("div", { class: "absolute left-1/2 top-8 -translate-x-1/2" },
          h("button", { type: "button", "data-slot": "popover-trigger", "data-automation-id": "popover.default.trigger", "data-popup-open": true, class: `${BTN_BASE} cn-button-variant-outline cn-button-size-default` }, "Open popover")) +
        h("div", { class: "absolute left-1/2 top-[calc(2rem+36px)] -translate-x-1/2 isolate z-50" },
          h("div", {
            "data-slot": "popover-content", "data-automation-id": "popover.default.content", "data-part": "content",
            "data-open": true, "data-side": "bottom", "data-align": "center",
            class: "cn-popover-content cn-popover-content-logical z-50 w-72 origin-(--transform-origin) outline-hidden",
          },
            h("div", { "data-slot": "popover-header", "data-part": "header", class: "cn-popover-header" },
              h("div", { "data-slot": "popover-title", "data-part": "title", class: "cn-popover-title" }, "Dimensions") +
              h("div", { "data-slot": "popover-description", "data-part": "description", class: "cn-popover-description" }, "Set the dimensions for the layer.")) +
            h("div", { class: "flex flex-col gap-2" }, input({ value: "25%", placeholder: "Width" }) + input({ value: "300px", placeholder: "Max. width" })))),
        "h-64"),
  });

  // Tooltip (anchored top - upstream side=top, align=center, sideOffset=4; Nova renders an arrow)
  e({
    automation_id: "tooltip.default", component: "tooltip", variant: "default", state: "open", category: "overlays", kind: "overlay",
    meta: { anchor: { side: "top", align: "center", sideOffset: 4, alignOffset: 0 } },
    render: () =>
      stage("tooltip.default",
        h("div", { class: "absolute left-1/2 top-[120px] -translate-x-1/2" },
          h("button", { type: "button", "data-slot": "tooltip-trigger", "data-automation-id": "tooltip.default.trigger", "data-popup-open": true, class: `${BTN_BASE} cn-button-variant-outline cn-button-size-default` }, "Hover me")) +
        h("div", { class: "absolute left-1/2 top-[88px] -translate-x-1/2 isolate z-50" },
          h("div", {
            "data-slot": "tooltip-content", "data-automation-id": "tooltip.default.content", "data-part": "content",
            "data-open": true, "data-side": "top", "data-align": "center",
            class: "cn-tooltip-content cn-tooltip-content-logical z-50 w-fit max-w-xs origin-(--transform-origin) bg-foreground text-background relative",
          },
            "Add to library" +
            h("div", { "data-part": "arrow", "data-side": "top",
              style: "position:absolute;left:calc(50% - 5px)",
              class: "cn-tooltip-arrow cn-tooltip-arrow-logical z-50 bg-foreground fill-foreground data-[side=top]:-bottom-2.5" }))),
        "h-52"),
  });

  // Dropdown Menu (anchored start/below trigger - upstream side=bottom, align=start, sideOffset=4)
  const menuItem = (label_, opts = {}) =>
    h("div", {
      role: "menuitem" + (opts.type ? opts.type : ""),
      "data-slot": opts.slot || "dropdown-menu-item",
      "data-part": opts.part || "item",
      "data-variant": opts.variant,
      "data-inset": opts.inset,
      "data-highlighted": opts.highlighted || undefined,
      "data-force-state": opts.force,
      "data-automation-id": opts.id,
      "data-checked": opts.checked,
      "aria-checked": opts.checked !== undefined ? (opts.checked ? "true" : "false") : undefined,
      "aria-disabled": opts.disabled || undefined,
      "data-disabled": opts.disabled || undefined,
      class: opts.cls,
    }, (opts.indicator || "") + (opts.icon ? ic(opts.icon) : "") + label_ + (opts.shortcut ? h("span", { "data-slot": "dropdown-menu-shortcut", "data-part": "shortcut", class: "cn-dropdown-menu-shortcut" }, opts.shortcut) : ""));
  e({
    automation_id: "dropdown-menu.default", component: "dropdown-menu", variant: "default", state: "open", category: "overlays", kind: "overlay",
    meta: { anchor: { side: "bottom", align: "start", sideOffset: 4, alignOffset: 0, width_authority: "w-(--anchor-width) - upstream sets --anchor-width = trigger offsetWidth; frozen at a fixed width for determinism" } },
    render: () =>
      stage("dropdown-menu.default",
        h("div", { class: "absolute left-8 top-6" },
          h("button", { type: "button", "data-slot": "dropdown-menu-trigger", "data-automation-id": "dropdown-menu.default.trigger", "data-popup-open": true, "aria-expanded": "true", "aria-haspopup": "menu", class: `${BTN_BASE} cn-button-variant-outline cn-button-size-default` }, "Open menu")) +
        h("div", { class: "absolute left-8 top-[calc(1.5rem+36px)] isolate z-50 outline-none" },
          h("div", {
            "data-slot": "dropdown-menu-content", "data-automation-id": "dropdown-menu.default.content", "data-part": "content",
            "data-open": true, "data-side": "bottom", "data-align": "start", role: "menu",
            class: "cn-dropdown-menu-content cn-dropdown-menu-content-logical z-50 max-h-(--available-height) w-(--anchor-width) origin-(--transform-origin) overflow-x-hidden overflow-y-auto outline-none data-closed:overflow-hidden w-48",
          },
            h("div", { "data-slot": "dropdown-menu-label", "data-part": "label", class: "cn-dropdown-menu-label" }, "My Account") +
            h("div", { "data-slot": "dropdown-menu-separator", "data-part": "separator", class: "cn-dropdown-menu-separator" }) +
            h("div", { "data-slot": "dropdown-menu-group", role: "group" },
              menuItem("Profile", { icon: "user", cls: "cn-dropdown-menu-item group/dropdown-menu-item relative flex cursor-default items-center outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0", shortcut: "Ctrl+Shift+P" }) +
              menuItem("Settings", { icon: "settings", cls: "cn-dropdown-menu-item group/dropdown-menu-item relative flex cursor-default items-center outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0", shortcut: "Ctrl+," }) +
              menuItem("Team members", { icon: "user", highlighted: true, force: "focus", id: "dropdown-menu.default.item-highlighted", cls: "cn-dropdown-menu-item group/dropdown-menu-item relative flex cursor-default items-center outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0" }) +
              menuItem("Show toolbar", { slot: "dropdown-menu-checkbox-item", type: "checkbox", checked: true, indicator: h("span", { "data-slot": "dropdown-menu-item-indicator", "data-part": "indicator", class: "cn-dropdown-menu-item-indicator" }, ic("check")), cls: "cn-dropdown-menu-checkbox-item group/dropdown-menu-item relative flex cursor-default items-center outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0" })) +
            h("div", { "data-slot": "dropdown-menu-separator", "data-part": "separator", class: "cn-dropdown-menu-separator" }) +
            menuItem("Delete account", { variant: "destructive", cls: "cn-dropdown-menu-item group/dropdown-menu-item relative flex cursor-default items-center outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0", icon: "log-out", part: "item-destructive" }))),
        "h-72"),
  });

  // Select open (trigger + popup list - selected item indicator)
  const selectItem = (label_, opts = {}) =>
    h("div", {
      role: "option",
      "aria-selected": opts.selected ? "true" : "false",
      "data-selected": opts.selected || undefined,
      "data-slot": "select-item",
      "data-part": opts.selected ? "item-selected" : "item",
      "data-highlighted": opts.highlighted || undefined,
      "data-force-state": opts.force || (opts.highlighted ? "focus" : undefined),
      "data-automation-id": opts.id,
      "data-focused": opts.focused || undefined,
      class: "cn-select-item relative flex w-full cursor-default items-center outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0",
    },
      h("span", { class: "cn-select-item-text shrink-0 whitespace-nowrap flex flex-1 gap-2" }, label_) +
      (opts.selected ? h("span", { "data-part": "indicator", class: "cn-select-item-indicator" }, ic("check", "cn-select-item-indicator-icon pointer-events-none")) : ""));
  e({
    automation_id: "select.default.open", component: "select", variant: "default", state: "open", category: "overlays", kind: "overlay",
    meta: { anchor: { side: "bottom", align: "center", sideOffset: 4, alignOffset: 0, alignItemWithTrigger: false, width_authority: "w-(--anchor-width) - upstream sets --anchor-width = trigger offsetWidth; frozen at a fixed width for determinism" } },
    render: () =>
      stage("select.default.open",
        h("div", { class: "absolute left-1/2 top-6 -translate-x-1/2" },
          selectTrigger({ text: "Production", id: "select.default.open.trigger" , open: true })) +
        h("div", { class: "absolute left-1/2 top-[calc(1.5rem+36px)] -translate-x-1/2 isolate z-50" },
          h("div", {
            "data-slot": "select-content", "data-automation-id": "select.default.open.content", "data-part": "content",
            "data-open": true, "data-side": "bottom", "data-align": "center", "data-align-trigger": "false", role: "listbox",
            class: "cn-select-content cn-select-content-logical relative isolate z-50 max-h-(--available-height) w-(--anchor-width) origin-(--transform-origin) overflow-x-hidden overflow-y-auto data-[align-trigger=true]:animate-none w-52",
          },
            h("div", { "data-slot": "select-group", role: "group", class: "cn-select-group scroll-my-1 p-1" },
              h("div", { "data-slot": "select-label", "data-part": "label", class: "cn-select-label" }, "Environment") +
              selectItem("Development") +
              selectItem("Staging", { highlighted: true, id: "select.default.open.item-highlighted" }) +
              selectItem("Production", { selected: true }) +
              selectItem("Preview")))),
        "h-64"),
  });

  // ---------- native text (upstream Input/Textarea visual contract) ----------
  const nt = (kind, state, opts = {}) => {
    const id = `native-text.${kind}.${state}`;
    const v = {
      placeholder: { placeholder: "Placeholder text" },
      filled: { value: kind === "multiline" ? "First line of a note.\nSecond line with more text.\nThird line closes it out." : "A line of prose" },
      focused: { value: kind === "multiline" ? "Focused multiline text\nwith a second line." : "Focused text", force: "focus,focus-visible" },
      selection: { value: kind === "multiline" ? "First line of a note.\nSecond line with more text.\nThird line closes it out." : "Select this phrase" },
      disabled: { placeholder: kind === "multiline" ? "Disabled textarea" : "Disabled input", disabled: true },
      readonly: { value: kind === "multiline" ? "Read-only body.\nNo editing allowed." : "Read-only value", readonly: true },
      invalid: { value: "not-an-email", invalid: true },
    }[state];
    const el_ = kind === "single-line" ? input({ ...v, id }) : textarea({ ...v, id, cls: "resize-none" });
    return el_;
  };
  for (const st of ["placeholder", "filled", "focused", "selection", "disabled", "readonly", "invalid"]) {
    e({ automation_id: `native-text.single-line.${st}`, component: "native-text", variant: "single-line", state: st, category: "native-text", kind: "text", render: () => nt("single-line", st) });
  }
  for (const st of ["placeholder", "filled", "focused", "selection", "disabled", "readonly"]) {
    e({ automation_id: `native-text.multiline.${st}`, component: "native-text", variant: "multiline", state: st, category: "native-text", kind: "text", render: () => nt("multiline", st) });
  }

  window.CATALOG = C;
  window.PAGES = [
    { id: "all", label: "All" },
    { id: "components", label: "Components" },
    { id: "forms", label: "Forms" },
    { id: "navigation", label: "Navigation" },
    { id: "typography", label: "Typography" },
    { id: "overlays", label: "Overlays" },
    { id: "native-text", label: "Native Text" },
  ];
})();
