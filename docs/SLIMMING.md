# Phase 1-B 配置级瘦身（S1）

评估报告 §二 S1 / §3.1 / §七 2。**纯打包层：不改任何代码文件内容，不动源树。**
引擎与 native 库的删除属 S2（Phase 2 fork 裁剪），本工具明确不做。

## 裁剪面（Phase 1-B 范围）

| 项 | 设备实测（2026-11-08） | 瘦身后预期 |
|---|---|---|
| plugins | 35 个 / 4.5 MB | 保留 3（hotkeys / weread_K4 / k4refresh），约 -4 MB |
| fonts | 多语言全集 / 68.4 MB | 1 CJK + 1 西文族，约 -55 MB |
| 合计 | 176.3 MB | **约 115-120 MB** |

> 诚实校准：报告 §七 写的 "176M → ~70M" 需要叠加 S2 的引擎/Lua 裁剪才能达到；
> S1（本阶段）的合理预期是 ~115-120 MB。§五 的 S1 "体积 -60%+" 指字体+插件单项占比。

## K5 实测差异（2026-11-08）

用户指定 K5 的 KOReader v2026.07.1 树，`du -sk` 实测整树 65728 KiB、字体 25168 KiB、插件 4008 KiB；这不是上表报告中的 K4 基线。
该树没有 `droid/DroidSansFallback.ttf` 或 `freefont/FreeSansBold.ttf`，CJK 文件是 `noto/NotoSansCJKsc-Regular.otf`（16032 KiB）。
现有 manifest **不能用于该 K5**；必须确认界面字体、当前正文字体及字体许可证保留集后单独调整，再核算真实节省量。
设备还有 `SSH.koplugin`；未经确认不得按三插件白名单部署而移除远程恢复入口。本轮仅枚举和测量，没有裁剪或部署设备树。

## 工具

- `tools/slim_koreader.py`：源树 → 精简副本。幂等、永不写源树、产物带 `SLIM_REPORT.json`。
- `tools/check_slim.py`：产物门禁（必需入口、保留插件齐全、排除面无泄漏、CJK 回退字体在位）。
- `tools/slim_manifest.json`：数据驱动清单（插件白名单 + 字体白名单）。
- `tools/test_slim.py`：合成树自测试（无需设备）。

## 流程（首次需设备或拉取树核对名称）

1. **枚举真机清单**（名称以设备为准，修订 manifest）：
   ```bash
   # 设备树在 /mnt/us/koreader；可 rsync/scp 拉取后本机跑，或直接传设备路径
   python3 tools/slim_koreader.py <src> --list-plugins
   python3 tools/slim_koreader.py <src> --list-fonts
   ```
2. **修订 `slim_manifest.json`**：插件白名单核对；字体白名单**必须包含 weread
   样式面板当前正文字体**（doc_settings 持久化的 font_family），否则阅读字体回退默认。
   样式体系是评估报告 §八 的一等公民——字体清单是它的配置面，不得砍掉正在用的字体。
3. **生成精简副本**：
   ```bash
   python3 tools/slim_koreader.py <src> <dst>
   python3 tools/check_slim.py <dst>
   ```
4. **部署（可逆）**：设备上 `mv /mnt/us/koreader /mnt/us/koreader.bak-<date>` 后
   rsync 副本上去，重启 KOReader 冒烟（书架、开书、翻页、样式面板、Menu、Exit）。
   回滚 = 删瘦身后 `mv koreader.bak-<date> koreader` 回来。
5. **真机验收清单**：weread-only 入口直达书 / Reading style 菜单打开 ConfigDialog /
   字号字体行距调整且退出重进保持 / B-3 之外无网络回归 / crash.log 无新栈。

## 禁止

- 直接在设备 `/mnt/us/koreader` 原树上删除（先拉取或副本操作；原树只作回滚锚点）。
- 把 excluded 的 native 库（libwrap-mupdf 等）一并删除——那是 S2 面，Phase 1 不碰。
- 未跑 `check_slim.py` 就部署。
