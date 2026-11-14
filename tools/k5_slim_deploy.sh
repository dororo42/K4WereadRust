#!/bin/sh
# K5 slim deploy — device-side surgical deployment with one-command rollback.
# KEEP_PLUGINS/KEEP_FONTS 是 tools/slim_manifest.json 的设备侧副本（设备无 Python）。
# 两份清单由 tools/test_slim.py 的 test_deploy_keep_lists_match_manifest 校验一致；
# 改 manifest 必须同步改这里，否则部署结果与 check_slim 门禁脱节。
# Usage: k5_slim_deploy.sh {dryrun|execute|rollback|status}
set -u

KR=/mnt/us/koreader
BAK=/mnt/us/koreader.pre-slim-20261108
LIST=/tmp/k5_slim_delete.txt
LUAJIT_MD5=4b6d8f84f0a01b19a72ae04834cd6e50

KEEP_PLUGINS="hotkeys.koplugin weread_K4.koplugin k4refresh.koplugin SSH.koplugin"
KEEP_FONTS="noto/NotoSansCJKsc-Regular.otf noto/NotoSans-Regular.ttf noto/NotoSans-Bold.ttf noto/NotoSans-Italic.ttf noto/NotoSans-BoldItalic.ttf noto/NotoSansBengaliUI-Bold.ttf noto/NotoSansBengaliUI-Regular.ttf noto/NotoSansDevanagariUI-Bold.ttf noto/NotoSansDevanagariUI-Regular.ttf noto/NotoSansArabicUI-Bold.ttf noto/NotoSansArabicUI-Regular.ttf noto/LICENSE droid/DroidSansMono.ttf droid/NOTICE nerdfonts/symbols.ttf nerdfonts/LICENSE freefont/FreeSans.ttf freefont/FreeSerif.ttf freefont/COPYING README.md"

# pgrep -f koreader 会命中 dropbear（argv[0]=/mnt/us/koreader/dropbear），必须按 comm 匹配
koreader_running() {
  for f in /proc/[0-9]*/stat; do
    comm=$(cut -d" " -f2 "$f" 2>/dev/null)
    case "$comm" in
      *koread*|*luajit*|*reader.lua*) return 0 ;;
    esac
  done
  return 1
}

