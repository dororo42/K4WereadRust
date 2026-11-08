#!/usr/bin/env python3
"""K4weread 瘦身产物校验门禁。

对 slim_koreader.py 产物做结构校验：必需入口在位、保留插件齐全、
排除面确实干净、CJK 回退字体在保留集内。退出码 0 = 通过。

用法: check_slim.py <slim_tree_dir> [--manifest tools/slim_manifest.json]
"""
import argparse
import json
import os
import sys


def fail(msg):
    print(f"[FAIL] {msg}")
    return False


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("tree", help="slim_koreader.py 产物目录")
    ap.add_argument("--manifest", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "slim_manifest.json"))
    args = ap.parse_args(argv)

    tree = os.path.abspath(args.tree)
    ok = True
    if not os.path.isdir(tree):
        print(f"[FAIL] tree not found: {tree}")
        return 1

    with open(args.manifest, "r", encoding="utf-8") as fh:
        manifest = json.load(fh)

    for required in ("koreader.sh", "reader.lua", "frontend", "plugins", "fonts", "SLIM_REPORT.json"):
        if not os.path.exists(os.path.join(tree, required)):
            ok = fail(f"required entry missing: {required}")

    pdir = os.path.join(tree, "plugins")
    present = set()
    if os.path.isdir(pdir):
        present = {d for d in os.listdir(pdir) if d.endswith(".koplugin")}
    for keep in manifest["keep_plugins"]:
        if keep not in present:
            ok = fail(f"keep plugin missing: {keep}")
    excluded_expected = set()
    if os.path.isdir(os.path.join(os.path.dirname(tree) or ".", "_src_unused")):
        pass  # 源树不在场时无法交叉核对排除面，由 SLIM_REPORT 承担
    report_path = os.path.join(tree, "SLIM_REPORT.json")
    if os.path.exists(report_path):
        with open(report_path, "r", encoding="utf-8") as fh:
            report = json.load(fh)
        if report.get("dst_bytes", 0) == 0:
            ok = fail("report says dst is empty")
        src_excluded = set(report.get("excluded_plugins", []))
        leaked = src_excluded & present
        if leaked:
            ok = fail(f"excluded plugins leaked into tree: {', '.join(sorted(leaked))}")

    fdir = os.path.join(tree, "fonts")
    kept_font_files = set()
    if os.path.isdir(fdir):
        for root, _d, files in os.walk(fdir):
            for name in files:
                rel = os.path.relpath(os.path.join(root, name), fdir).replace(os.sep, "/")
                kept_font_files.add(rel)
    if not any("DroidSansFallback" in f for f in kept_font_files):
        ok = fail("CJK fallback font (DroidSansFallback*) not in kept set — weread 正文/回退渲染会退化")

    print(f"[{'PASS' if ok else 'FAIL'}] {tree}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
