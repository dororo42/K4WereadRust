#!/usr/bin/env python3
"""slim_koreader.py / check_slim.py 自测试：合成 KOReader 树驱动，无设备依赖。

运行: python3 tools/test_slim.py   （在仓根执行）
"""
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

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
                 "SSH.koplugin", "opds.koplugin", "wallabag.koplugin", "newsdownloader.koplugin",
                 "texteditor.koplugin", "cloudstorage.koplugin"):
        touch(f"plugins/{plug}/main.lua")
    for font in ("droid/DroidSansFallback.ttf", "droid/DroidSansFallbackBold.ttf",
                 "freefont/FreeSans.ttf", "freefont/FreeSansBold.ttf",
                 "freefont/FreeSerif.ttf", "freefont/COPYING", "noto/NotoSansArabic-Regular.ttf",
                 "noto/NotoSansHebrew-Regular.ttf", "custom/ReadingSerif.otf",
                 "noto/NotoSansCJKsc-Regular.otf", "noto/NotoSans-Regular.ttf",
                 "noto/NotoSans-Bold.ttf", "noto/NotoSans-Italic.ttf",
                 "noto/NotoSans-BoldItalic.ttf", "noto/LICENSE",
                 "noto/NotoSansBengaliUI-Bold.ttf", "noto/NotoSansBengaliUI-Regular.ttf",
                 "noto/NotoSansDevanagariUI-Bold.ttf", "noto/NotoSansDevanagariUI-Regular.ttf",
                 "noto/NotoSansArabicUI-Bold.ttf", "noto/NotoSansArabicUI-Regular.ttf",
                 "droid/DroidSansMono.ttf", "droid/NOTICE",
                 "nerdfonts/symbols.ttf", "nerdfonts/LICENSE", "README.md"):
        touch(f"fonts/{font}", 4096)


def run(args):
    return subprocess.run([sys.executable, "-B"] + args, capture_output=True, text=True, timeout=15)


def snapshot_tree(root):
    """比较目录、文件字节及链接目标，不跟随目录符号链接。"""
    entries = {}
    for current, dirs, files in os.walk(root):
        for name in dirs + files:
            path = os.path.join(current, name)
            rel = os.path.relpath(path, root)
            if os.path.islink(path):
                entries[rel] = ("link", os.readlink(path))
            elif os.path.isdir(path):
                entries[rel] = ("dir",)
            else:
                with open(path, "rb") as fh:
                    entries[rel] = ("file", fh.read())
    return entries


def write_manifest(path, data):
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(data, fh)


def make_fixture(tmp, existing_dst=False):
    src = os.path.join(tmp, "parent", "koreader")
    dst = os.path.join(tmp, "slim")
    manifest_path = os.path.join(tmp, "manifest.json")
    build_fake_tree(src)
    manifest = {
        "keep_plugins": ["hotkeys.koplugin", "weread_K4.koplugin", "k4refresh.koplugin"],
        "keep_fonts": ["droid/DroidSansFallback.ttf", "freefont/FreeSans.ttf",
                       "freefont/FreeSansBold.ttf", "freefont/FreeSerif.ttf",
                       "custom/ReadingSerif.otf"],
    }
    write_manifest(manifest_path, manifest)
    if existing_dst:
        build_fake_tree(dst)
        os.makedirs(os.path.join(dst, "existing-empty-dir"))
        with open(os.path.join(dst, "sentinel"), "wb") as fh:
            fh.write(b"existing destination must survive preflight failure")
    return src, dst, manifest_path, manifest


