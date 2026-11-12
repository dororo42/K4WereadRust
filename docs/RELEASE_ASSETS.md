# Release 资产与跨仓兼容矩阵（libk4tls）

> 2026-11-12 三仓审查（I-4 / F-10）落地：把散落在 workflow、LOG、交接文档中的资产指纹与版本组合收敛到一份可核对清单。
> **本文件只记录已核实的事实及其出处；本次未联网核验远端资产，不构成对远端状态的证明。**

## 1. 资产清单

| 项 | 值 | 出处 |
|---|---|---|
| Release / tag | `libk4tls-v0.1.0` | `K4weread/LOG.md:170`、本仓 `LOG.md:200` |
| 资产文件名 | `libk4tls.so` | 插件仓 `.github/workflows/ci.yml:87`、`release.yml:34` |
| 下载 URL | `https://github.com/dororo42/K4WereadRust/releases/download/libk4tls-v0.1.0/libk4tls.so` | 同上 |
| 完整 sha256 | `edb6dc76849861b464e86c3422390841b9b30cbe8d59822c9dde372619408fde` | 插件仓 `ci.yml:88`、`release.yml:35`（固化校验值） |
| 8 位前缀记录 | `edb6dc76…` | 本仓 `LOG.md:146,201`、`docs/INTEGRATION.md:138` |
| 产出来源 | CI run `42629289540` 的 `so-armel-glibc` job | 本仓 `docs/INTEGRATION.md:138`、`LOG.md:146,200` |
| 设备现役 .so（记账） | md5 `8b89393a…` | 本仓 `LOG.md:153,200`、`K4weread/LOG.md:170` |
| 目标 ABI | `arm-unknown-linux-gnueabi` / armel glibc 2.12.1 / 1,186,764 B | 本仓 `.github/workflows/ci.yml` so-armel-glibc、`docs/INTEGRATION.md:85-88` |

## 2. 本机 `target/` 产物不是该资产（如实登记）

本机 `k4tls/target/arm-unknown-linux-gnueabi/release/libk4tls.so`（mtime 2026-11-08 19:03）实测：

- sha256 `164a13806495a360b9670f863ddc9b67f0fa4ee6085cae6b89a519c404ac951d`
- md5 `978b57392d85770f7b132d93ff363ada`

`978b5739…` 与本仓 `docs/INTEGRATION.md:137`、`LOG.md:136,153` 记载的「设备修复前版 / 与 R4 基准同一构建」一致；CI 版（含 `27c8af3`/`2d25c33` 语义修复）是另一个文件（`INTEGRATION.md:138`）。因此本机 target 产物是**修复前版**，既不是 Release 资产，也不等于设备现役版——三者不同是已被日志记录的事实，不是异常。

> **复核更正**：`~/reports/2026-11-12-K4weread-三仓代码审查与项目一致性核查报告.md` 的 I-4 曾写「Rust 仓内没有记录 `edb6dc76…` 这个 sha256」。实际本仓 `LOG.md:146,201` 与 `docs/INTEGRATION.md:138` 以 8 位前缀记录了它，是审查时只按 64 位完整值检索导致的漏检；报告已更正，结论（仓内无条目化清单、无 Release 产出 workflow）不受影响。

## 3. 跨仓兼容矩阵（2026-11-12 工作树快照）

| 仓 | 版本 / 标识 | 位置 |
|---|---|---|
| K4weread（KUAL 扩展） | `v0.1.2` | `kual/config.xml:11` |
| weread_K4.koplugin（插件） | `0.6.0-k4-v9.12` 已发版；`v9.13` 未发版候选 | `_meta.lua:9` / `main.lua:35` / `ChangeLog.md` `[Unreleased]` |
| K4WereadRust（crate） | `k4tls 0.1.0` | `k4tls/Cargo.toml:3`、`Cargo.lock:52` |
| Release 资产 | `libk4tls-v0.1.0` | 本文件 §1 |

四套编号相互独立，**没有机器可校验的跨仓版本绑定**；本表是人工维护的兼容快照，任一版本变化须同步更新。注意 `v0.1.2`（扩展）与 `0.1.0`（crate / 资产）同形异义。

## 4. 依赖链与同步顺序

1. 本仓 `.github/workflows/ci.yml` 的 `so-armel-glibc` job 产出 `libk4tls.so` 作为 **CI artifact**（`actions/upload-artifact@v4`）。
2. `libk4tls-v0.1.0` Release 及其资产在 GitHub 侧创建。仓内 workflow **不产出 Release**（`.github/workflows/` 只有 `ci.yml`），本地 `git tag -l` 为空；LOG.md:200 记该 Release「仅 release 资产、无代码变更」。该步骤未见仓内可复现配置，属仓外动作。
3. 插件仓 `ci.yml:85-88`（每次 push 的资产哨兵）与 `release.yml:31-36`（打包前 fetch）按完整 sha256 校验后使用。
4. 插件仓 `release.yml:43-44` 断言 zip 内含 `weread_K4.koplugin/weread/lib/libk4tls.so`。
5. 插件 zip 部署到设备；K4weread 不引用该资产（两仓无代码契约）。

**资产更新顺序**：本仓产出新 .so → 更新本文件 §1 → 同步插件仓两处 workflow 的 URL/sha256 → 打 tag 前确认插件仓 `ChangeLog.md` 的 `[Unreleased]` 已重命名为 `## [vX] - YYYY-MM-DD` → 由 workflow 出包。

**本批修复未执行任何发布动作**：不打 tag、不上传资产、不 push、不构建。

## 5. 未验证项

- 未下载远端 Release 资产，未核对其实际 sha256 是否仍等于 §1 的固化值。
- 未查询 GitHub Actions run；文中的 run id 均为仓内文档记载。
- 未在本机重新构建 .so（构建纪律：编译产物一律由 GitHub Actions 产出）。