derive() {
  : > "$LIST"
  for d in "$KR"/plugins/*; do
    [ -e "$d" ] || continue
    name=$(basename "$d")
    keep=""
    for k in $KEEP_PLUGINS; do [ "$k" = "$name" ] && keep=1; done
    [ -n "$keep" ] || echo "plugins/$name" >> "$LIST"
  done
  ( cd "$KR/fonts" 2>/dev/null || exit 0
    find . -type f | sed 's|^\./||' | sort
  ) | while IFS= read -r rel; do
    [ -n "$rel" ] || continue
    keep=""
    for k in $KEEP_FONTS; do [ "$k" = "$rel" ] && keep=1; done
    [ -n "$keep" ] || echo "fonts/$rel" >> "$LIST"
  done
}

writable() {
  touch /mnt/us/.slim_rw_test 2>/dev/null || return 1
  rm -f /mnt/us/.slim_rw_test
  return 0
}

verify() {
  fail=0
  for k in $KEEP_PLUGINS; do
    [ -f "$KR/plugins/$k/main.lua" ] || { echo "FAIL keep plugin: $k"; fail=1; }
  done
  for f in $KEEP_FONTS; do
    [ -f "$KR/fonts/$f" ] || { echo "FAIL keep font: $f"; fail=1; }
  done
  [ -f "$KR/koreader.sh" ] || { echo "FAIL: koreader.sh"; fail=1; }
  [ -x "$KR/luajit" ] || { echo "FAIL: luajit"; fail=1; }
  got=$(md5sum "$KR/luajit" 2>/dev/null | awk '{print $1}')
  [ "$got" = "$LUAJIT_MD5" ] || { echo "FAIL: luajit md5 got=$got want=$LUAJIT_MD5"; fail=1; }
  while IFS= read -r rel; do
    [ -n "$rel" ] || continue
    [ -e "$KR/$rel" ] && { echo "FAIL leak: $rel"; fail=1; }
  done < "$LIST"
  [ -d "$KR/settings" ] || { echo "FAIL: settings/ missing"; fail=1; }
  [ -f "$KR/settings/weread.credentials.lua" ] || echo "WARN: settings/weread.credentials.lua not at expected path (metadata check only)"
  [ -d "$KR/weread/cache" ] || echo "WARN: weread/cache missing"
  # 泄漏判定只依赖 $LIST 逐项检查（上面 63-66 行）。不要用目录存在与否断言：
  # KEEP_FONTS 保留了 freefont/FreeSans.ttf 等文件，目录必然存在，旧断言会让
  # 任何合规产物都被判 VERIFY FAIL。
  ( cd "$KR" && LD_LIBRARY_PATH="$KR/libs" ./luajit -e 'print("SMOKE ok", jit.version)' ) || { echo "FAIL: luajit smoke"; fail=1; }
  if [ "$fail" = 0 ]; then echo "VERIFY: PASS"; else echo "VERIFY: FAIL"; fi
  return $fail
}

cmd_dryrun() {
  echo "== K5 SLIM DRYRUN $(date) =="
  echo "-- preflight --"
  if koreader_running; then
    echo "WARN: koreader running (execute will refuse):"
    ps w | grep -i koread | grep -v grep
  else
    echo "ok: no koreader process"
  fi
  [ -e "$BAK" ] && echo "WARN: backup already exists: $BAK" || echo "ok: no backup dir yet"
  if writable; then echo "ok: /mnt/us writable"; else echo "WARN: /mnt/us NOT writable (USB mass-storage mode?)"; fi
  echo "-- keep plugins --"
  for k in $KEEP_PLUGINS; do echo "KEEP plugins/$k"; done
  echo "-- keep fonts --"
  for k in $KEEP_FONTS; do echo "KEEP fonts/$k"; done
  echo "-- delete list --"
  derive
  echo "delete counts: plugins=$(grep -c '^plugins/' "$LIST") fonts=$(grep -c '^fonts/' "$LIST")"
  cat "$LIST"
  echo "-- sizes --"
  du -sk "$KR" 2>/dev/null
  df -m /mnt/us | tail -1
  echo "== DRYRUN DONE — no changes made =="
}

cmd_execute() {
  echo "== K5 SLIM EXECUTE $(date) =="
  if koreader_running; then echo "REFUSE: koreader running"; ps w | grep -i koread | grep -v grep; exit 1; fi
  [ -e "$BAK" ] && { echo "REFUSE: $BAK exists"; exit 1; }
  [ -d "$KR" ] || { echo "REFUSE: $KR missing"; exit 1; }
  writable || { echo "REFUSE: /mnt/us not writable (USB mode?)"; exit 1; }
  derive
  [ -s "$LIST" ] || { echo "REFUSE: empty delete list"; exit 1; }
  echo "delete counts: plugins=$(grep -c '^plugins/' "$LIST") fonts=$(grep -c '^fonts/' "$LIST")"

  echo "step1: mv $KR -> $BAK"
  mv "$KR" "$BAK" || { echo "FATAL: mv failed, tree untouched"; exit 1; }

  echo "step2: cp -dpR backup -> fresh tree"
  if ! cp -dpR "$BAK" "$KR"; then
    echo "CP FAILED — restoring original"
    rm -rf "$KR"
    mv "$BAK" "$KR" && echo "RESTORED: original tree back in place" || echo "FATAL: restore failed, inspect manually"
    exit 1
  fi
  sync

  echo "step3: surgical delete ($(wc -l < "$LIST") items)"
  while IFS= read -r rel; do
    [ -n "$rel" ] || continue
    rm -rf "$KR/$rel"
  done < "$LIST"
  for d in "$KR"/fonts/*/; do [ -d "$d" ] && rmdir "$d" 2>/dev/null; done
  sync

  echo "step4: verify"
  if verify; then
    echo "== EXECUTE OK — backup at $BAK =="
    echo "rollback: sh $0 rollback"
    du -sk "$KR" "$BAK" 2>/dev/null
    df -m /mnt/us | tail -1
  else
    echo "== VERIFY FAILED — original intact at $BAK, rollback with: sh $0 rollback =="
    exit 1
  fi
}

cmd_rollback() {
  echo "== K5 SLIM ROLLBACK $(date) =="
  [ -d "$BAK" ] || { echo "REFUSE: no backup at $BAK"; exit 1; }
  if koreader_running; then echo "REFUSE: koreader running"; exit 1; fi
  # 先把当前树改名保留，再恢复备份：任一 mv 失败都还有完整副本，
  # 不出现「已 rm -rf 原树但备份还在」的破坏性窗口。
  FAILED="$KR.failed-$(date +%Y%m%d%H%M%S)"
  if [ -e "$KR" ]; then
    echo "keeping slim tree as $FAILED"
    mv "$KR" "$FAILED" || { echo "FATAL: cannot move current tree aside"; exit 1; }
  fi
  if ! mv "$BAK" "$KR"; then
    echo "FATAL: mv backup failed; restoring $FAILED"
    [ -e "$FAILED" ] && mv "$FAILED" "$KR"
    exit 1
  fi
  sync
  echo "ROLLED BACK: original tree restored (slim tree kept at $FAILED)"
  md5sum "$KR/luajit" 2>/dev/null
}

cmd_status() {
  echo "tree:   $([ -d "$KR" ] && echo present || echo MISSING)"
  echo "backup: $([ -d "$BAK" ] && echo present || echo none)"
  du -sk "$KR" "$BAK" 2>/dev/null
  if koreader_running; then echo "koreader: RUNNING"; else echo "koreader: not running"; fi
  df -m /mnt/us | tail -1
}

case "${1:-}" in
  dryrun)   cmd_dryrun ;;
  execute)  cmd_execute ;;
  rollback) cmd_rollback ;;
  status)   cmd_status ;;
  *)        echo "usage: $0 {dryrun|execute|rollback|status}"; exit 2 ;;
esac