def main():
    tmp = tempfile.mkdtemp(prefix="k4slimtest_")
    try:
        src = os.path.join(tmp, "koreader")
        dst = os.path.join(tmp, "slim")
        build_fake_tree(src)
        src_before = snapshot_tree(src)

        r = run([SLIM, src, dst, "--manifest", os.path.join(TOOLS, "slim_manifest.json")])
        assert r.returncode == 0, f"slim failed: {r.stdout}\n{r.stderr}"
        out = r.stdout
        assert "excluded 5 (expected 5)" in out, f"plugin exclusion count wrong: {out}"

        for keep in ("hotkeys.koplugin", "weread_K4.koplugin", "k4refresh.koplugin", "SSH.koplugin"):
            assert os.path.isfile(os.path.join(dst, "plugins", keep, "main.lua")), f"missing keep plugin {keep}"
        for gone in ("opds.koplugin", "wallabag.koplugin", "newsdownloader.koplugin",
                     "texteditor.koplugin", "cloudstorage.koplugin"):
            assert not os.path.exists(os.path.join(dst, "plugins", gone)), f"excluded plugin leaked: {gone}"
        for keep in ("noto/NotoSansCJKsc-Regular.otf", "noto/NotoSans-Regular.ttf", "noto/LICENSE",
                     "noto/NotoSansBengaliUI-Bold.ttf", "noto/NotoSansBengaliUI-Regular.ttf",
                     "noto/NotoSansDevanagariUI-Bold.ttf", "noto/NotoSansDevanagariUI-Regular.ttf",
                     "noto/NotoSansArabicUI-Bold.ttf", "noto/NotoSansArabicUI-Regular.ttf",
                     "droid/DroidSansMono.ttf", "nerdfonts/symbols.ttf", "freefont/FreeSans.ttf",
                     "freefont/FreeSerif.ttf", "freefont/COPYING", "README.md"):
            assert os.path.isfile(os.path.join(dst, "fonts", keep)), f"missing keep font {keep}"
        for gone in ("custom",):
            assert not os.path.exists(os.path.join(dst, "fonts", gone)), f"excluded font dir leaked: {gone}"
        for gone in ("freefont/FreeSansBold.ttf",):
            assert not os.path.isfile(os.path.join(dst, "fonts", gone)), f"excluded font leaked: {gone}"
        assert not os.path.isfile(os.path.join(dst, "fonts", "noto", "NotoSansHebrew-Regular.ttf")), \
            "excluded noto font leaked"

        src_size = sum(os.path.getsize(os.path.join(r_, f)) for r_, _, fs in os.walk(src) for f in fs)
        dst_size = sum(os.path.getsize(os.path.join(r_, f)) for r_, _, fs in os.walk(dst) for f in fs)
        assert dst_size < src_size, "slim tree not smaller"

        with open(os.path.join(dst, "SLIM_REPORT.json")) as fh:
            report = json.load(fh)
        assert report["excluded_plugins"] == ["cloudstorage.koplugin", "newsdownloader.koplugin",
                                              "opds.koplugin", "texteditor.koplugin", "wallabag.koplugin"]

        c = run([CHECK, dst, "--manifest", os.path.join(TOOLS, "slim_manifest.json")])
        assert c.returncode == 0, f"check failed: {c.stdout}\n{c.stderr}"

        dst_before = snapshot_tree(dst)
        r2 = run([SLIM, src, dst, "--manifest", os.path.join(TOOLS, "slim_manifest.json")])
        assert r2.returncode != 0, "idempotency guard failed: second run should refuse existing dst"
        assert snapshot_tree(dst) == dst_before, "existing destination mutated!"
        assert snapshot_tree(src) == src_before, "source tree mutated!"

        print("test_slim: PASS (exclusion, keep-set, report, check gate, idempotency guard, source untouched)")
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


