#!/usr/bin/env python3
"""K4weread 瘦身产物校验门禁。

对 slim_koreader.py 产物做结构校验：必需入口在位、保留插件齐全、
排除面确实干净、CJK 回退字体在保留集内、产物与 manifest 版本一致。
退出码 0 = 通过。

用法: check_slim.py <slim_tree_dir> [--manifest tools/slim_manifest.json]
"""
import argparse
import hashlib
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

    with open(args.manifest, "rb") as fh:
        manifest_sha256 = hashlib.sha256(fh.read()).hexdigest()
    with open(args.manifest, "r", encoding="utf-8") as fh:
        manifest = json.load(fh)

    for required in ("koreader.sh", "reader.lua", "frontend", "plugins", "fonts", "SLIM_REPORT.json"):
        if not os.path.exists(os.path.join(tree, required)):
            ok = fail(f"required entry missing: {required}")

    pdir = os.path.join(tree, "plugins")
    present = set()
    if os.path.isdir(pdir):
        present = {d for d in os.listdir(pdir) if d.endswith(".koplugin")}
    keep_plugins = set(manifest["keep_plugins"])
    for keep in sorted(keep_plugins):
        if keep not in present or not os.path.isdir(os.path.join(pdir, keep)):
            ok = fail(f"keep plugin missing: {keep}")
    for extra in sorted(present - keep_plugins):
        ok = fail(f"unexpected plugin: {extra}")
    report_path = os.path.join(tree, "SLIM_REPORT.json")
    if os.path.exists(report_path):
        with open(report_path, "r", encoding="utf-8") as fh:
            report = json.load(fh)
        if report.get("dst_bytes", 0) == 0:
            ok = fail("report says dst is empty")
        # 产物必须由本次 --manifest 生成，否则「门禁通过」不能证明清单版本一致。
        if report.get("manifest_sha256") != manifest_sha256:
            ok = fail("SLIM_REPORT.json was produced from a different manifest")

    fdir = os.path.join(tree, "fonts")
    kept_font_files = set()
    if os.path.isdir(fdir):
        for root, _d, files in os.walk(fdir):
            for name in files:
                rel = os.path.relpath(os.path.join(root, name), fdir).replace(os.sep, "/")
                kept_font_files.add(rel)
    keep_fonts = set(manifest["keep_fonts"])
    for missing in sorted(keep_fonts - kept_font_files):
        ok = fail(f"keep font missing: {missing}")
    for extra in sorted(kept_font_files - keep_fonts):
        ok = fail(f"unexpected font: {extra}")
    # 必须是真正的字体文件；许可证/说明文件即使含 CJK 字样也不算回退载体。
    cjk_fonts = [
        f for f in kept_font_files
        if ("DroidSansFallback" in f or "NotoSansCJK" in f)
        and f.lower().endswith((".ttf", ".otf", ".ttc"))
    ]
    if not cjk_fonts:
        ok = fail("CJK fallback font (DroidSansFallback*/NotoSansCJK*) not in kept set — weread 正文/回退渲染会退化")

    print(f"[{'PASS' if ok else 'FAIL'}] {tree}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
