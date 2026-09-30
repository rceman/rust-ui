# Proven Context from Mascot

This document provides **requirements/evidence context only** for rust-ui architecture work.

Astra should not spend the architecture task reverse-engineering or migrating the Mascot implementation.

## Source reference

Current proven native UI/component-gallery work exists in:

- repository: `rceman/mascot`
- branch: `agent/native-ui-component-gallery-v0.1-swe2`
- reference HEAD at concept kickoff: `50b2284c1f4e77e751fde75fed78651a4aa34bc7`

The branch may continue receiving small visual corrections. Those implementation details are not architectural authority for rust-ui.

## What has already been proven on Windows

The existing Mascot prototype demonstrates that the following direction is viable:

- Rust native desktop implementation;
- Win32/Direct2D/DirectWrite/DirectComposition rendering;
- native windowless RichEdit for editable composer text;
- transparent borderless composition;
- Lucide icons converted to compact native path data;
- no runtime SVG/XML parser;
- shadcn-first visual styling;
- light/dark;
- DPI validation at 100/125/150/200%;
- event-driven static rendering;
- zero continuous idle frame loop;
- deterministic component gallery/evidence tooling.

## Current generic component vocabulary

Current implemented/generic concepts include:

- Surface/Bubble
- Typography/Label
- Icon
- IconButton
- Button
- Tooltip
- Native TextInput/Composer
- Separator
- Response/Content Surface
- Badge/Status Pill

Likely future components, not currently required:

- ScrollArea
- Popover
- Dropdown/MenuItem
- Select
- Switch
- Checkbox
- RadioGroup
- Dialog
- Settings row
- Progress/activity indicator

Do not reproduce the whole shadcn catalog.

## Design constraints worth preserving

- shadcn-first component visual grammar;
- Lucide as canonical icon source;
- one coherent light/dark token system;
- subtle shadcn-like motion where direct analogues exist;
- no decorative loader palette;
- no colour-only status semantics;
- native text controls for real editing/IME/accessibility behavior;
- no browser runtime;
- no generic UI framework unless present needs prove it necessary.

## Important distinction

The existing Mascot code is a **prototype/proof source**, not the public API design for rust-ui.

Astra is specifically asked to improve the architecture rather than preserve accidental implementation structure.

Conceptually:

```text
Mascot existing UI
        |
        | evidence / requirements
        v
rust-ui architecture design

NOT

Mascot existing UI
        |
        | mechanical code extraction
        v
rust-ui
```

## Product boundary

Future Mascot should consume rust-ui.

rust-ui should provide generic:

- components;
- layout;
- styling;
- state/event mechanism;
- motion;
- native backends;
- custom render/image extension points.

Mascot should own:

- mascot character animation;
- rig/runtime;
- agent/chat semantics;
- provider/session logic;
- product composition and behaviors.