class SlimTests(unittest.TestCase):
    def assert_read_only_run(self, tmp, args, success=False, diagnostic=None):
        # 整个临时沙箱快照覆盖源树、现存目标、空目录和所有别名。
        before = snapshot_tree(tmp)
        result = run(args)
        self.assertTrue(before == snapshot_tree(tmp), "source or destination contents changed")
        output = result.stdout + result.stderr
        if success:
            self.assertEqual(result.returncode, 0, output)
        else:
            self.assertNotEqual(result.returncode, 0, output)
        if diagnostic:
            self.assertIn(diagnostic, output)

    def assert_slim(self, src, dst, manifest_path, force=False):
        before = snapshot_tree(src)
        result = run([SLIM, src, dst, "--manifest", manifest_path] + (["--force"] if force else []))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(snapshot_tree(src), before)

    def test_existing_smoke(self):
        self.assertEqual(main(), 0)

    def test_source_report_symlink_cannot_overwrite_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            src, dst, manifest_path, _manifest = make_fixture(tmp)
            os.symlink(os.path.join(src, "reader.lua"), os.path.join(src, "SLIM_REPORT.json"))
            self.assert_slim(src, dst, manifest_path)
            self.assertFalse(os.path.islink(os.path.join(dst, "SLIM_REPORT.json")))
            self.assert_read_only_run(tmp, [CHECK, dst, "--manifest", manifest_path], success=True)

    def test_overlap_rejected_before_mutation(self):
        for relation in ("same", "ancestor", "descendant", "existing_descendant"):
            for alias in ("direct", "source", "destination", "destination_parent"):
                for force in (False, True):
                    with self.subTest(relation=relation, alias=alias, force=force), tempfile.TemporaryDirectory() as tmp:
                        src, _dst, manifest_path, _manifest = make_fixture(tmp)
                        if relation == "same":
                            dst = src
                        elif relation == "ancestor":
                            dst = os.path.dirname(src)
                        else:
                            # 长组件限制旧实现 RED 时的递归深度，避免耗尽临时空间。
                            dst = os.path.join(src, "nested-slim-" + "x" * 180)
                            if relation == "existing_descendant":
                                build_fake_tree(dst)
                        link = os.path.join(tmp, "alias")
                        if alias == "source":
                            os.symlink(src, link)
                            src = link
                        elif alias == "destination":
                            os.symlink(dst, link)
                            dst = link
                        elif alias == "destination_parent":
                            os.symlink(os.path.dirname(dst), link)
                            dst = os.path.join(link, os.path.basename(dst))
                        args = [SLIM, src, dst, "--manifest", manifest_path]
                        if force:
                            args.append("--force")
                        self.assert_read_only_run(tmp, args, diagnostic="overlap")

    def test_invalid_manifest_preserves_existing_destination(self):
        invalid = ["{", "[]", "null", "{}",
                   '{"keep_plugins": "hotkeys.koplugin", "keep_fonts": []}',
                   '{"keep_plugins": [], "keep_fonts": null}',
                   '{"keep_plugins": [{}], "keep_fonts": []}',
                   '{"keep_plugins": [], "keep_fonts": [null]}']
        for content in invalid:
            with self.subTest(manifest=content), tempfile.TemporaryDirectory() as tmp:
                src, dst, manifest_path, _manifest = make_fixture(tmp, existing_dst=True)
                with open(manifest_path, "w", encoding="utf-8") as fh:
                    fh.write(content)
                self.assert_read_only_run(tmp, [SLIM, src, dst, "--manifest", manifest_path, "--force"])

    def test_missing_kept_resources_fail_before_mutation(self):
        for kind, count in (("plugins", 3), ("fonts", 5)):
            for index in range(count):
                for existing_dst in (False, True):
                    with self.subTest(kind=kind, index=index, existing_dst=existing_dst), tempfile.TemporaryDirectory() as tmp:
                        src, dst, manifest_path, manifest = make_fixture(tmp, existing_dst=existing_dst)
                        name = manifest[f"keep_{kind}"][index]
                        path = os.path.join(src, kind, name)
                        if kind == "plugins":
                            shutil.rmtree(path)
                        else:
                            os.remove(path)
                        self.assert_read_only_run(tmp, [SLIM, src, dst, "--manifest", manifest_path, "--force"])

    def test_disjoint_prefix_paths_and_force_still_work(self):
        with tempfile.TemporaryDirectory() as tmp:
            src, _dst, manifest_path, _manifest = make_fixture(tmp)
            dst = src + "-slim"
            self.assert_slim(src, dst, manifest_path)
            before = snapshot_tree(tmp)
            self.assert_slim(src, dst, manifest_path, force=True)
            self.assertEqual(snapshot_tree(tmp), before)
            self.assert_read_only_run(tmp, [CHECK, dst, "--manifest", manifest_path], success=True)

    def test_check_rejects_every_missing_kept_resource(self):
        for kind, count in (("plugins", 3), ("fonts", 5)):
            for index in range(count):
                with self.subTest(kind=kind, index=index), tempfile.TemporaryDirectory() as tmp:
                    src, dst, manifest_path, manifest = make_fixture(tmp)
                    self.assert_slim(src, dst, manifest_path)
                    name = manifest[f"keep_{kind}"][index]
                    path = os.path.join(dst, kind, name)
                    if kind == "plugins":
                        shutil.rmtree(path)
                    else:
                        os.remove(path)
                    self.assert_read_only_run(tmp, [CHECK, dst, "--manifest", manifest_path])

    def test_check_rejects_extra_resources_despite_forged_report(self):
        for kind, name in (("plugins", "opds.koplugin/main.lua"),
                           ("plugins", "unreported.koplugin/main.lua"),
                           ("fonts", "freefont/Unused.otf"),
                           ("fonts", "droid/DroidSansFallbackSpoof.ttf")):
            with self.subTest(kind=kind, name=name), tempfile.TemporaryDirectory() as tmp:
                src, dst, manifest_path, _manifest = make_fixture(tmp)
                self.assert_slim(src, dst, manifest_path)
                path = os.path.join(dst, kind, name)
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "wb") as fh:
                    fh.write(b"not in manifest")
                report_path = os.path.join(dst, "SLIM_REPORT.json")
                with open(report_path, encoding="utf-8") as fh:
                    report = json.load(fh)
                report["excluded_plugins"] = []
                report[f"kept_{kind}"].append(name.split("/")[0] if kind == "plugins" else name)
                write_manifest(report_path, report)
                self.assert_read_only_run(tmp, [CHECK, dst, "--manifest", manifest_path])

    def test_check_uses_manifest_not_report_exclusions(self):
        with tempfile.TemporaryDirectory() as tmp:
            src, dst, manifest_path, manifest = make_fixture(tmp)
            self.assert_slim(src, dst, manifest_path)
            report_path = os.path.join(dst, "SLIM_REPORT.json")
            with open(report_path, encoding="utf-8") as fh:
                report = json.load(fh)
            report["excluded_plugins"] = manifest["keep_plugins"]
            write_manifest(report_path, report)
            self.assert_read_only_run(tmp, [CHECK, dst, "--manifest", manifest_path], success=True)

    def test_check_rejects_file_in_place_of_kept_plugin(self):
        with tempfile.TemporaryDirectory() as tmp:
            src, dst, manifest_path, manifest = make_fixture(tmp)
            self.assert_slim(src, dst, manifest_path)
            path = os.path.join(dst, "plugins", manifest["keep_plugins"][0])
            shutil.rmtree(path)
            with open(path, "wb") as fh:
                fh.write(b"not a plugin directory")
            self.assert_read_only_run(tmp, [CHECK, dst, "--manifest", manifest_path])

    def test_check_accepts_k5_noto_cjk(self):
        # K5 树的 CJK 回退是 NotoSansCJKsc（无 DroidSansFallback）；门禁必须接受该形态。
        with tempfile.TemporaryDirectory() as tmp:
            src, dst, manifest_path, _manifest = make_fixture(tmp)
            manifest = {
                "keep_plugins": ["hotkeys.koplugin", "weread_K4.koplugin", "k4refresh.koplugin"],
                "keep_fonts": ["noto/NotoSansCJKsc-Regular.otf", "noto/NotoSans-Regular.ttf"],
            }
            write_manifest(manifest_path, manifest)
            self.assert_slim(src, dst, manifest_path)
            self.assert_read_only_run(tmp, [CHECK, dst, "--manifest", manifest_path], success=True)

    def deploy_probe(self, tmp, manifest):
        """把设备侧脚本的路径常量改指向合成树，并让默认分支执行 verify()。"""
        with open(os.path.join(TOOLS, "k5_slim_deploy.sh"), encoding="utf-8") as fh:
            body = fh.read()
        kr = os.path.join(tmp, "koreader")

        def touch(rel):
            path = os.path.join(kr, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "wb") as fh:
                fh.write(b"")

        for name in manifest["keep_plugins"]:
            touch(os.path.join("plugins", name, "main.lua"))
        for name in manifest["keep_fonts"]:
            touch(os.path.join("fonts", name))
        for directory in ("settings", "weread/cache", "libs"):
            os.makedirs(os.path.join(kr, directory), exist_ok=True)
        touch("koreader.sh")
        luajit = os.path.join(kr, "luajit")
        with open(luajit, "w", encoding="utf-8") as fh:
            fh.write("#!/bin/sh\nexit 0\n")
        os.chmod(luajit, 0o755)
        with open(luajit, "rb") as fh:
            digest = hashlib.md5(fh.read()).hexdigest()
        listing = os.path.join(tmp, "delete.txt")
        with open(listing, "w", encoding="utf-8") as fh:
            fh.write("")

        constants = {
            "KR": kr,
            "BAK": os.path.join(tmp, "backup"),
            "LIST": listing,
            "LUAJIT_MD5": digest,
        }
        lines = []
        for line in body.splitlines():
            match = re.match(r"^(KR|BAK|LIST|LUAJIT_MD5)=", line)
            if match:
                lines.append(f"{match.group(1)}={constants[match.group(1)]}")
            elif line.startswith('  *)        echo "usage:'):
                lines.append("  *)        verify ;;")
            else:
                lines.append(line)
        probe = os.path.join(tmp, "probe.sh")
        with open(probe, "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines) + "\n")
        return probe, kr

    def test_deploy_keep_lists_match_manifest(self):
        with open(os.path.join(TOOLS, "slim_manifest.json"), encoding="utf-8") as fh:
            manifest = json.load(fh)
        with open(os.path.join(TOOLS, "k5_slim_deploy.sh"), encoding="utf-8") as fh:
            body = fh.read()
        for key, var in (("keep_plugins", "KEEP_PLUGINS"), ("keep_fonts", "KEEP_FONTS")):
            match = re.search(rf'^{var}="([^"]*)"$', body, re.M)
            self.assertIsNotNone(match, f"{var} not found in deploy script")
            self.assertEqual(set(manifest[key]), set(match.group(1).split()),
                             f"{var} drifted from slim_manifest.json")

    def test_deploy_verify_accepts_kept_freefont_dir(self):
        # F-01 回归：KEEP_FONTS 保留 freefont/* 时目录必然存在，
        # verify() 不得再以「目录存在」判泄漏。
        with tempfile.TemporaryDirectory() as tmp:
            with open(os.path.join(TOOLS, "slim_manifest.json"), encoding="utf-8") as fh:
                manifest = json.load(fh)
            probe, kr = self.deploy_probe(tmp, manifest)
            self.assertTrue(os.path.isdir(os.path.join(kr, "fonts", "freefont")))
            result = subprocess.run(["sh", probe, "verify"],
                                    capture_output=True, text=True, timeout=15)
            output = result.stdout + result.stderr
            self.assertNotIn("FAIL leak: fonts/freefont", output)
            self.assertIn("VERIFY: PASS", output)
            self.assertEqual(result.returncode, 0, output)


if __name__ == "__main__":
    unittest.main(verbosity=2)
