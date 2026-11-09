# K4WereadRust

K4weread Phase 1 工程仓。上游评估：`~/reports/K4weread激进形态评估_KOReader裁剪+weread主程序化+Rust加持-2026-11-08.md`（结论：分期采纳，本仓 = Phase 1 的 S1+S3 解耦部分）。

> 状态核对基线：`main @ c5462e4`（含 2026-11-09 独立审查修复批次 `27c8af3`）。
> 权威状态以 `AGENTS.md` 的 Current Handoff 为准；下表只作速览。

## 交付物

| 项 | 状态 | 说明 |
|---|---|---|
| **k4tls**（P1-A） | R4 传输基准完成（K5 真实端点 30/30）；默认后端仍为 LuaSec，翻闸待集成阶段 | Rustls C ABI + LuaJIT FFI；bench 为 musl 静态件，FFI 库依赖 armel glibc。K5 已通过 dlopen/FFI 往返与受控 TLS 夹具；K4 真机延后。测试数量与验收范围以 `LOG.md` 最新记录为准。 |
| **配置级瘦身**（P1-B） | K5 已部署上线（可回滚），两轮字形回补完成 | 纯打包层不改源树。瘦身面净省约 6.7M；字体白名单须覆盖 UI 回退链全部符号载体（FreeSans/FreeSerif）。报告中的 K4 体积预期不可直接套用 K5。 |

## 快速上手

```sh
# Rust 测试与交叉产物：由 GitHub Actions 产出（.github/workflows/ci.yml），本机不手工构建
gh run list --repo dororo42/K4WereadRust    # CI 运行状态；产物在各 run 的 artifacts

# 本机允许的源码级检查与自测
lua tools/test_k4tls_lua.lua
python3 -B tools/test_slim.py
luac -p $(git ls-files '*.lua')
sh -n k4tls/build.sh tools/k5_slim_deploy.sh

# 瘦身（需设备树或拉取树）
python3 tools/slim_koreader.py <src> --list-plugins   # 先核对清单
python3 tools/slim_koreader.py <src> <dst>
python3 tools/check_slim.py <dst>
```

**构建纪律**：Rust/APK/EXE 等编译产物不在本机产出；需要验证构建时改 workflow 配置并推送，由 GitHub Actions 产出。CI 已上线（F-09 关闭，2026-11-09 首绿 run 42629289540）：`.github/workflows/ci.yml` 三 job——host-gates（cargo test/clippy/Lua/Python/shell 门禁）、bench-musleabihf（musl 静态 k4tls-bench）、so-armel-glibc（armel glibc libk4tls.so）；rustc 固定 1.98.1，交叉工具链取本仓 release `cross-toolchains` 资产并 sha256 固化（musl.cc 直连不可用，2026-11-09 实证宕机；资产为官方包原样）。

## 文档

- `docs/INTEGRATION.md` —— k4tls 接入 weread_K4.koplugin 的全部落地件（client.lua 单点 diff、双产物线依据、真机验收清单）
- `docs/SLIMMING.md` —— 瘦身流程、体积校准、禁止事项
- `LOG.md` —— 追加式动作日志（ub-mode Practice）
- `AGENTS.md` —— 接手 agent 必读（工作法 + 项目事实 + 交接指针）

## 相关仓库

- `~/weread_K4.koplugin`（v9.8 设备运行中）——client.lua 集成补丁已合入（b82d7ed，k4tls/transport 双副本与本仓逐字节一致，默认关）
- `~/K4weread`——KUAL 扩展侧
- `~/work/k4refresh`——Rust-on-K4 构建配方先例（本仓交叉配置的依据来源）
