#!/usr/bin/env python3
"""CI gate: the two Lua transport bindings in this repo must stay byte-identical
to their production copies in weread_K4.koplugin.

2026-11-12 three-repo audit (I-1): weread/lib/transport.lua gained
`backend_explicit` (menu choice outranks the deployment env) while this repo's
lua/transport.lua kept the old logic, yet README.md / AGENTS.md still claimed
"byte-for-byte identical".

Specs are deliberately NOT compared: this repo runs them with
tools/test_k4tls_lua.lua (Lua 5.1 shim) and the plugin repo uses busted, so
they are semantically aligned but not byte-identical.

Usage (from the repo root):
    python3 tools/check_lua_mirror.py [--peer-repo /path/to/weread_K4.koplugin]
"""
import argparse
import hashlib
import pathlib
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
MIRRORS = {
    "lua/k4tls.lua": "weread/lib/k4tls.lua",
    "lua/transport.lua": "weread/lib/transport.lua",
}
PEER_DEFAULT = "../weread_K4.koplugin"


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fail(message: str) -> int:
    print(f"lua mirror FAIL: {message}", file=sys.stderr)
    return 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo-root", default=str(REPO_ROOT), help="this repo's root")
    ap.add_argument("--peer-repo", default=PEER_DEFAULT,
                    help="path to the weread_K4.koplugin checkout")
    args = ap.parse_args()
    root = pathlib.Path(args.repo_root)
    peer = pathlib.Path(args.peer_repo)

    if not peer.is_dir():
        print(f"lua mirror SKIP: peer repo not present ({peer})")
        return 0

    print(f"peer repo: {peer.resolve()}")
    for local_rel, peer_rel in MIRRORS.items():
        local, other = root / local_rel, peer / peer_rel
        if not local.is_file():
            return fail(f"local mirror missing: {local}")
        if not other.is_file():
            return fail(f"peer production copy missing: {other}")
        a, b = digest(local), digest(other)
        if a != b:
            return fail(f"mirror drift: {local_rel} sha256={a} != "
                        f"{peer_rel} sha256={b} "
                        "(plugin repo is the source of truth; sync this repo)")
        print(f"mirror OK: {local_rel} == {peer_rel} ({a[:12]}…)")
    print(f"lua mirror OK: {len(MIRRORS)} binding(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
