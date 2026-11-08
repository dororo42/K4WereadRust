# P1-A 集成指南：k4tls 接入 weread_K4.koplugin

状态：**组件就绪，集成补丁待应用**。真机 bench（R4）通过前默认后端 = LuaSec，
设备行为与 v9.7 完全一致；本指南给出全部落地件。

## 架构

```
client.lua (weread_K4.koplugin)
  └─ weread/lib/transport.lua        ← 新增（本仓 lua/transport.lua）：后端选择
       ├─ weread/lib/k4tls.lua       ← 新增（本仓 lua/k4tls.lua）：FFI 绑定+适配
       │    └─ libk4tls.so           ← 本仓 k4tls crate 交叉产物（armel glibc 动态）
       └─ 回退 socket.http（LuaSec） ← 现路径，逐字不动
```

- `client.lua` 唯一改动：`http.request` → `transport.request`（单点，B-3/重试/重定向逻辑零变动）。
- 语义对齐（R2）：k4tls 适配层把 C ABI 错误码映射到 client.lua 既有分类词表——
  `-13`→timeout（F-21 语义：不重试）、`-11`→connection refused、`-10`→host or
  service not provided（ipv4_dns 解析类单次重试）、`-15/-14`→closed（F-23 瞬态
  单次重试）、`-16`→原样 raise "binary response exceeds the N-byte limit"（N-1 有界 sink 同形）。
- 超时注入：client.lua 现有 block/total 秒值 ×1000 传入 C ABI；luasocket 路径忽略新字段。

## client.lua 补丁（唯一 diff）

```lua
-- 顶部 require 区（socketutil 之后）：
local transport = require("weread.lib.transport")

-- request() 内，socketutil:set_timeout 之后：
req_opts.block_timeout = block_timeout
req_opts.total_timeout = total_timeout
req_opts.transport_max_bytes = max_bytes

-- 传输调用点（原 results = { pcall(http.request, req_opts) }）：
results = { pcall(transport.request, req_opts) }
```

`http` 局部变量保留（transport 回退路径仍用它）；`req_opts.max_bytes` 剥离逻辑不变。

## 文件放置（weread_K4.koplugin 仓）

| 源（本仓） | 目标（插件仓） |
|---|---|
| `lua/k4tls.lua` | `weread/lib/k4tls.lua` |
| `lua/transport.lua` | `weread/lib/transport.lua` |
| `k4tls/target/arm-unknown-linux-gnueabi/release/libk4tls.so` | `weread/lib/libk4tls.so`（可选：缺它自动回退 LuaSec） |

`module-map.json` 增补（N-2 门禁）：

```json
"weread/lib/k4tls.lua":   { "specs": ["spec/k4tls_spec.lua"] },
"weread/lib/transport.lua": { "specs": ["spec/transport_spec.lua"] }
```

spec 骨架见本仓 `lua/spec/`（stub 模式：ffi/ko reader 设备模块按
client_redirect_spec.lua 的 preload 模式处理；纯函数测映射，select 测回退）。

## 后端开关（部署后启用方式）

- 环境变量 `WEREAD_TLS_BACKEND=k4tls`（KUAL 脚本/KOReader 环境注入），或
- `weread/lib/transport.lua` 的 `M.backend` 改为 `"k4tls"`（设备 settings.reader.lua
  层面的持久开关可在插件 settings 模块加一项，二阶段做）。
- 缺省 `"auto"`：libk4tls.so 不可加载 → 自动 LuaSec。

## 构建产物（k4tls/build.sh）

| 产物 | target | 用途 | 状态 |
|---|---|---|---|
| `k4tls-bench` | armv7-unknown-linux-musleabihf 静态 | SSH/KUAL 跑真机 TLS 基准（R4 先决件） | ✅ 1.1M 全静态 |
| `libk4tls.so` | arm-unknown-linux-gnueabi 动态（kindle5 交叉 gcc） | KOReader luajit FFI dlopen | 需 rustup armel std + 真机验证 dlopen |

**为什么 .so 不用 musl 静态**：K4 用户态 armel glibc 2.12（k4refresh .cargo/config.toml
记录 + 评估报告 §一 libcrypto armel）；musl 静态 cdylib dlopen 进 glibc 宿主进程
属未验证路径，k4refresh 的 FFI .so 同样计划 armel glibc 重建。bench CLI 走 musl
静态（k4refresh-cli 已实证可跑）。

## 真机验收清单（设备在线后）

1. `k4tls-bench --url https://weread.qq.com/web/ --n 20` —— R4 bench：
   均值/中位数与 LuaSec 路径（改造前的 sync 8s 基线）对比；**性能持平即换**。
2. 部署 libk4tls.so + 两 lua 文件 + client.lua 补丁，`WEREAD_TLS_BACKEND=k4tls`：
   B-3 mock 网关全模式（slow/drop/empty）跑 client spec。
3. 开关回 `auto`（luasec）回归：v9.7 行为零变化。
4. crash.log 监控：FFI 段不得出现新栈（catch_unwind -99 会在 err 字段可见）。
