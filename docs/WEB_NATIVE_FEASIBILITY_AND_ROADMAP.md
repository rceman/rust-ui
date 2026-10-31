# Web-native feasibility and roadmap

**Status: STRATEGIC DIRECTION APPROVED IN PRINCIPLE BY PLANNER; TECHNICAL CHOICES REMAIN PROVISIONAL.** Founder publication/merge authorization is pending. This analysis provides no implementation authorization, measured memory guarantee or compatibility certification. The concise approval record is in [PRODUCT_VISION.md](PRODUCT_VISION.md#strategic-decision-record).

**Prompt:** `rust-ui-astra-PRODUCT-VISION-20261031-225600`.

**Base:** verified remote `main`, `873c9996a908d5307c364925bfec6b4c1c40eb54`, on 2026-10-31. The documentation branch intentionally starts from main, not active P12. Remote P12 was `cb6d888207321a2b724e68c302175661245ee00e` when inspected. The [systems audit](https://github.com/rceman/rust-ui/blob/a8a0cf0fcb5e66e508dd9a2c2234efac56529c1a/docs/reviews/ASTRA_SYSTEM_WORKFLOW_AUDIT_20261031.md) covers that P12 snapshot; subsequent Worker changes are outside this proposal's acceptance claims.

Read with [PRODUCT_VISION.md](PRODUCT_VISION.md). This review used repository contracts, selected actual source, current primary standards/tool documentation and historical evidence. It ran no native application, benchmark, screenshot sweep or Rust test suite. Quantitative budgets below are proposed targets or arithmetic examples, never new measurements.

**Approval reconciliation:** `rust-ui-astra-VISION-APPROVAL-20261031-235000`, following Planner review of `e95fcd92ae3897cc4ba40949678e6c4f8885d436`. The feasibility research and limitations are preserved without a new research pass. **P12 remains the active implementation milestone.**

## Strategic conclusion

**The direction is feasible with a versioned compatibility profile and staged native capability growth.** A compiler can remove parsing, static cascade work and authoring machinery from deployed applications while preserving the existing typed native core. It cannot remove live layout, text measurement, state-dependent style resolution or application logic.

The current foundation is compatible with this direction but not sufficient to claim it implemented. Permanent prohibition of relative layout/Flexbox/Grid semantics would conflict with the migration ambition. Prohibition of a browser engine, DOM runtime or mandatory JavaScript VM does not: useful layout algorithms and compiled styling can exist independently of those systems.

Under 100 MB is a credible engineering target for bounded utilities, settings/workbench applications and modest local-data applications, **not a demonstrated cross-platform result**. Display surfaces, native text peers, font fallback, decoded assets and application caches are the dominant risks. General-purpose readiness also requires application lifecycle, packaging and platform behavior beyond a component catalog.

## Existing architecture: preserve, extend, reconsider

The required repository documents were inspected: [product scope](PRODUCT_SCOPE.md), [style/customization](STYLE_CUSTOMIZATION_MODEL.md), [platform contracts](PLATFORM_CONTRACTS.md), [backend architecture](BACKEND_ARCHITECTURE.md), [programming model](PROGRAMMING_MODEL.md) and [review authority](REVIEW_AUTHORITY.md). Some retain concept-era status text; concrete signatures are not treated as proof of implemented capability. Actual seams include [typed geometry](../src/geom.rs), [layout](../src/layout/mod.rs), [style types](../src/style/mod.rs), [runtime](../src/runtime/mod.rs), [TextValue](../src/text.rs) and [feature/dependency configuration](../Cargo.toml).

| Category | Existing decision | Disposition for the founder's vision |
|---|---|---|
| Preserve | One retained generational arena; staged keyed transactions; typed events | Compiler emits into the same model. No DOM/virtual-DOM mirror, alternate update loop or foreign native handles. |
| Preserve | Native text/input/accessibility; generation fencing and deterministic teardown | HTML authoring cannot bypass platform semantics. Backend differences are capability-tested. |
| Preserve | Typed style/layout ownership; concrete equality drives work; bounded damage | Extend typed vocabulary when required. Do not inject generic CSS strings or pre-resolved renderer state. |
| Preserve | Optional frontend depends on core, not vice versa | Build dependencies remain outside deployment; compiled dynamic support is opt-in and separately measured. |
| Temporary exclusion | No parser, selectors, variables or frontend integration in v0.1 | End the exclusion only in a separately authorized compiler milestone. |
| Temporary exclusion | Narrow effects, units and authored motion | Add individually tested native capabilities; no wholesale browser-effects promise. Some implementation capabilities have already grown beyond old prose. |
| Strategic direction approved; policy amendment pending | Style document calls browser-relative layout and general Flex/Grid requirements genuinely rejected | These are not permanent product bans under the approved direction. Keep the native/no-mandatory-browser boundary; extend typed algorithms only through an authorized milestone and reconcile older wording separately. |
| Requires extension | No implicit core inheritance; exclusive native state branches; limited Length vocabulary | Frontend resolves inheritance/combined predicates; relative constraints need typed shared layout inputs. Existing Rust behavior remains stable. |
| Requires explicit policy | Rust inline host patches outrank frontend CSS; OS accessibility enforcement is last | This is a native host policy, not literal browser cascade. HTML inline style remains inside the frontend cascade. |
| Genuine incompatibility | Arbitrary live DOM scripting and browser APIs with no equivalent runtime | Migration must rewrite/adapt these uses. Do not promise transparent execution. |
| Genuine incompatibility | Unlimited content/effects/windows under a fixed 100 MB ceiling | Define workload envelopes and charge optional capabilities honestly. |

The older prohibition on mandatory async runtime, browser fallback, CI and other backend implementation remains effective for current work. This proposal authorizes none of those changes. Future backend work needs its own task. Native editing restrictions remain until a proven native capability extension is approved; the compiler is not a reason to add a custom editor or HWND substitution.

## Smallest useful architecture

| Responsibility | Proposed owner and boundary |
|---|---|
| Parsing/analysis | Compiler tooling: HTML/CSS/component AST, imports, diagnostics, source locations, asset/license inventory. Temporary compile-time trees are allowed. |
| Optimization/lowering | Compiler: constant folding, static cascade, selector specialization, typed declarations/bindings, dead-code elimination proven against declared dynamic inputs. |
| Dynamic authoring support | Optional compiled functions/tables over arena identity: state/class predicates, variable dependencies and responsive conditions. No source parser or general CSS string evaluation in production. |
| Components | Existing typed component semantics, plus explicit props/events/slots and logical parts. Plain HTML does not silently acquire Shadcn styling. |
| Runtime/core | Existing update/view/transaction model, native state, shared typed validation, resolved values, layout, damage and scheduling. |
| Platform | One backend implementation of native window, text, input, accessibility, rendering and resource contracts per OS. |
| Migration | Source adapters and capability reports; generated code and explicit replacement tasks. No runtime compatibility service. |
| Development | Existing devctl plus compiler diagnostics/source maps; optional native preview and coalesced hot reload. |
| Packaging | Native executable/resources, optional desktop services, signing/installer/update integration; no compiler/watcher/browser automatically shipped. |

Begin with an external compiler tool or build dependency and generated Rust that uses existing builders. Do not create eight crates or a plugin registry to match this table. Split packages only where it enforces build-versus-runtime dependency isolation or an independently useful release boundary. Generated source is an initial inspectable lowering, not a promise that enormous generated Rust per element will remain efficient; measure code size and compile time before changing representation.

The compiler may use a frontend-specific intermediate representation. Core continues to receive typed style patches and separate typed layout constraints. Intern constants and share compiled recipe data where safe; do not add a generic dynamic property bag to every node. Optional metadata stores only identifiers/dependency indexes necessary for the enabled profile.

### One tree, several relationships

CSS styling ancestry, accessibility relationships, component parts and reconciliation keys are related but not interchangeable. Preserve logical ancestry through same-arena semantic nodes or generation-keyed metadata; never mirror the entire live UI in a second frontend tree. Native layout may maintain derived caches, not a second authoritative lifetime graph.

CSS `id`/class changes must not remount nodes. Reconciliation keys remain parent-scoped identity; automation IDs remain stable semantic test/accessibility identity. Private component recipe nodes are not automatically CSS-addressable. Expose declared parts deliberately. A build can diagnose static ID duplication; dynamic instances need runtime validation of the applicable identity rules.

### Transaction and failure model

Compile and validate candidates off the UI thread using CPU data only. On the UI thread, match current generations/state, stage typed contributions and layout/native-capability checks, then publish one logical revision. Parse/type/capability errors keep the prior valid development UI. Do not promise rollback of arbitrary native side effects after allocation/device failure: preserve existing typed failure and teardown contracts.

All state sources remain native/runtime-owned: hover, focus, activation, disabled state, selection, composition, theme and available size. A dynamic styling integration observes those sources, including changes outside app `view`; it does not recreate input state in a frontend. Concrete output equality determines layout/paint/motion work. Stylesheet revision alone cannot trigger perpetual repaint or restart identical transitions.

P12's theme and successful-presentation findings make this boundary a prerequisite, not optional polish. A compiler cannot repair stale native palette/damage logic by emitting more markup. Hot reload must preserve stable keys and text peers where compatible; incompatible structural changes need an explicit restart/remount diagnostic, especially during IME composition.

## HTML/CSS compatibility and compilation

### Standards position

Define a versioned **rust-ui desktop web-authoring profile**, with exact syntax and semantics per feature. Do not call it full HTML/CSS conformance. Source parsing should use standards-aware libraries, with their licenses and costs reviewed, rather than regular-expression translations. HTML tree construction has specified recovery behavior; either apply it in the importer or reject unsupported markup explicitly. [HTML parsing standard](https://html.spec.whatwg.org/multipage/parsing.html)

Default production compilation rejects unsupported declarations/behaviors with source spans and migration actions. An analysis-only mode can inventory them without producing a runnable build. Browser CSS intentionally ignores some invalid declarations; a strict compiler's errors are a documented authoring-profile difference. Explicit fallback chains or compatibility annotations require a declared rule and a report, never silent deletion. [CSS Syntax](https://www.w3.org/TR/css-syntax-3/)

Do not prune a rule because it did not match one captured page. Runtime classes, repeated components and conditional content can make it reachable. Arbitrary runtime stylesheet strings or `innerHTML` are unsupported initially; bounded typed style/state inputs are the extension mechanism.

### Compile-time versus runtime

| Feature | Build-time work | Necessary native runtime work / restriction |
|---|---|---|
| Semantic HTML/attributes | Validate structure; map supported elements, names, relationships and props | Focus/navigation, enabled state, activation, form submission and native text remain semantic contracts. Noninteractive markup cannot accidentally acquire an action. |
| Components | Expand templates/slots; type-check props/events; generate stable key rules | Reconcile conditional/repeated instances using the existing arena; no second component runtime. |
| Selectors/cascade | Parse, calculate specificity/layers/source order, compile match predicates and static winners | Dynamic classes/attributes/ancestry/state require invalidation and matching over affected nodes. Cost is not zero. |
| Custom properties | Fold static substitution, fallbacks and known cycles | Changed scoped values require dependency invalidation; preserve specified invalid-value/cycle rules. |
| Inheritance/currentColor | Compile dependencies and supported inherited-property rules | Recompute descendants on effective inherited changes; submit explicit typed inputs, not implicit inheritance in every core builder. |
| Units/calculations | Type-check dimensions; fold constants | Resolve percentages, font-relative/viewport/container values against their defined bases; reject unresolved or cyclic unsupported constraints. |
| Flex/Grid | Validate constraints and specialize static structure | Content-dependent layout, intrinsic text, available space and resizing still execute native algorithms. |
| Media/containers | Compile conditions and dependency graph | Observe client/container size, preferences and accessible settings through shared environment state. No polling. |
| Typography | Bundle/licence fonts, compile runs/style declarations | Shaping, fallback, bidi, line breaking, intrinsic size and native editor measurement depend on content/platform. |
| Paint/effects | Validate colors, borders, gradients, shadows and capability limits | Render resources, clipping, transforms and ink bounds require typed native support and bounded allocations. |
| Transitions/keyframes | Compile tracks, easing, durations, eligibility and cascade rules | Schedule only active motion; resolve interruption/cancellation, successful presentation and reduced motion. |
| Assets/icons | Resolve local imports, generate manifests, subset only when valid | Decode/upload on demand within byte budgets. Dynamic localization must not lose required glyphs through unsafe subsetting. |
| Event/reactive bindings | Generate typed state access and message factories | Updates, async completion, subscriptions and task cancellation use the native runtime. No hidden JavaScript evaluation. |

Selectors operate on tree relationships and specificity; they cannot generally be replaced by a flat class-to-style lookup. Start with type/class/ID, bounded combinators and supported state/attribute predicates; defer costly relational selectors until invalidation is proven. [Selectors Level 4](https://www.w3.org/TR/selectors-4/)

Cascade is more than specificity: ordering includes origin/importance and, where supported, layers and animation/transition contributions. Implement the declared subset coherently. Imported HTML `style` participates in author cascade; explicit Rust host policy remains a separate higher boundary, followed by native accessibility enforcement. Report that distinction as an adaptation. [CSS Cascade](https://www.w3.org/TR/css-cascade-6/)

Custom properties are not all theme tokens or numeric fields. Preserve scoped token-value substitution and fallback/cycle behavior in the frontend, then lower validated typed values. Map an author variable to a native theme role only through an explicit binding. [CSS Custom Properties](https://www.w3.org/TR/css-variables-1/)

### Layout is a substantive extension

Current `Content`/`Fixed`/`Fill`, Row/Column and min/max constraints are useful native primitives, not evidence of CSS Flexbox/Grid equivalence. Grow/shrink, basis, automatic minimum size and intrinsic sizing can change layout even when a simple demo looks identical. Grid adds track sizing and placement; neither is a parser-only feature. [Flexbox](https://www.w3.org/TR/css-flexbox-1/), [Grid](https://www.w3.org/TR/css-grid-2/)

Propose typed additive native layout modes with declared semantics. Preserve existing Row/Column behavior. When a CSS mode is implemented, both compiler-generated and direct Rust consumers should use its single shared algorithm. Evaluate an existing standalone engine such as Taffy against a small conformance/performance corpus before deciding to implement complex algorithms internally. Its documented independent Block/Flex/Grid support demonstrates a possible separation from a browser, not proof it satisfies rust-ui. Prefer adapting low-level computation to the arena over installing a second authoritative tree. No dependency is selected here. [Taffy upstream](https://github.com/DioxusLabs/taffy)

Initially map `1 CSS px` to `1 Dp` in the profile, preserving platform scaling and accessibility policy. This is not a physical-inch promise. Define font-relative and percentage reference bases before supporting them; do not multiply by Windows DPI in generated code. Content-box versus border-box, default margins, white-space, inline runs, overflow, baseline alignment and hidden/display behavior need explicit tests, not only Row/Flex examples. [CSS values and units](https://www.w3.org/TR/css-values-4/)

Container queries require containment/dependency rules that avoid feedback cycles; “keep laying out until stable” is not an acceptable unbounded algorithm. Begin with size-contained ancestor queries and desktop window breakpoints. Do not claim general container/style-query conformance before it is implemented. [CSS containment](https://www.w3.org/TR/css-contain-3/)

### Visual and native-control capability boundaries

Basic borders, radii, colors, shadows and typography lower to existing typed concepts where their units/semantics match. Add gradients, transforms, group opacity, clipping and more shadow forms only with hit-test, accessibility-geometry, damage and resource contracts. A transform can affect stacking and containing blocks, not just paint coordinates. [CSS Transforms](https://www.w3.org/TR/css-transforms-1/)

RichEdit remains the Windows editable-text authority. Arbitrary transforms, nonrectangular clipping, filters and rich `contenteditable` behavior around native peers cannot be advertised from painted-box support. Diagnose unsupported combinations before native mutation. macOS/Linux must publish the same capability contract or an explicit unsupported result, not quietly substitute an editor with different semantics.

A shared font family reduces variation but does not guarantee identical shaping, rasterization, fallback or editor metrics across platforms. Measure visible strings, advances, baselines, wrapping, IME and selection. The frozen Shadcn target stays unchanged; broader authoring fixtures are separate versioned test inputs.

Motion must define whether it implements a specific CSS behavior or a native adaptation. CSS interrupted transitions have explicit reversal rules; simple linear interpolation from a newly sampled value is not sufficient evidence of conformance. Keyframes also require iteration/fill/composition rules. Compile tracks into the existing scheduling/presentation authority, with no idle frame loop, and reject unsupported animated properties. [CSS Transitions](https://www.w3.org/TR/css-transitions-1/), [CSS Animations](https://www.w3.org/TR/css-animations-1/)

Accessibility is semantic lowering, not copying ARIA strings to a dictionary. Labels, names, descriptions, roles, states, relationships and actions must map to the shared native model; native backends expose them. Test keyboard focus/order and disabled versus read-only behavior separately. Where browser behavior and native policy differ, diagnose/report the adaptation. [HTML accessibility mappings](https://www.w3.org/TR/html-aam-1.0/)

### Initial proof-of-concept profile

Choose one small settings/detail screen with semantic containers, headings/text, labels, buttons and plain single/multiline input; static local assets; basic colors/borders/padding; type/class selectors; `:hover`, `:active`, `:focus-visible`, `:disabled`; one scoped custom property; explicit Light/Dark and one desktop breakpoint. Use only demonstrably supported native geometry at first and report unsupported imported layout rather than approximate it.

Generated output must match a handwritten native equivalent in semantics, geometry and native interactions, remain independent of the compiler at runtime, and reject negative capability fixtures. A static-only prototype is an intermediate compiler proof, not a migration-ready framework. The next milestone must exercise dynamic state and intrinsic sizing before freezing the integration API.

## Migration feasibility

Electron uses Chromium's multi-process model; Tauri hosts web UI in platform WebViews. Their deployment/runtime choices differ, so do not assume equal overhead or present binary size as memory usage. Both approaches retain a web execution environment that this proposal intentionally removes. [Electron process model](https://github.com/electron/electron/blob/main/docs/tutorial/process-model.md), [Tauri process model](https://v2.tauri.app/concept/process-model/)

| Level | Realistic automation | Manual work / exclusions | Evidence needed |
|---|---|---|---|
| 1 — static UI | Parse source markup/CSS/assets; normalize supported selectors/units; emit typed structure/style; preserve IDs and accessible relationships | Unsupported layout/effects, document-flow edge cases, dynamic markup and font differences | Original versus native at named desktop sizes/themes, semantic/part geometry, accessibility names; no arbitrary JS executed |
| 2 — stateful UI | Recognized component patterns, typed props/events, conditional/keyed lists, form bindings, explicit navigation/menu/dialog adapters | Framework lifecycle differences, portals, validation/default actions, focus traps, input composition and third-party component assumptions | Interaction traces, state outcomes, focus/IME/accessibility and visual transitions |
| 3 — application logic | Extract pure typed functions/data models; generate reviewed Rust scaffolding; retain separable existing Rust business logic from Tauri | JS coercion/prototypes/exceptions, async ordering, library behavior, Node/browser APIs, storage/auth/network integrations | Behavioral fixtures, cancellation/error tests, schema/API compatibility, security review; no claim of universal equivalence |
| 4 — advanced compatibility | Analyze dependencies and offer explicit native replacement recipes | Canvas/WebGL/WebGPU, arbitrary rich editors, embedded websites, dynamic code, complex DOM measurement, plugins and full browser libraries | Per-capability milestone and real application proof; unsupported features remain blockers to that migration, not silently dropped |

TypeScript's annotations are erased in its normal JavaScript output; they do not establish Rust ownership or preserve all dynamic language behavior. Do not treat a type-checked TS program as mechanically convertible Rust. [TypeScript handbook](https://www.typescriptlang.org/docs/handbook/2/basic-types.html)

React DOM targets browser DOM nodes, while Svelte compilation emits JavaScript. Their source/template structure can inform adapters, but their compiled output cannot simply be linked into a no-JS native executable. Build source adapters selectively; do not promise drop-in third-party framework runtime compatibility. [React createRoot](https://react.dev/reference/react-dom/client/createRoot), [Svelte compiler](https://svelte.dev/docs/svelte/svelte-compiler)

Slint's build tool demonstrates declarative UI compilation to Rust code. It supports the feasibility of the authoring boundary, not HTML/CSS compatibility or this memory target. [Slint build API](https://docs.slint.dev/latest/docs/rust/slint_build/)

### A predictable migration pipeline

1. Inventory exact source revision, framework/build configuration, assets/licenses, screen routes and required desktop behavior. Analyze without executing arbitrary dependency scripts by default.
2. Report feature usage by source location and route: supported-exact, supported-native-adaptation, rewrite-required, unsupported or unknown. Dynamic code that cannot be analyzed remains unknown.
3. Convert supported structure/style and recognized component patterns. Generate explicit typed bindings; keep business logic changes separate from geometry changes.
4. Use AI for bounded translations/scaffolding with source maps, diffs and behavioral fixtures. Human review owns unresolved semantics; AI confidence is not proof.
5. Compare representative real application workflows, not only isolated widgets. Record unchanged source, rewritten sections, unsupported features and manual effort without inventing an overall compatibility percentage.
6. Package a native build and measure its complete process/resource family under the same workload as the source application. Keep runtime-network/data requirements explicit; no hidden WebView fallback.

The first real application should be a small existing, redistributable desktop settings/local-data utility with forms, lists, filtering, navigation, dialogs and asynchronous I/O. Select its repository and acceptance tasks before implementing missing features, so the benchmark does not become a handpicked synthetic success. Follow with a notes/task workspace to test larger data and multiline editing. An IDE, browser or rich-document editor is a poor first migration because it concentrates unsupported browser/editor dependencies.

Concrete candidates for Founder selection, not porting commitments:

| Existing application | Proposed initial workflow | What it would expose |
|---|---|---|
| [Pomatez](https://github.com/zidoro/pomatez) | Task entry, timer start/pause, settings, theme and compact window; then persistence/tray/notifications | A real desktop utility with state, motion and lifecycle. Its [upstream changelog](https://github.com/zidoro/pomatez/blob/master/CHANGELOG.md) records both Electron and Tauri work, making source/runtime boundaries worth inspecting. Timer-active and timer-stopped idle must be measured separately. |
| [Pot](https://github.com/pot-app/pot-desktop/blob/master/README_EN.md) | Settings plus input translation against a controlled service fixture | A Tauri application exercising native text, clipboard and async requests. OCR, speech and external integrations are separate declared capabilities, not silently counted as migrated. |

Prefer Pomatez for initial source analysis; choose and pin a source revision, license inventory and feature checklist before implementation. These proposed slices do not certify a complete application migration. A later full-app claim must account for every required desktop feature or explicitly name omissions. Neither application was built, profiled or converted in this review.

Packaging must eventually cover multiple windows, menus/tray, files/dialogs, permissions, logging, signing/installers and update choices. Keep these optional native application capabilities separate from styling and the retained UI core. A no-browser process is not a security sandbox: plugins and untrusted content need explicit trust boundaries.

## Memory feasibility and accounting

### The target and what is actually known

Use **100,000,000 bytes total attributable application memory** as the proposed reference ceiling. Include application data, all required helper processes, native/graphics resources and transient UI work in the named workload. Also target **<=40 MB** for the empty native shell in the small-window workload below, leaving meaningful application headroom. Both are proposed acceptance targets, not measured baselines.

The retained composer evidence reports roughly 32 MiB working set on an older build, with explicit provenance limitations. Working set alone excludes commitments and attribution required here; it proves neither current Gallery memory nor the 100 MB product target. The P12 audit lists current memory/latency metrics as unmeasured. [Historical receipt account](../benchmark/results/windows-native-composer-spike-v0.1/README.md), [P12 audit](https://github.com/rceman/rust-ui/blob/a8a0cf0fcb5e66e508dd9a2c2234efac56529c1a/docs/reviews/ASTRA_SYSTEM_WORKFLOW_AUDIT_20261031.md)

Removing a browser/VM removes mandatory categories of work, but does not guarantee a particular saving against a named Electron or Tauri application. Compare identical data/features and include all process families before making a competitive claim.

### Accounting policy

Maintain three views, not one misleading counter:

- **Attributed total for the workload:** application-owned/accounted host memory plus an explicit share of clean shared mappings and nonoverlapping graphics/native allocations. Count app-owned child processes and services. Do not subtract the application data to obtain a headline pass.
- **OS counters:** private commitment/footprint, resident set, shared/proportional pages, compression/swap, peak and graphics usage as separately named fields. Preserve units and metric definitions; they are not numerically interchangeable across OSes.
- **Engineering breakdown:** runtime/arena, compiled authoring, text/fonts, decoded assets, surfaces, app data, worker stacks/queues and optional features. Use allocation/resource instrumentation to explain growth, not replace OS accounting.

On unified memory, a mapped graphics buffer may already contribute to host accounting. Count its physical backing once; do not blindly add GPU counters to RSS. Where ownership/overlap cannot be established, publish lower/upper bounds and the unknown category. A conservative upper bound under 100 MB can pass; an unknown bound spanning 100 MB cannot. Do not treat unobservable OS/GPU costs as zero.

Charge directly attributable application resources in shared native services where measurable. Publish unrelated whole-system/compositor baseline separately rather than charging the entire desktop to one app. Shared library/code pages should have both a fair-share estimate and a conservative full-resident upper bound when a cross-process share is unavailable. This avoids hiding costs while retaining reproducibility.

| Platform | Primary collection and interpretation | Required caveat |
|---|---|---|
| Windows | GetProcessMemoryInfo: PrivateUsage/private commit and working-set/peak fields for the process family; VM/working-set inspection for clean sharing; graphics process usage and owned surface inventory | PrivateUsage is committed private memory, not simply resident heap. Treat it as a conservative host budget charge; do not add private working set again. Add only nonoverlapping shared/graphics costs. |
| macOS | Instruments/VM Tracker and process footprint/regions, resident data, compression and Metal/native resource tracing | Footprint differs from RSS and includes compressed/swapped dirty accounting. Reconcile shared/graphics ownership before aggregation. |
| Linux | `/proc/<pid>/smaps_rollup` PSS and SwapPss for the process family, RSS/private diagnostics, native graphics resource accounting | PSS apportions shared mappings; it is not Windows commit. Reserved virtual address space is not resident usage. Record driver/GPU attribution limits. |

These metric distinctions follow platform documentation: [Windows process counters](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex), [DXGI process usage](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-queryvideomemoryinfo), [Apple memory analysis](https://developer.apple.com/documentation/xcode/analyzing-the-memory-usage-of-your-metal-app), [Linux proc accounting](https://www.kernel.org/doc/html/latest/filesystems/proc.html).

### Workload classes to approve before claiming success

| Class | Defined application content | Target / purpose |
|---|---|---|
| Empty shell | One 640x480 Dp window at scale 1 and 1.25, basic text/button, no devtools, no background app data | <=40 MB total target; baseline startup and idle |
| Settings utility | One 1280x800 Dp window; <=1,000 live semantic nodes; <=12 mounted editors; six settings pages; one open menu/dialog; <=2 MB UTF-8 app text and <=4 MB decoded images | <=100 MB including normal interaction peaks; first representative class |
| Local-data workspace | One 1280x800 Dp window; <=2,000 live nodes; 10,000 bounded records with <=20 MB total app data/index/cache allocation; <=8 mounted editors; <=8 MB decoded images; scrolling/filtering | <=100 MB target; requires bounded/virtualized visible realization where needed |
| Display/capability stress | Same applications at scale 2, 1440x1000 or larger, second window, extra language fonts, longer documents or media | Record separately. Pursue <=100 MB for each adopted representative envelope; no automatic pass transfer or silent ceiling increase. |

These are workload contracts, not arbitrary application limits in public APIs. Larger inputs must remain correct and resource-bounded, but do not inherit the memory claim. Do not virtualize solely to pass by dropping accessible state or retaining unbounded hidden peers. Source and migrated applications must load equivalent data and perform equivalent tasks.

Proposed allocation envelopes in decimal MB for the two representative application classes:

| Attributed category | Windows | macOS | Linux |
|---|---:|---:|---:|
| Fixed runtime/native service overhead and charged shared mappings | 25 | 20 | 20 |
| Arena, layout, compiled style/state metadata | 8 | 8 | 10 |
| Text/font/decoded-asset caches | 12 | 12 | 10 |
| Nonoverlapping render targets/surfaces | 20 | 25 | 25 |
| Application data, model and application caches | 20 | 20 | 20 |
| Temporary work / event / motion peaks | 10 | 10 | 10 |
| Unallocated headroom | 5 | 5 | 5 |
| **Total ceiling** | **100** | **100** | **100** |

These are initial design allocations, **not measured platform costs or approved promises**. Charges must be exclusive; text-peer backing surfaces belong to surfaces, not both text and graphics. Rebalance categories only with a documented measurement, keeping the total ambition unchanged. If a workload exceeds the ceiling, report failure and optimize or explicitly revisit its product classification; do not redefine the metric after measuring it.

### Why graphics and content can defeat the target

A 1280x800 Dp surface at scale 1.25 has 1600x1000 pixels: one tightly packed 32-bit image is 6.4 MB; two are 12.8 MB before alignment/driver overhead. A 1440x1000 Dp surface at scale 2 needs 23.04 MB per such image, or 46.08 MB for two. Two 3840x2160 images alone occupy approximately 66.36 MB. These are arithmetic examples, not claims about current buffering. Offscreen effects, WGC copies and peer bitmaps add more.

Therefore one full-window texture per component/effect is incompatible with the target. Bound caches by bytes as well as entries; share immutable font/style assets; cap decoded image residency; size native peers to actual visible needs; account for transient old/new surfaces during resize; and release generation-owned resources promptly. Hardware acceleration must be measured for both latency and allocation cost. Switching to software is not automatically cheaper and is not proposed as a hidden fallback.

A useful first compiler test compares hand-authored and generated native versions of the same utility. The difference isolates frontend/runtime metadata overhead; the total still governs product acceptance. Do not confuse that delta with the whole application's footprint.

### Minimal repeatable measurement protocol

For each adopted platform/workload, freeze source, compiler/profile version, build flags, asset hashes, OS/hardware/driver, display scale and data fixtures. Use optimized builds without devtools for product numbers; measure instrumentation overhead separately. Developer CLI/capture processes are excluded only because they are not shipped, and their cost is disclosed in tooling measurements.

Run at least three clean process launches, distinguishing warm file cache from true cold conditions. Record startup-to-first-successful-presentation, first-input responsiveness, a fixed navigation/edit/scroll/theme/resize sequence, 30 seconds idle and bounded repeated open/close cycles. Keep the complete app process family. Sample memory externally at a stated interval (for example 100 ms) plus OS high-water/resource events; sampled maxima alone can miss transient peaks. Instrument known allocation transitions to close that uncertainty.

Report startup, steady, interaction and teardown memory, observed peaks, category bounds, run spread, idle CPU/wakeups, layout/paint/present counts and input-to-present p50/p95/p99 with sample counts. Do not infer presentation latency from CLI wall time. No continuous idle render scheduling is permitted; legitimate caret/timer work is separately identified. Freeze latency thresholds only after baseline and product acceptance, not from invented numbers in this proposal.

An optional capability such as database search, media or a second window gets its own incremental and total result. A model-loading/LLM/video workload is not evidence for or against the bounded settings application's claim unless that capability is part of its declared content.

## Roadmap ordered by evidence and risk

These are dependency stages, not delivery estimates, a new gate taxonomy, or concurrent implementation authorization.

| Stage | Scope | Measurable exit / dependency |
|---|---|---|
| P12 closure | Existing native rendering/visual fixes and evidence integrity | Independent Opus visual and Astra systems acceptance of the actual frozen candidate; first-presentation theme/damage and material geometry/motion findings closed or explicitly authorized. Current Workers continue independently. |
| Native stabilization | Application lifecycle/text/accessibility, typed layout/style seams, memory accounting and packaging smoke | One small real native utility with bounded data, release baseline, idle/teardown proof; no current P12 PASS inherited without identity. Preserve known separate test-isolation issue as its own task. |
| Compiler proof | Initial static HTML/CSS/component profile lowering to existing typed APIs | Reproducible generated native output; native/Rust equivalent semantic/geometry tests; unsupported-feature diagnostics; no parser/Node/browser deployment dependency; frontend overhead measured. |
| Stateful authoring | Props/events/slots, scoped variables, conditional/keyed lists, forms and dynamic state predicates | Native focus/IME/selection survives updates; simultaneous pseudo-states correct; failed development update retains valid UI; source maps explain winners and adaptations. |
| Layout/profile expansion | Needed relative sizing, intrinsic text/inline behavior, Flexbox then selected Grid, desktop/container queries and motion/effects | Explicit feature ledger and targeted standards-derived tests; real migration slice validates resizing, text and budgets. Choose algorithm reuse by measured tradeoff, not familiarity. |
| First complete migration | Existing small desktop utility selected earlier; migrate UI plus required application logic and packaging | Same named user tasks, data and supported states; counted manual changes/unsupported uses; native accessibility and <=100 MB target measured. No browser fallback. |
| macOS realization | Shared contracts implemented with native services; small feasibility probe may begin after stateful contracts stabilize and before profile/API freeze | Reuse the same compiler output/app semantics; real native text/IME/accessibility, DPI/backing scale, packaging and memory proof. No Windows-only assumptions in shared authoring. |
| Linux realization | Select supported desktop/window-system/accessibility/text environment explicitly | Same semantic conformance suite and real app acceptance; record supported environment, native input/accessibility and resource results. Do not advertise all Linux combinations without proof. |
| General framework readiness | More than one independent app/team, optional desktop integrations, stable profile/API policy, migration tooling and packaging | Published supported-feature/platform matrix, compatibility upgrade policy, repeated performance/memory results and independent reviews. Gallery completion alone is insufficient. |

Memory optimization is continuous from stabilization, not postponed until all features ship. First measure the current backend, then enforce budgets while expanding compiler/runtime capabilities. A small second-platform probe before freezing public compiler assumptions reduces expensive redesign; it does not interrupt P12 or require implementing two whole backends at once. Default roadmap preference is macOS before Linux, subject to owner priorities and available native test environments.

## Technical risks and bounded responses

| Risk | Why it matters | Bounded response / decision |
|---|---|---|
| Compatibility expands into a browser engine | Arbitrary DOM/JS/web API support defeats footprint and native semantics | Versioned profile, explicit unsupported diagnostics and migration adapters; no silent browser fallback |
| Tiny subset becomes a permanent ceiling | Static demo success fails real desktop migrations | Select real application early; add required layout/semantic capabilities deliberately |
| Dynamic cascade invalidation | Ancestor/class/variable changes can cause broad work | Compiled dependency indexes over the arena, bounded invalidation and output equality; profile worst cases |
| Native text versus CSS effects | Editing/IME/accessibility can diverge from painted chrome | Capability checks and native acceptance; unsupported combinations diagnosed, not flattened to custom editing |
| JS/TS conversion claims | Type erasure and dynamic/async behavior prevent general equivalence | Source analysis, explicit rewrites, behavioral fixtures and human-reviewed AI assistance |
| Memory accounting optimism | RSS-only or omitted GPU/child costs can manufacture a pass | Process-family and graphics accounting with bounds; separate measured data from target allocations |
| Cross-platform metric drift | Same font/markup does not guarantee same editor/text geometry | Platform conformance and real native application tests before cross-platform claims |
| Generated-code bloat | Massive Rust expansion increases compile time/binary size and developer friction | Start inspectable; measure output and build cost; share typed static data before inventing a VM |
| Hot reload damages identity | Style/structure updates can replace active text peers | Revision/generation fencing, recoverable validation and explicit restart boundaries |
| App framework breadth | Packaging, storage, permissions and lifecycle can overwhelm UI work | Optional capability modules driven by first real applications, not a general plugin framework |

## Verification and engineering discipline

Reuse the existing native authority and devctl. Compiler tests should cover parsing/lowering/diagnostics and targeted standards behavior; native integration should cover the resulting actual application. Use selected licensed standards tests or equivalent focused cases, not a full browser conformance suite. Browser rendering can be a pinned development oracle for the declared subset; it is neither a shipped dependency nor proof of native editing behavior.

Use the existing audit's focused inner loop, paired capture reuse and frozen acceptance proposal when authorized. No full screenshot sweep after each compiler edit. Preserve the independent visual/systems review split. P12 evidence and frozen reference are inputs, not editable implementation conveniences.

No new CI, backend, parser, runtime, tests or dependency is added by this branch. Documentation verification checks identity, links, scope and repository file-budget policy only. Universal Gates 1–20 still apply to future implementation milestones; no additional numbered gate system is proposed.

## Remaining Founder / Planner decisions

The strategic destination and permission for future native capabilities are approved in principle; they do not need to be rediscovered in later reviews. The following remain pending:

1. Founder authorization to publish/merge these documents as durable strategic authority, followed by a narrowly scoped reconciliation of conflicting older exclusion wording.
2. The concrete versioned compatibility profile, supported semantics, diagnostics/adaptations and logic-language/binding scope. Universal browser or TS-to-Rust equivalence is not promised.
3. The proposed 100,000,000-byte accounting definition, workload envelopes, 40 MB shell target and per-platform allocations as acceptance targets. No measurement or achievement is claimed.
4. The first migration repository/revision and success tasks; Pomatez remains preferred for analysis, not an authorized port.
5. Detailed milestone sequencing and native validation environments. Windows-first, bounded macOS validation and subsequent Linux work remain the proposed order.
6. Explicit authorization of future bounded implementation tasks after their prerequisites. Frozen reference and independent review ownership remain unchanged.

**Immediate action:** route the strategic publication decision to Founder while P12 continues. Subsequent native stabilization, measurement and compiler experiments require their own authorized tasks; this record dispatches none.

## Research provenance

Primary source links above were consulted on 2026-10-31. Living standards, drafts and tool documentation are not immutable compiler specifications. The first implemented profile must record the precise specification editions/sections, test corpus revisions and dependency versions it implements. The links establish the technical distinctions used in this analysis, not a claim that all listed features are implemented or that another toolkit's performance transfers to rust-ui.

**Decision status:** `STRATEGIC_DIRECTION_APPROVED_IN_PRINCIPLE` — technical targets provisional; Founder publication/merge authorization pending.

**Next owner:** rust-ui Planner

**Merge authority:** NONE
