# LOG — K4WereadRust

追加式动作日志。每条：日期时间、动作、原因、验证。

## 2026-11-08 17:33 — 项目创建

- 动作：创建本仓（git init），AGENTS.md/.gitignore/README/LOG，目录骨架 k4tls|lua|tools|docs。
- 原因：落实评估报告 §七 Phase 1（rustls 组件 + 配置级瘦身），与 K4weread（KUAL 侧）、weread_K4.koplugin（插件侧）分仓。
- 验证：目录树就位；工作法（ub-mode/orbital_handoff）写入 AGENTS.md。

## 2026-11-08 17:30 前置核实（报告与资源）

- 动作：核实评估报告全文；确认 K4weread v9.7 交接文档「立即下一步 #3」即本仓任务。
- 事实：rustup@~/.cargo/bin + armv7-musleabihf std 已装（k4refresh 先例）；本机无 busted（plugin 368 spec 在 CI 跑）；本机无本地 KOReader 树（瘦身工具按设备树/拉取树设计）；x-tools 仅有 gnu-eabi 交叉 gcc，musl 交叉 C 编译器待下载（ring 需要）。
- 更正：busted/luacheck 实际在 ~/.luarocks/bin 可用（17:55 发现），插件门禁已全量本地执行。

## 2026-11-08 17:45 — P1-B 配置级瘦身工具落地

- 动作：slim_koreader.py（幂等打包裁剪：插件白名单 35→3 + 字体裁剪，SLIM_REPORT.json）+ check_slim.py 门禁 + slim_manifest.json + test_slim.py 合成树自测 + docs/SLIMMING.md（体积诚实校准：S1≈115-120MB，报告 §七 ~70M 需叠加 S2）。
- 原因：评估报告 §二 S1/§3.1——纯打包层不碰代码，独立可交付。
- 验证：test_slim PASS（排除计数/保留集/报告/门禁/幂等守卫/源树未动）。真机清单核对与部署 blocked（设备间歇在线）。

## 2026-11-08 18:05 — P1-A k4tls crate（Rust 核心）

- 动作：rustls 0.23+ring+webpki-roots 单请求同步传输（IPv4 DNS→TCP→TLS→HTTP/1.1，content-length/chunked/EOF，max_bytes 硬上限，block/total 双预算对齐 socketutil），C ABI（catch_unwind -99、错误码族 -10…-17、trust_pem 可选 fail-closed）、k4tls-bench CLI。
- 关键修复（测试驱动暴露）：getaddrinfo 返回的 sin_port=0 未替换目标端口（连接打到 0 端口）；坏 PEM 静默零证书改 fail-closed 显式报错；bench 的 c_char 在 ARM 上是 u8（host 编过、交叉编不过的隐性平台差）。
- 验证：cargo test 28 绿（12 单测 + 6 FFI 层 + 10 hermetic TLS 端到端）、clippy 零告警、k4tls-bench 真站冒烟 weread.qq.com 200 OK 3/3、交叉产物 1.1M ARM EABI5 全静态。

## 2026-11-08 18:25 — P1-A Lua 绑定 + 插件仓集成（接线不翻闸）

- 动作：lua/k4tls.lua（FFI 绑定 + socket.http 形状适配 + 错误码映射到 client.lua 重试分类词表）+ lua/transport.lua（WEREAD_TLS_BACKEND/luasec/auto 三态选择）；插件仓 client.lua 三点最小 diff 接入 transport，module-map + 两 spec 增补；docs/INTEGRATION.md 全套落地件。
- 关键事实：K4 用户态 armel glibc 2.12（k4refresh .cargo/config.toml + 报告 §一）→ FFI .so 必须走 armel glibc 交叉（kindle5 工具链），musl 静态 cdylib dlopen 进 glibc 宿主属未验证路径；bench CLI 走 musl 静态（已实证）。
- 关键修复：luacheck 发现 cdef() 定义后未调用（FFI 声明不注册的真 bug）。
- 验证：插件仓全量门禁 busted 375/0/0、luacheck 0 告警、luac -p OK、module-map 49 模块 0 孤儿；两仓各自提交（插件仓 d644920 默认关零行为变更）。

## 2026-11-08 18:40 — 未决与 blocked

- armel std 下载中（rustup 走代理慢）→ libk4tls.so 交叉构建待其完成（后台任务）。
- 设备侧全部 blocked：k4tls-bench 真机 R4 bench、libk4tls.so dlopen 验证、瘦身清单真机核对与部署（设备间歇在线）。
