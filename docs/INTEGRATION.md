# P1-A 集成指南：k4tls 接入 weread_K4.koplugin

状态：**插件仓已接线；R4 传输基准已完成（2026-11-08 K5 真实端点 weread.qq.com
30/30，中位 k4tls 546.6 vs LuaSec 643.1 ms）；登录态完整业务验收已通过（2026-11-09
K5 真实 KOReader 内 k4tls 全程驱动 ProgressSync/ReadReport，实录见「结论边界」）**。
默认后端保持 LuaSec；验收经由显式 `WEREAD_TLS_BACKEND=k4tls` 注入完成，验收后已移除恢复默认。
K5 的测试结果不替代 K4 验收。

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
  单次重试）、`-16`→`(nil, "binary response exceeds the N-byte limit")`（由 client 统一抛错）。
  `-18`（非法 URL）/`-19`（请求构造错误）是确定性参数失败：文本不含上述词表关键词，
  也不拼接 Rust 侧 detail，避免确定性错误被 client 当成可重试故障反复消耗预算。
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

`client.lua` 不再需要 `http` 局部变量；LuaSec 回退由 transport 模块负责。
`req_opts.max_bytes` 剥离逻辑不变。有界 sink 接受结束标记 `nil` 并返回 `1`；
所有传输失败必须返回 `(nil, err)`，不能返回 `(1, nil, err)`，否则会绕过既有重试分类。

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
- 缺省 `"luasec"`；未知值同样回到 LuaSec。显式 `"auto"` 或 `"k4tls"` 才尝试加载候选库。
- 回滚必须显式设置 `WEREAD_TLS_BACKEND=luasec`；`auto` 不是回滚开关。

> 协调项（2026-11-08 双仓同步核查）：插件仓 `weread/lib/client.lua` 中
> "k4tls 传输" 注释仍写"默认后端 auto"——那是 d644920 初版设计的旧文；
> b82d7ed 同步后的实际语义以 `transport.lua` 为准（默认 luasec）。该注释
> 在插件仓（本仓只读，且有并行在制品），翻闸集成阶段一并修正。

## 构建产物（k4tls/build.sh）

| 产物 | target | 用途 | 状态 |
|---|---|---|---|
| `k4tls-bench` | armv7-unknown-linux-musleabihf 静态 | SSH/KUAL 跑真机 TLS 基准（R4 先决件） | K5 可执行；公开测试 URL 返回 500，不是成功基准样本 |
| `libk4tls.so` | arm-unknown-linux-gnueabi 动态（kindle5 交叉 gcc） | KOReader luajit FFI dlopen | K5 glibc 2.12.1 已通过加载与版本调用，K4 待验 |

**为什么 .so 不用 musl 静态**：K4 用户态 armel glibc 2.12（k4refresh .cargo/config.toml
记录 + 评估报告 §一 libcrypto armel）；musl 静态 cdylib dlopen 进 glibc 宿主进程
属未验证路径，k4refresh 的 FFI .so 同样计划 armel glibc 重建。bench CLI 走 musl
静态（k4refresh-cli 已实证可跑）。

## 真机验收清单（设备在线后）

1. 先在设备临时目录加载库并调用版本，再发真实请求。GLIBC 版本号合格不能证明可加载：
   原产物的无版本强依赖 `getauxval` 已在 K5 失败；现在用受限 ARM/glibc 包装读取 `/proc/self/auxv`，
   构建加入 `-z defs` 拒绝缺失强符号。不得部署替换系统 libc。
2. R4 使用相同设备、URL/请求头/请求体、证书验证、超时及负载交替测试两个后端，记录成功率、正文校验、
   均值/中位数/p95。单个 GET 不能与完整同步流程的 8 秒比较；HTTP 500 或只有失败的样本不通过 R4。
3. 先在独立进程运行 `tools/device_transport_check.lua`，配合本机 `tools/tls_fixture.py`，
   仅通过 SSH 反向转发访问回环 TLS 服务。测试 CA 只注入测试调用，绝不关闭证书验证或安装到系统。
   该夹具检查正文含 NUL、chunked、interim、HEAD、304、空响应、500、断连、超时、限额及不可信 CA；
   其耗时只代表受控 TLS1.2 夹具，不替代真实业务或 K4 的 R4 验收。
4. B-3 原网关只提供 HTTP，不能直接宣称覆盖 k4tls 的 HTTPS 路径；本机 B-3 继续守护 LuaSec，
   HTTPS 夹具是独立的传输回归。完整 client 业务集成仍需真实场景验收。
5. 回滚开关为 `WEREAD_TLS_BACKEND=luasec`；默认保持 LuaSec，直到性能与业务回归均通过。
6. 设备 UI、样式面板及故障日志的验收单独进行；独立 LuaJIT 进程测试不能代替正在运行的 KOReader UI。

## 受控 TLS 夹具复现命令（K5 已验证，-joff）

