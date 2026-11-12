# K4WereadRust

K4weread Phase 1 工程仓。上游评估：`~/reports/K4weread激进形态评估_KOReader裁剪+weread主程序化+Rust加持-2026-11-08.md`（结论：分期采纳，本仓 = Phase 1 的 S1+S3 解耦部分）。

> 状态基线以 `AGENTS.md` 的 Current Handoff（及其指向的 HANDOFF.md）为**唯一权威**；本文件不再重复写 HEAD，避免 README / AGENTS 双源漂移（2026-11-12 审查 I-3）。
> 下表只作速览；设备与翻闸的最新变化以 `LOG.md` 追加节为准。

## 交付物

| 项 | 状态 | 说明 |
|---|---|---|
| **k4tls**（P1-A） | R4 基准 PASS；2026-11-10 K5 翻闸为常态 k4tls，2026-11-12 因 segfault 归因临时切回 `luasec` 做 A/B（下次 KOReader 重启生效，见 `LOG.md`）；**仓库代码默认后端始终是 LuaSec**，设备实际后端由 env / 菜单决定 | Rustls C ABI + LuaJIT FFI；bench 为 musl 静态件，FFI 库依赖 armel glibc。K5 已通过 dlopen/FFI 往返与受控 TLS 夹具；K4 的 k4tls 链路仍未完整验收。测试数量与验收范围以 `LOG.md` 最新记录为准。 |
| **配置级瘦身**（P1-B） | K5 已部署上线（可回滚），两轮字形回补完成 | 纯打包层不改源树。瘦身面净省约 6.7M；字体白名单须覆盖 UI 回退链全部符号载体（FreeSans/FreeSerif）。报告中的 K4 体积预期不可直接套用 K5。 |

## 快速上手

```sh
# Rust 测试与交叉产物：由 GitHub Actions 产出（.github/workflows/ci.yml），本机不手工构建
gh run list --repo dororo42/K4WereadRust    # CI 运行状态；产物在各 run 的 artifacts
# 注意：CI artifact ≠ Release 资产。插件仓消费的是 libk4tls-v0.1.0 Release；
# 资产指纹、来源与跨仓兼容矩阵见 docs/RELEASE_ASSETS.md

# 本机允许的源码级检查与自测（与 CI 用同一 5.1 命令名）
lua5.1 tools/test_k4tls_lua.lua
python3 -B tools/test_slim.py
luac5.1 -p $(git ls-files '*.lua')
sh -n k4tls/build.sh tools/k5_slim_deploy.sh
python3 tools/check_lua_mirror.py --peer-repo ../weread_K4.koplugin

# 瘦身（需设备树或拉取树）
python3 tools/slim_koreader.py <src> --list-plugins   # 先核对清单
python3 tools/slim_koreader.py <src> <dst>
python3 tools/check_slim.py <dst>
```

**构建纪律**：Rust/APK/EXE 等编译产物不在本机产出；需要验证构建时改 workflow 配置并推送，由 GitHub Actions 产出。CI 已上线（F-09 关闭，2026-11-09 首绿 run 42629289540）：`.github/workflows/ci.yml` 三 job——host-gates（cargo test/clippy/Lua/Python/shell 门禁）、bench-musleabihf（musl 静态 k4tls-bench）、so-armel-glibc（armel glibc libk4tls.so）；rustc 固定 1.98.1，交叉工具链取本仓 release `cross-toolchains` 资产并 sha256 固化（musl.cc 直连不可用，2026-11-09 实证宕机；资产为官方包原样）。

2026-11-12 三仓审查后：CI 增加 `pull_request` 触发、`permissions: contents: read`、checkout 固定 SHA、跨仓 Lua 镜像门禁（`tools/check_lua_mirror.py`）与非阻断 `cargo fmt --check` 观察步骤。本机 `cargo fmt --check` 仍有 6 处历史格式差异（`http.rs` / `lib.rs` / `transport.rs` / `url.rs`），故暂不升级为硬门禁——待单独格式化批次后再改。

## 文档

- `docs/INTEGRATION.md` —— k4tls 接入 weread_K4.koplugin 的全部落地件（client.lua 单点 diff、双产物线依据、真机验收清单）
- `docs/RELEASE_ASSETS.md` —— Release 资产指纹、跨仓兼容矩阵、依赖与同步顺序（2026-11-12 新增）
- `docs/SLIMMING.md` —— 瘦身流程、体积校准、禁止事项
- `LOG.md` —— 追加式动作日志（ub-mode Practice）
- `AGENTS.md` —— 接手 agent 必读（工作法 + 项目事实 + 交接指针）

## 相关仓库

- `~/weread_K4.koplugin`（已发版 v9.12；v9.13 候选未发版）——`client.lua` 集成补丁已合入。**`lua/k4tls.lua` 与 `lua/transport.lua` 两个绑定文件与插件仓 `weread/lib/` 逐字节一致**，由 `tools/check_lua_mirror.py` 机器校验；`lua/spec/` 因本仓用 `test_k4tls_lua.lua`、插件仓用 busted 而各自维护，只做语义对齐，**不**逐字节一致（2026-11-12 审查 I-1 更正）。仓库默认后端 LuaSec，设备后端由 env / 菜单决定。
- `~/K4weread`——KUAL 扩展侧（v0.1.2；与本仓无代码契约，仅经插件仓间接受资产影响）
- `~/work/k4refresh`——Rust-on-K4 构建配方先例（本仓交叉配置的依据来源）
