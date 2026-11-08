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

## 2026-11-08 19:05 — P1-A libk4tls.so armel 交叉构建打通（本机侧收官）

- 动作：build.sh 修正 + .cargo/config.toml 链接器全路径，`./build.sh` 双产物线全绿；so 产物 target/arm-unknown-linux-gnueabi/release/libk4tls.so（ARM EABI5 动态库，1.1M，stripped）。
- 根因（三个都看代码看不出来）：① cc-rs 1.11+ 只读小写目标变量 `CC_arm_unknown_linux_gnueabi`，原 `tr` 转大写导出的 CC_ARM_UNKNOWN_LINUX_GNUEABI 从未被读（musl 线当时能成是 shell 里恰有小写导出，ring 构建日志实证）；② kindle5 工具链不在 PATH，裸名链接器令 cargo 链接阶段报 cc-rs 找不到 gcc；③ 后台 shell 缺 ~/.cargo/bin，rustup 静默失败被 `|| true` 吞掉后 cargo 报缺 std——build.sh 现自补 PATH。
- 验收：nm -D 导出 k4tls_perform/k4tls_free_response/k4tls_version 三符号；objdump 符号版本需求仅 GLIBC_2.4（≤ K4 基线 2.12，dlopen 无版本风险）；NEEDED 全为 armel glibc 标准件（libc/ld-linux/librt/libpthread/libgcc_s/libdl）。链接期 accept4 警告属 std accept 路径，k4tls 不引用（链接器 GC 剥离），良性。
- 剩余：设备侧三项仍 blocked（R4 bench、dlopen 探针 + mock 网关、瘦身清单核对部署），等设备在线。

## 2026-11-08 — 状态复核与 K5 首轮实测（续）

- 更正上一条验收推断：仅 GLIBC_2.4 版本需求不能证明运行时兼容；K5 首次真实 dlopen 报 undefined symbol: getauxval。来源是 ring 的 ARM CPU 特性探测，无版本强符号被此前检查遗漏。
- 用户指定 K5 192.168.2.157:2222 可测试，确认 glibc 2.12.1、KOReader v2026.07.1、LuaJIT 2.1.1783773675；所有新产物仅在独立临时目录运行，未修改正式插件、系统 CA 或设备后端。K4 192.168.2.140:2222 仍拒绝连接，K5 结果不冒充 K4 验收。
- 构建修复：ARM/glibc 链接包装 getauxval，读取真实 /proc/self/auxv，不伪造 CPU 能力；-z defs 使缺失强符号在链接期失败。4 个辅助向量测试 RED 3 失败后全部 GREEN；K5 重新加载和版本调用成功，AT_HWCAP=12503。
- Lua 修复：ffi.string(ptr,len) 取代错误 ffi.copy；响应复制后释放，异常也释放；失败标准化为 (nil,err)，source/sink 错误与结束标记正确传递；默认及未知后端明确为 LuaSec。client 有界 sink 接受 nil 并返回 1。host 65 项通过，K5 LuaJIT 66 项通过（多出的真实 cdata 用例），不是网络业务验收。
- 瘦身修复：拒绝源目标双向重叠与路径别名，manifest/资源校验先于 --force 删除，保留字体缺失硬失败，门禁严格对照白名单。追加报告软链接回写源文件的 RED 复现，复制时跳过旧 SLIM_REPORT.json，10 组回归全部通过。
- K5 只读盘点：du -sk 整树65728 KiB、字体25168 KiB、插件4008 KiB；现有 DroidSansFallback/FreeSansBold 名称不存在，实际 CJK 为 NotoSansCJKsc-Regular.otf（16032 KiB）。还需保留当前正文/UI 字体、许可证及 SSH 恢复入口，禁止直接按现有三插件清单部署。
- 静态 bench 在 K5 可执行，但 weread.qq.com/web/ 两次为 HTTP500（284.1/303.8ms），成功样本0/2，不构成 R4 性能证据，也不能与完整同步的8秒比较。

## 2026-11-08 — K5 传输链验收（受控 TLS 夹具）与 JIT-on 段错误定性

- Rust 修复收口：http.rs 类型化 ReadError（Io/Invalid/TooLarge），响应解析支持 interim（100/103）跳过、HEAD/204/304 无正文、CL/TE 重复与冲突拒绝、chunk 声明先验上限再读载荷、行级+累计 64KiB 头部预算、EOF 封顶单字节探针区分恰好截断；transport.rs 全链 DeadlineStream（补上 rustls complete_io 内部多次 I/O 的漏洞）、单一进程级 DNS worker（有界队列 8、不可取消的 getaddrinfo 不再拖死调用方）、-13/-10/-11 按 is_timeout_err 分类、证书错误按 rustls::Error 下转型判定。clippy -D warnings 0 告警，cargo test 60/60（36 单元+8 FFI+16 传输）。
- 库生命周期：持久 DNS worker 线程在 dlclose 后继续执行已卸载代码属真实风险，build.rs 给 Linux/gnu cdylib 加 `-Wl,-z,nodelete`（K5 产物 readelf 确认 FLAGS_1 含 NODELETE+NOW），配合文档声明库为进程期常驻。
- K5 受控夹具结果（tools/tls_fixture.py，TLSv1.2 双端、临时 CA 仅注入测试调用、SSH -R 反向转发回环）：-joff 下 LuaSec 与 k4tls 各 9 路径全过（二进制 NUL 正文/chunked/interim/空/HEAD/304/HTTP500/断连/超时），k4tls 限额 -16、drip 总期限、不可信 CA 全过；50 轮浸泡（约 400 请求，覆盖全部 k4tls 错误路径）零失败。夹具耗时 k4tls 中位 33ms vs LuaSec 101ms，只代表受控回环 TLS1.2，不构成 R4。
- K5 JIT-on 独立进程段错误定性：与本组件无关。证据：`./luajit -e 'print(...)'` 纯启动 15/15 段错误（EXIT 139，无任何脚本输出）；`-joff` 启动后运行时 `jit.on()` 正常（ARMv7 VFPv3 全优化开）；同一段会话早期 JIT-on 跑通 30 次循环+全二分。期间确认一个自身泄漏：一次后台 harness 的本地 ssh 被 TaskStop 终止后设备端 luajit 残留（PID 32192，6:40 CPU），已 kill；但清除后 JIT-on 基线仍必崩，且 61MB free 下复现。属 KOReader 补丁版 LuaJIT 在该设备当前状态下的启动期问题，超出 Phase 1 范围；设备侧独立验证一律 -joff，正式 KOReader 应用内行为另行验收。
- 复现命令已写入 docs/INTEGRATION.md 验收清单（夹具启动、CA 签发约束 keyUsage、设备 harness/二分/浸泡脚本）。
- 未做：R4 正式业务基准、K4 真机、瘦身影响 manifest 调整、部署/翻默认/发版/commit——均待用户明确授权或设备条件。
