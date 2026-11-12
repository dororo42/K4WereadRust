# P1-A 集成指南：k4tls 接入 weread_K4.koplugin

状态（2026-11-12 更新）：**插件仓已接线；R4 基准 PASS；2026-11-10 K5 翻闸为常态 k4tls；2026-11-12 因 segfault 归因，K5 的 `WEREAD_TLS_BACKEND` 临时改回 `luasec` 做 A/B（下次 KOReader 重启生效，见 `LOG.md` 与 `K4weread/LOG.md`）**。
历史证据：R4（2026-11-08 K5 真实端点 30/30，中位 546.6 vs 643.1 ms）、登录态完整业务验收（2026-11-09 K5，实录见「结论边界」）。
**仓库代码默认后端始终是 LuaSec**（`weread/lib/transport.lua` 的 `M.backend`）；设备实际后端由 `WEREAD_TLS_BACKEND` 或插件菜单勾选（v9.13 候选）决定，菜单显式选择优先于 env。
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

2026-11-12 审查（I-1 / I-7）后按**实际方向**重写（原先误把本仓 `target/` 产物列为分发源）：

| 方向 | 源 | 目标 | 说明 |
|---|---|---|---|
| Lua 绑定 | **插件仓** `weread/lib/k4tls.lua`、`weread/lib/transport.lua` | 本仓 `lua/k4tls.lua`、`lua/transport.lua` | 插件仓是唯一生产来源，本仓为镜像；`tools/check_lua_mirror.py` 校验两个绑定文件逐字节一致（spec 因测试框架不同只做语义对齐） |
| .so 资产 | K4WereadRust Release `libk4tls-v0.1.0/libk4tls.so` | 插件包内 `weread/lib/libk4tls.so` → 设备插件目录 | 由插件仓 `release.yml:31-36` 打包前匿名下载 + sha256 固化校验，`release.yml:43-44` 断言入包；**不再**从本机 `k4tls/target/` 拷贝。完整指纹与同步顺序见 `docs/RELEASE_ASSETS.md` |

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

> 协调项（2026-11-08 提出，**2026-11-12 已修复**，审查项 I-5）：插件仓
> `weread/lib/client.lua` 的「k4tls 传输」注释曾写「默认后端 auto / R4 通过前
> 不翻默认」，与 `transport.lua` 的 `backend = "luasec"` 不符（d644920 初版旧文）。
> 11-12 已改为实际语义：默认 luasec、只有 auto/k4tls 探测库、菜单显式选择
> 优先于部署 env。保留本条便于追溯，勿再按旧注释理解后端选择。

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
- CI 门禁（2026-11-09，F-09 关闭）：`.github/workflows/ci.yml` 三 job 全绿（run 42629289540）——host-gates（cargo test --locked / clippy -D warnings / luac -p + lua5.1 全量 / test_slim.py / sh -n）、bench-musleabihf（静态 k4tls-bench 1,135,568 B）、so-armel-glibc（libk4tls.so 1,186,764 B，ELF armel 动态）。rustc 固定 1.98.1；交叉工具链取本仓 release `cross-toolchains` 资产（musl.cc 官方包原样 + koxtoolchain armel）并 sha256 固化（musl.cc 直连 2026-11-09 实证宕机）。CI 版 .so（sha256 edb6dc76…）含 27c8af3/2d25c33 语义修复，与设备现存修复前 .so（md5 978b5739…）尺寸相同内容不同。**已替换（2026-11-09，用户授权）**：设备副本现为 CI 产物（md5 8b89393a…），替换前版本备份于同目录 `libk4tls.so.pre-ci`；设备 LuaJIT 生产路径探针通过（dlopen + version 串正常；CI 产物符号版本上限 GLIBC_2.12 与设备匹配）。
- 补记（2026-11-10）：插件仓发版 v9.10（K5 全菜单 TouchMenu + QR 物理键提示条，898dde8）与 v9.11（上游 v1.7.0 QR 健壮性 U17-1/U17-2，66ca115，Latest，run 42825530616）；本仓 lua 镜像随之再对齐——`lua/k4tls.lua` 换为插件仓 aa24b9c5（内置库优先：ffi.load 先试模块同目录 libk4tls.so 再试全局名 "k4tls"，防系统同名旧库静默遮蔽；设备上为可证明 no-op），spec 加载顺序断言对调后 host 门禁 67/67 绿。设备浸泡三件套冻结不动（k4tls.lua 30a9db1a / transport.lua 6346dd22 / .so 8b89393a）——浸泡证据链以开始时组合为准，shim 漂移 aa24b9c5 列为翻闸后动作；设备运行时 v9.11 差集（qr_login.lua/_meta/main）由持久监视器外科手术式同步（.pre-v911 备份 + SOAK-INTACT 三件套完整性校验）。
- 补记（2026-11-11）：**翻闸落地**——R4 bench PASS 后用户批准，koreader.sh 注入 `WEREAD_TLS_BACKEND=k4tls` 为常态（插件仓并行批次执行）；设备 shim 升 aa24b9c5（内置库优先生效：加载一行化，裸名失败尝试消失）；生产证据一手核实（crash.log）：全日志 0 真实 k4tls 错误，11-10 23:57 会话全链（-2012 旧模式 → account_cleared → 弱网超时被 U17-1 接住 → 登录完成 → relogin 重启 → 上报成功）；K5 运行时 v9.12 与仓/Release 零差集；K4（B00E）v9.12 全量部署（无 .so，LuaSec 默认），用户实测通过。
- **K4 k4tls 口径统一（2026-11-12）**：上文 `:140` 记 11-11 K4「无 .so，LuaSec 默认」；`K4weread/LOG.md:163-164` 记同日稍晚已从 K5 拷入 .so 并完成 k4tls/LuaSec bench 对照（0.833s vs 0.921s），本仓 `LOG.md:193` 亦记 K4 = v9.12 全量。两者是**同一天的时间线前后**，不是矛盾：11-11 早间无 .so → 当日稍晚拷入并跑 bench。但「K4 真机 k4tls 完整业务链路」仍未按本节验收清单验证，仍属未验证；最新状态以各仓 `LOG.md` 追加节为准。
- 未验证：KOReader 应用内非传输层的其余行为；D11 两观察点登记中（用户日常：续期路径脚标同步消失 / 无效 skey 报错路径）。

## 三仓协同（2026-11-12 审查后）

| 链 | 方向 | 同步要求 |
|---|---|---|
| 握手标记 | K4weread `kual/bin/k4weread.sh`（写）↔ 插件仓 `weread/lib/only_mode.lua`（读+删） | 路径字面量必须同批次改；K4weread `tools/check_marker_path.py --peer-repo` 与插件仓 `spec/only_mode_spec.lua` 双向钉死 |
| Lua 绑定 | 插件仓 `weread/lib/` → 本仓 `lua/` | 插件仓为源、本仓跟随；`tools/check_lua_mirror.py` 机器校验 |
| .so 资产 | 本仓 CI / Release → 插件仓 workflow → 设备插件目录 | 见 `docs/RELEASE_ASSETS.md` §4 |
| 版本 | 三仓独立编号，无机器绑定 | 见 `docs/RELEASE_ASSETS.md` §3 兼容矩阵 |

本仓与 K4weread **无直接代码契约**。三仓改动范围、依赖关系与同步顺序的完整记录见三仓各自 HANDOFF.md 的「三仓协同」节。
