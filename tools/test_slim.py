#!/usr/bin/env python3
"""slim_koreader.py / check_slim.py 自测试：合成 KOReader 树驱动，无设备依赖。

运行: python3 tools/test_slim.py   （在仓根执行）
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile

TOOLS = os.path.dirname(os.path.abspath(__file__))
SLIM = os.path.join(TOOLS, "slim_koreader.py")
CHECK = os.path.join(TOOLS, "check_slim.py")


def build_fake_tree(root):
    def touch(rel, size=64):
        p = os.path.join(root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "wb") as fh:
            fh.write(b"x" * size)

    touch("koreader.sh", 200)
    touch("reader.lua", 100)
    touch("frontend/apps/reader/readerui.lua")
    touch("frontend/apps/filemanager/filemanager.lua")
    for plug in ("hotkeys.koplugin", "weread_K4.koplugin", "k4refresh.koplugin",
                 "opds.koplugin", "wallabag.koplugin", "newsdownloader.koplugin",
                 "texteditor.koplugin", "cloudstorage.koplugin"):
        touch(f"plugins/{plug}/main.lua")
    for font in ("droid/DroidSansFallback.ttf", "droid/DroidSansFallbackBold.ttf",
                 "freefont/FreeSans.ttf", "freefont/FreeSansBold.ttf",
                 "freefont/FreeSerif.ttf", "noto/NotoSansArabic-Regular.ttf",
                 "noto/NotoSansHebrew-Regular.ttf"):
        touch(f"fonts/{font}", 4096)


def run(args):
    return subprocess.run([sys.executable] + args, capture_output=True, text=True)


def main():
    tmp = tempfile.mkdtemp(prefix="k4slimtest_")
    try:
        src = os.path.join(tmp, "koreader")
        dst = os.path.join(tmp, "slim")
        build_fake_tree(src)

        r = run([SLIM, src, dst, "--manifest", os.path.join(TOOLS, "slim_manifest.json")])
        assert r.returncode == 0, f"slim failed: {r.stdout}\n{r.stderr}"
        out = r.stdout
        assert "excluded 5 (expected 5)" in out, f"plugin exclusion count wrong: {out}"

        for keep in ("hotkeys.koplugin", "weread_K4.koplugin", "k4refresh.koplugin"):
            assert os.path.isfile(os.path.join(dst, "plugins", keep, "main.lua")), f"missing keep plugin {keep}"
        for gone in ("opds.koplugin", "wallabag.koplugin", "newsdownloader.koplugin",
                     "texteditor.koplugin", "cloudstorage.koplugin"):
            assert not os.path.exists(os.path.join(dst, "plugins", gone)), f"excluded plugin leaked: {gone}"
        assert os.path.isfile(os.path.join(dst, "fonts", "droid", "DroidSansFallback.ttf"))
        assert not os.path.exists(os.path.join(dst, "fonts", "noto")), "non-kept font dir leaked"

        src_size = sum(os.path.getsize(os.path.join(r_, f)) for r_, _, fs in os.walk(src) for f in fs)
        dst_size = sum(os.path.getsize(os.path.join(r_, f)) for r_, _, fs in os.walk(dst) for f in fs)
        assert dst_size < src_size, "slim tree not smaller"

        with open(os.path.join(dst, "SLIM_REPORT.json")) as fh:
            report = json.load(fh)
        assert report["excluded_plugins"] == ["cloudstorage.koplugin", "newsdownloader.koplugin",
                                              "opds.koplugin", "texteditor.koplugin", "wallabag.koplugin"]

        c = run([CHECK, dst, "--manifest", os.path.join(TOOLS, "slim_manifest.json")])
        assert c.returncode == 0, f"check failed: {c.stdout}\n{c.stderr}"

        r2 = run([SLIM, src, dst, "--manifest", os.path.join(TOOLS, "slim_manifest.json")])
        assert r2.returncode != 0, "idempotency guard failed: second run should refuse existing dst"

        src_after = sum(os.path.getsize(os.path.join(r_, f)) for r_, _, fs in os.walk(src) for f in fs)
        assert src_after == src_size, "source tree mutated!"

        print("test_slim: PASS (exclusion, keep-set, report, check gate, idempotency guard, source untouched)")
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
