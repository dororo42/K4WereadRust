# K4WereadRust

K4weread Phase 1 工程仓。上游评估：`~/reports/K4weread激进形态评估_KOReader裁剪+weread主程序化+Rust加持-2026-11-08.md`（结论：分期采纳，本仓 = Phase 1 的 S1+S3 解耦部分）。

## 交付物

| 项 | 状态 | 说明 |
|---|---|---|
| **k4tls**（P1-A） | ✅ 组件就绪 / ⏳ 设备 bench | rustls 静态传输组件：C ABI + LuaJIT FFI 替换 weread client.lua 的 LuaSec/OpenSSL 传输层。rustls+ring+webpki-roots 零系统依赖；28 host 测试 + 375 插件 spec 全绿；已"接线不翻闸"（默认 LuaSec，`WEREAD_TLS_BACKEND=k4tls` 启用，R4 bench 通过前不翻默认） |
| **配置级瘦身**（P1-B） | ✅ 工具就绪 / ⏳ 真机核对 | 插件 35→3 + 字体裁剪，纯打包层不改代码；176.3M→预期 ~115-120M（S1 范围诚实校准） |

## 快速上手

```sh
# k4tls（host 测试 + 交叉构建）
cd k4tls && cargo test && ./build.sh          # bench(musl 静态) + .so(armel glibc)

# Lua 绑定纯函数测试
lua tools/test_k4tls_lua.lua

# 瘦身（需设备树或拉取树）
python3 tools/slim_koreader.py <src> --list-plugins   # 先核对清单
python3 tools/slim_koreader.py <src> <dst>
python3 tools/check_slim.py <dst>
```

## 文档

- `docs/INTEGRATION.md` —— k4tls 接入 weread_K4.koplugin 的全部落地件（client.lua 单点 diff、双产物线依据、真机验收清单）
- `docs/SLIMMING.md` —— 瘦身流程、体积校准、禁止事项
- `LOG.md` —— 追加式动作日志（ub-mode Practice）
- `AGENTS.md` —— 接手 agent 必读（工作法 + 项目事实 + 交接指针）

## 相关仓库

- `~/weread_K4.koplugin`（v9.7 设备运行中）——client.lua 集成补丁已合入（d644920，默认关）
- `~/K4weread`——KUAL 扩展侧
- `~/work/k4refresh`——Rust-on-K4 构建配方先例（本仓交叉配置的依据来源）
