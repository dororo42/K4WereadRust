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

### K5 修订落地（2026-11-08 深夜）

manifest 已按 K5 实测修订：CJK=`noto/NotoSansCJKsc-Regular.otf`，西文=KOReader 默认 Noto Sans 族（4 文件），另留 mono/symbols 与各保留字体许可证（OFL NOTICE/LICENSE 不丢）；插件白名单加 `SSH.koplugin`（远程恢复入口）。设备 weread 设置与书籍 sdr 均未发现自定义字体键，故按 KOReader 默认字族保留。check_slim 门禁同步接受 `NotoSansCJK*`（RED→GREEN，test_slim 11/11）。**K5 树真实节省估算 ≈ 9.4 MiB（65.7M → ~56M，~14%）**：字体省 ~6.8M、插件省 ~2.6M——K5 树本身只有 64M，与 K4 基线（176M 树）的 115-120M 预期不可混用。实际部署（替换设备树）仍需用户确认。

### K5 部署实录（2026-11-08，已上线）

实际部署未走本机构建+rsync，而是 `tools/k5_slim_deploy.sh` 设备侧四段式（dryrun/execute/rollback/status）：
`mv /mnt/us/koreader → koreader.pre-slim-20261108`（原子改名即完整备份）→ `cp -dpR` 同树副本（settings/凭据、weread/cache 用户数据原样保留）→ 按 manifest 派生清单手术删除（35 插件 + 32 字体，dry-run 先行逐项核对）。execute 有 KOReader 运行拒绝（comm 级检测）、/mnt/us 写保护拒绝、cp 失败自动还原、verify 门禁（keep 集/luajit md5/无泄漏/用户数据在位/luajit 冒烟）；**回滚 = 一条 `rollback` 命令**。实测 **95016→85536 KiB**，VERIFY PASS。

**字体白名单教训（冒烟回归，两轮）**：
1. 首版 manifest 漏了 `freefont/FreeSans.ttf`——KOReader UI 回退链用它渲染 ▸/▼/→ 等几何符号（menu.lua/numberpickerwidget/readersearch/toc），NotoSansCJKsc 无 U+25B8，缺失即菜单符号变方框（crash.log freetype 报错实证）。已恢复 FreeSans.ttf+COPYING（0.9M）并清 `cache/fontlist/fontinfo.dat` 过期缓存。
2. 二轮冒烟：菜单/页脚时间行 ⌚（及 ⏳/⊛/⤟/⤠/⤻/⥖）变方框。设备裸 FFI FreeType 探针（FT_Get_Char_Index 全量矩阵，绕开 bare luajit 下 ffi/freetype.lua 引导失败）实证这些 Unicode 符号区字形在 NotoSans/CJKsc/symbols.ttf/FreeSans 四面全无覆盖——**FreeSerif（回退链 [8]）是它们的唯一提供者**，被首版裁掉。已恢复 FreeSerif.ttf（1.8M，md5 对账备份树一致）并清字体缓存。
校准后结论：**字体白名单必须覆盖 KOReader UI 回退链的全部符号载体——不止正文/CJK，几何符号（FreeSans）与 Unicode 符号区（FreeSerif）都是一等公民**。
体积（du 口径）：fonts 25168→21192 KiB、plugins 4104→1376 KiB，瘦身面净省 ≈6.7M；整树 du 受 weread 数据目录（8.4M 书架缓存，用户数据）增长影响不做整树口径承诺。新发现：`l10n/` 全语言翻译 18.9M 是未来瘦身候选（未经用户确认不裁）。

## 工具

- `tools/slim_koreader.py`：源树 → 精简副本。幂等、永不写源树、产物带 `SLIM_REPORT.json`。
- `tools/check_slim.py`：产物门禁（必需入口、保留插件齐全、排除面无泄漏、CJK 回退字体在位）。
- `tools/slim_manifest.json`：数据驱动清单（插件白名单 + 字体白名单）。
- `tools/test_slim.py`：合成树自测试（无需设备）。
- `tools/k5_slim_deploy.sh`：K5 设备侧部署/回滚脚本（四段式；koreader 检测为 /proc comm 级，规避 busybox `pgrep -f` 命中 dropbear argv0 的自匹配）。

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
