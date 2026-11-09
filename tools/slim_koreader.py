#!/usr/bin/env python3
"""K4weread Phase 1 配置级瘦身（S1）：从 KOReader 树生成精简副本。

纯打包层：不修改源树、不修改任何代码文件内容。裁剪面 = 插件（保留清单外
整目录排除）+ 字体（保留清单外整文件排除），对应评估报告 §二 S1 / §3.1。
引擎与 native 库删除属 S2（Phase 2 fork 裁剪），本工具不做。

用法:
  slim_koreader.py <src_koreader_tree> <dst_dir> [--manifest tools/slim_manifest.json]
  slim_koreader.py <src> --list-plugins          # 枚举源树插件与体积（生成清单用）
  slim_koreader.py <src> --list-fonts            # 枚举源树字体与体积（生成清单用）

属性: 幂等（重跑产物一致）；永不写源树；目标目录已存在时拒绝（--force 覆盖）。
"""
import argparse
import hashlib
import json
import os
import shutil
import sys

PLUGINS_SUBDIR = os.path.join("plugins")
FONTS_SUBDIR = os.path.join("fonts")


def dir_size(path):
    total = 0
    for root, _dirs, files in os.walk(path):
        for name in files:
            fp = os.path.join(root, name)
            if not os.path.islink(fp):
                total += os.path.getsize(fp)
    return total


def human(n):
    for unit in ("B", "K", "M", "G"):
        if n < 1024 or unit == "G":
            return f"{n:.1f}{unit}" if unit != "B" else f"{n}B"
        n /= 1024.0
    return f"{n:.1f}G"


def plugins_in(src):
    pdir = os.path.join(src, PLUGINS_SUBDIR)
    if not os.path.isdir(pdir):
        return []
    return sorted(
        d for d in os.listdir(pdir)
        if d.endswith(".koplugin") and os.path.isdir(os.path.join(pdir, d))
    )


def fonts_in(src):
    fdir = os.path.join(src, FONTS_SUBDIR)
    if not os.path.isdir(fdir):
        return []
    out = []
    for root, _dirs, files in os.walk(fdir):
        for name in sorted(files):
            rel = os.path.relpath(os.path.join(root, name), fdir)
            out.append(rel.replace(os.sep, "/"))
    return sorted(out)


def copy_tree(src, dst, skip_dirs=(), skip_files=()):
    """递归复制，跳过匹配的相对目录/文件；被跳过文件的父目录不残留在产物里
    （源树中本来就空的目录仍保留）。返回跳过统计 (dirs, files)。"""
    skipped_dirs = skipped_files = 0
    os.makedirs(dst, exist_ok=True)
    for root, dirs, files in os.walk(src):
        rel_root = os.path.relpath(root, src)
        for d in list(dirs):
            rel = os.path.normpath(os.path.join(rel_root, d)).replace(os.sep, "/")
            if rel in skip_dirs:
                dirs.remove(d)
                skipped_dirs += 1
        dst_root = os.path.join(dst, rel_root) if rel_root != "." else dst
        if not dirs and not files and not os.path.exists(dst_root):
            os.makedirs(dst_root, exist_ok=True)
        for f in files:
            rel = os.path.normpath(os.path.join(rel_root, f)).replace(os.sep, "/")
            if rel == "SLIM_REPORT.json":
                continue
            if rel in skip_files:
                skipped_files += 1
                continue
            s = os.path.join(root, f)
            d = os.path.join(dst_root, f)
            os.makedirs(os.path.dirname(d), exist_ok=True)
            if os.path.islink(s):
                link = os.readlink(s)
                if os.path.lexists(d):
                    os.remove(d)
                os.symlink(link, d)
            else:
                shutil.copy2(s, d)
    return skipped_dirs, skipped_files


def load_manifest(path):
    with open(path, "r", encoding="utf-8") as fh:
        data = json.load(fh)
    if not isinstance(data, dict):
        raise SystemExit(f"manifest invalid: expected an object ({path})")
    for key in ("keep_plugins", "keep_fonts"):
        entries = data.get(key)
        if not isinstance(entries, list) or not all(isinstance(entry, str) for entry in entries):
            raise SystemExit(f"manifest invalid: {key} must be a list of strings ({path})")
    return data


