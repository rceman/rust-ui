#!/usr/bin/env python3
"""Bounds/overflow QA over a devctl snapshot-layout document.

Usage: python3 tools/bounds_check.py <snapshot-layout.json> [--report out.json]

Rules (evidence tooling, not production semantics):
  - every VISIBLE node must lie within client bounds (eps 0.6dp);
    `popup` kind nodes are allowed to escape page/scroll clips but must
    still lie within the client
  - interactive leaves (button/combobox/switch/checkbox/radio/option/
    menuitem*/tab/navigation-item/input) must not be zero-size
  - popup descendants must lie within the popup's own rect
Exit 0 clean / 1 violations.
"""
import json
import sys

EPS = 0.6
INTERACTIVE = {
    "button", "combobox", "switch", "checkbox", "radio", "option",
    "menuitem", "menuitemcheckbox", "menuitemradio", "menu-button",
    "tab", "navigation-item", "input", "toggle",
}


def collect(node, path, in_popup, out):
    aid = node.get("automation_id") or f"<{node.get('kind', '?')}>"
    here = f"{path}/{aid}"
    popup = in_popup or node.get("kind") == "popup"
    out.append((node, here, in_popup))
    for c in node.get("children") or []:
        collect(c, here, popup, out)


def main() -> int:
    doc = json.load(open(sys.argv[1]))
    res = doc.get("result", doc)
    cw = res["client_dp"]["width"]
    ch = res["client_dp"]["height"]
    nodes = []
    collect(res["root"], "", False, nodes)
    viol = []
    # popup subtree containment
    def subtree_bounds(node, popup_rect, path):
        aid = node.get("automation_id") or f"<{node.get('kind', '?')}>"
        r = node.get("rect_dp")
        if node.get("visible") and r:
            if r["x"] < popup_rect["x"] - EPS or r["y"] < popup_rect["y"] - EPS or (
                r["x"] + r["width"] > popup_rect["x"] + popup_rect["width"] + EPS
            ) or (r["y"] + r["height"] > popup_rect["y"] + popup_rect["height"] + EPS):
                viol.append(
                    f"POPUP_CHILD_OUTSIDE {path}/{aid} rect={r} popup={popup_rect}"
                )
        for c in node.get("children") or []:
            subtree_bounds(c, popup_rect, f"{path}/{aid}")

    for node, path, _in_popup in nodes:
        r = node.get("rect_dp")
        if not r:
            continue
        vis = node.get("visible", True)
        if not vis:
            continue
        right = r["x"] + r["width"]
        bottom = r["y"] + r["height"]
        if (
            r["x"] < -EPS
            or r["y"] < -EPS
            or right > cw + EPS
            or bottom > ch + EPS
        ):
            viol.append(
                f"OUTSIDE_CLIENT {path} rect=({r['x']:.1f},{r['y']:.1f},{r['width']:.1f},{r['height']:.1f}) client=({cw}x{ch})"
            )
        if node.get("kind") in INTERACTIVE and (r["width"] < 1 or r["height"] < 1):
            viol.append(f"ZERO_SIZE {path} rect={r}")
        if node.get("kind") == "popup":
            for c in node.get("children") or []:
                subtree_bounds(c, r, path)

    report = {"client": [cw, ch], "nodes": len(nodes), "violations": viol}
    txt = json.dumps(report, indent=1)
    if len(sys.argv) > 3 and sys.argv[2] == "--report":
        open(sys.argv[3], "w").write(txt + "\n")
    print(txt if viol else txt)
    print(f"bounds_check: {len(viol)} violations / {len(nodes)} nodes")
    return 1 if viol else 0


if __name__ == "__main__":
    sys.exit(main())