```sh
# 1. 本机一次性证书（严格校验要求 CA 与叶子都带 keyUsage，缺一 Python3.14 拒绝）
mkdir /tmp/k4tls-https && cd /tmp/k4tls-https
openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 3 \
  -subj "/CN=k4tls-test-ca" \
  -addext "basicConstraints=critical,CA:TRUE" \
  -addext "keyUsage=critical,keyCertSign,cRLSign"
openssl req -newkey rsa:2048 -nodes -keyout server.key -out server.csr -subj "/CN=localhost"
printf 'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost,IP:127.0.0.1\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid,issuer\n' > leaf.ext
openssl x509 -req -in server.csr -CA ca.pem -CAkey ca.key -CAcreateserial \
  -out server.pem -days 3 -sha256 -extfile leaf.ext
# 2. 本机夹具（仅绑定回环）+ 设备端 harness（经 SSH -R 反向转发，各自会话自持转发）
python3 tools/tls_fixture.py --cert /tmp/k4tls-https/server.pem --key /tmp/k4tls-https/server.key &
# 限时用本机 timeout 包整条 ssh；设备端禁用 timeout（该机 busybox timeout applet 会段错误，见下）
timeout 300 sshpass -p '' ssh -p 2222 -o ExitOnForwardFailure=yes -R 127.0.0.1:18474:127.0.0.1:18474 \
  root@192.168.2.157 \
  'cd /mnt/us/koreader && LD_LIBRARY_PATH=/mnt/us/koreader/libs ./luajit /tmp/k4tls-check-*/tools/device_transport_check.lua \
   /tmp/k4tls-check-*/lua https://localhost:18474 /tmp/k4tls-check-*/ca.pem 10'
```

- 基址用 `https://localhost`（非 127.0.0.1）才会经过 k4tls 的 DNS worker 路径；证书 SAN 已含 `DNS:localhost`。
- 裸跑设备 luajit **必须带 `LD_LIBRARY_PATH=/mnt/us/koreader/libs`**，否则 `ssl.core` 找不到 `libssl.so.60`、LuaSec 不可用（luasec 臂全部失败）。KOReader 的 `socket.http` 对 https 基址会委托 `ssl.https`，这是 luasec 臂经 `http.request` 也能走 TLS 的机制。
- 独立进程 JIT-on 与 `-joff` 均实测正常（2026-11-08 晚各 5/5 rc=0）；早前"JIT-on 纯启动必崩"结论系设备端 `timeout` 段错误假象。正式 KOReader 应用内行为仍需单独验收。
- **设备端不要用 `timeout` 包裹命令**：该机 busybox 的 `timeout` applet 稳定段错误（EXIT 139，2026-11-08 实录 `timeout 5 /bin/true` 复现，曾致一连串"exec 普遍段错误/JIT-on 必崩"的误诊）。需要限时就在本机 `timeout N ssh …` 包整条会话。终止本地 ssh 前仍要确认远端进程退出（末尾 `pgrep` 检查），避免残留进程占满单核设备。

## 结论边界（2026-11-08 K5，2026-11-09 复核）

- 已验证：armel .so 在 glibc 2.12.1 LuaJIT 的 dlopen、FFI 往返、全部传输失败路径语义、限额/超时/证书校验、持久 DNS worker 生命周期（nodelete）、约 560 请求零失败（传输链 + 浸泡 + 基准）；受控负载基准（16K/128K/512K × 15 × 双后端交替，回环 TLS1.2）：k4tls 中位 43.1/118.9/367.6 ms vs LuaSec 106.0/188.4/434.8 ms。
- 后续批次补记（2026-11-08 深夜～2026-11-09，详见 `docs/SLIMMING.md` 与 `AGENTS.md`）：**R4 正式业务基准已完成**（真实 weread.qq.com，双后端交替 30/30、正文等长 525,412 B，中位 k4tls 546.6 vs LuaSec 643.1 ms）；**P1-B 瘦身已在 K5 部署上线且可回滚**（`sh tools/k5_slim_deploy.sh rollback`，备份树 `koreader.pre-slim-20261108`）。
- 验收补记（2026-11-09 13:18）：**登录态完整同步验收 PASS**——注入 `WEREAD_TLS_BACKEND=k4tls` 后全新启动 KOReader（插件 v9.8），crash.log 实录 `ffi.load: k4tls`（裸名失败）→ `ffi.load: plugins/weread_K4.koplugin/weread/lib/libk4tls.so`（插件同目录命中，生产形态位置；这两行本身即 env 传播与 `is_available()` 被调用的反证），加载 1 秒内 `ProgressSync verified: positions_match` + `read report success: has_synckey=true`，零传输错误、无回退；.so md5 `978b5739…` 与 R4 基准同一构建。边界：`/proc/<pid>/maps` 驻留快照未及采集（同步后 KOReader 即退出），以加载成功 + 成功请求链替代。验收后注入已移除，设备恢复默认 LuaSec（koreader.sh 与备份 `koreader.sh.pre-k4tls-accept` md5 一致，备份留设备作审计）。
- CI 门禁（2026-11-09，F-09 关闭）：`.github/workflows/ci.yml` 三 job 全绿（run 42629289540）——host-gates（cargo test --locked / clippy -D warnings / luac -p + lua5.1 全量 / test_slim.py / sh -n）、bench-musleabihf（静态 k4tls-bench 1,135,568 B）、so-armel-glibc（libk4tls.so 1,186,764 B，ELF armel 动态）。rustc 固定 1.98.1；交叉工具链取本仓 release `cross-toolchains` 资产（musl.cc 官方包原样 + koxtoolchain armel）并 sha256 固化（musl.cc 直连 2026-11-09 实证宕机）。CI 版 .so（sha256 edb6dc76…）含 27c8af3/2d25c33 语义修复，与设备现存修复前 .so（md5 978b5739…）尺寸相同内容不同；CI 产物替换设备副本属用户决策。
- 未验证：K4 真机（用户决策延后）、正式插件内翻默认后端（前置：插件仓双副本 lua 同步 + 用户决策；Rust 编译验证已过）、KOReader 应用内非传输层的其余行为。