def list_mode(src, what):
    if what == "plugins":
        pdir = os.path.join(src, PLUGINS_SUBDIR)
        for name in plugins_in(src):
            print(f"{human(dir_size(os.path.join(pdir, name)))}\t{name}")
    else:
        fdir = os.path.join(src, FONTS_SUBDIR)
        for name in fonts_in(src):
            print(f"{human(os.path.getsize(os.path.join(fdir, name)))}\t{name}")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("src", help="源 KOReader 树（只读）")
    ap.add_argument("dst", nargs="?", help="输出精简树目录")
    ap.add_argument("--manifest", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "slim_manifest.json"))
    ap.add_argument("--force", action="store_true", help="允许覆盖已存在的目标目录")
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument("--list-plugins", action="store_true")
    mode.add_argument("--list-fonts", action="store_true")
    args = ap.parse_args(argv)

    src = os.path.abspath(args.src)
    if not os.path.isdir(src):
        raise SystemExit(f"source tree not found: {src}")

    if args.list_plugins:
        return list_mode(src, "plugins")
    if args.list_fonts:
        return list_mode(src, "fonts")
    if not args.dst:
        ap.error("dst required unless --list-plugins/--list-fonts")

    dst = os.path.abspath(args.dst)
    real_src, real_dst = os.path.realpath(src), os.path.realpath(dst)
    if os.path.commonpath((real_src, real_dst)) in (real_src, real_dst):
        raise SystemExit(f"source and destination trees overlap: {src} -> {dst}")
    if os.path.exists(dst) and not args.force:
        raise SystemExit(f"dst exists (refusing to overwrite; idempotency guard): {dst}")

    manifest = load_manifest(args.manifest)
    keep_plugins = set(manifest["keep_plugins"])
    keep_fonts = set(manifest["keep_fonts"])

    present_plugins = set(plugins_in(src))
    missing_keep = sorted(keep_plugins - present_plugins)
    if missing_keep:
        raise SystemExit(
            "manifest keep_plugins not present in source (fix names via --list-plugins): "
            + ", ".join(missing_keep)
        )

    present_fonts = set(fonts_in(src))
    missing_fonts = sorted(keep_fonts - present_fonts)
    if missing_fonts:
        raise SystemExit(
            "manifest keep_fonts not present in source (fix names via --list-fonts): "
            + ", ".join(missing_fonts)
        )

    excluded_plugins = sorted(present_plugins - keep_plugins)
    excluded_fonts = sorted(present_fonts - keep_fonts)

    skip_dirs = {f"{PLUGINS_SUBDIR}/{name}" for name in excluded_plugins}
    skip_files = {f"{FONTS_SUBDIR}/{name}" for name in excluded_fonts}

    src_size = dir_size(src)
    # 所有只读预检通过后，才允许替换现存目标；且只删本工具自己的产物，
    # 避免误传路径时把任意目录 rmtree 掉。
    if args.force and os.path.exists(dst):
        if not os.path.isfile(os.path.join(dst, "SLIM_REPORT.json")):
            raise SystemExit(
                f"--force refuses to remove a directory that is not a slim product: {dst}"
            )
        print(f"[warn] removing existing dst: {dst}")
        shutil.rmtree(dst)
    skipped_dirs, skipped_files = copy_tree(src, dst, skip_dirs, skip_files)
    # 报告文件在下面才写入，dst_size 不含它；check_slim 只用它判「非空」。
    dst_size = dir_size(dst)

    print(f"src: {src} ({human(src_size)})")
    print(f"dst: {dst} ({human(dst_size)}, saved {human(src_size - dst_size)}, {100*(src_size-dst_size)/max(src_size,1):.1f}%)")
    print(f"plugins: kept {len(keep_plugins & present_plugins)}, excluded {skipped_dirs} (expected {len(excluded_plugins)})")
    print(f"fonts:   kept {len(present_fonts & keep_fonts)}, excluded {skipped_files} (expected {len(excluded_fonts)})")

    with open(args.manifest, "rb") as fh:
        manifest_sha256 = hashlib.sha256(fh.read()).hexdigest()
    report = {
        "src": src, "dst": dst, "src_bytes": src_size, "dst_bytes": dst_size,
        "kept_plugins": sorted(keep_plugins & present_plugins),
        "excluded_plugins": excluded_plugins,
        "kept_fonts": sorted(present_fonts & keep_fonts),
        "excluded_fonts_count": len(excluded_fonts),
        "manifest_sha256": manifest_sha256,
        "dst_bytes_note": "excludes SLIM_REPORT.json itself",
    }
    with open(os.path.join(dst, "SLIM_REPORT.json"), "w", encoding="utf-8") as fh:
        json.dump(report, fh, ensure_ascii=False, indent=2)
        fh.write("\n")
    print("report: SLIM_REPORT.json written into dst")
    return 0


if __name__ == "__main__":
    sys.exit(main())
