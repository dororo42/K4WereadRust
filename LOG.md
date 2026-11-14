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

## 2026-11-08 20:15 — K5 可控负载基准受阻（设备级 exec 段错误）；主仓 GH 上线

- 基准工具就绪并提交 bb24b19：tools/tls_fixture.py 增 /bench 路由（16KiB/128KiB/512KiB 等长 JSON 负载，修复单元实际 45 字节导致的长度 bug，host 三档 sanity 全过），tools/device_bench_check.lua 双后端交替采样（与已验收的 device_transport_check.lua 同一 dispatch 模式，走 https://localhost:18474 覆盖 DNS worker 路径）。
- 设备退化证据链：bench 三次未到 VERSION 输出 → -R 隧道对照矩阵（带/无隧道各 6/6 EXIT 139，否定隧道相关性）→ /tmp 副本 luajit（避开 /mnt/us 执行路径，md5 一致）仍 139 → 纯启动 `-e print` 5/5 139 → `/bin/busybox echo` 与 `/bin/cat` 同样 139。结论：K5 当前对新进程 exec 普遍段错误，与 luajit、本组件、二进制位置、文件读取路径均无关；dropbear 既有会话仍正常（md5sum/ls 等在退化初期尚可）。
- 环境核查排除项：无 core dump（ulimit -c 0）、内存充足（-/+ buffers/cache 约 94MB free）、dmesg 无新错误、/tmp 为 rwfs 32MB 非 tmpfs。dmesg 尾部见一次 TimeSuspended=1112s 的 resume；"好窗口=活跃期、坏窗口=挂起恢复后"为待验证假设（与更早"重写 luajit 后 10/10 过、随后又崩"的分钟级瞬态一致）。
- 处置：设备侧全部挂起（bench、R4、恢复 printk/core_pattern、清理 luajit.wedged/launch.flag），待设备重启后重跑——重启属用户设备操作，需用户执行或授权。本机夹具（18474）与临时证书保留待复跑，复跑结束后删除私钥。
- 主仓上线：GH public 仓库 github.com/dororo42/K4WereadRust（gh CLI 创建）。首推失败根因是主仓**未配置任何 remote**（此前误判为凭证问题）；remote add origin + 单次 `-c credential.helper='!/home/doro/.local/bin/gh auth git-credential'`（未改全局 git config）经 LAN 代理推送成功，origin/main=bb24b19 已 ls-remote 核实。
- 教训：对照矩阵全部失败时，先跑与被测物无关的最小进程（busybox 级）对照，可提前数轮定位到设备级问题，避免在组件侧反复归因。

## 2026-11-08 22:30 — 误诊修正（timeout applet 假象）与 K5 可控负载基准完成

- **根因修正，推翻本日两个结论**（"设备新进程 exec 普遍段错误"、"JIT-on 纯启动必崩"）：真正故障点是设备 busybox 的 `timeout` applet 稳定段错误（`timeout 5 /bin/true` → 139 复现；`/usr/bin/timeout → /bin/busybox`，busybox mtime Aug 2 但 echo/cat/ls/grep 同体裸跑全 rc=0）。当日所有失败测试均被 device-side `timeout` 包裹；裸跑对照 `/bin/busybox echo`、`cat`、`./luajit -e`（JIT-on 与 -joff 各 5/5）全部 rc=0。设备 uptime 1 天 19 小时**从未重启**——不存在"挂起后 exec 退化"，"好窗口/坏窗口"实为误诊链条。新纪律：**设备端禁用 `timeout`，限时用本机 `timeout` 包整条 ssh**（INTEGRATION.md 已更新）。
- 基准第二个坑：裸跑 luajit 必须带 `LD_LIBRARY_PATH=/mnt/us/koreader/libs`，否则 ssl.core 加载失败（libssl.so.60 缺失）、luasec 臂全灭；KOReader 的 `socket.http` 对 https 基址委托 `ssl.https`（probe 实证报错链），此前传输链全绿依赖同样的库路径可用。此前会话如何带入该环境未留痕，现已在 INTEGRATION.md 复现命令中显式写明。
- **K5 可控负载基准完成**（RC=0，90/90 正文等长校验通过，无残留进程）：16KiB/128KiB/512KiB × 15 样本 × 双后端交替，SSH -R 回环 TLS1.2 夹具，基址 https://localhost:18474 覆盖 DNS worker 路径。中位数 k4tls 43.1 / 118.9 / 367.6 ms vs LuaSec 106.0 / 188.4 / 434.8 ms（约 2.5× / 1.6× / 1.2×）；p95 k4tls 56.7 / 186.9 / 389.2 vs 274.7 / 316.5 / 500.7。仍只代表受控夹具传输层，不构成 R4 业务验收。
- 清理收尾：本机临时 CA 私钥与证书目录 /tmp/k4tls-https-r4 已删除，夹具进程已停；设备 core_pattern 复位为 core、luajit.wedged 与一次性探针已删。设备 /tmp/k4tls-check-XXpa5MtW 测试环境保留待 R4/K4 复用；/mnt/us/k4weread/launch.flag 未动（v8.4 上惰性，留待插件升级处理）。
- 期间设备曾掉线（休眠/USB 导出），DHCP 漂移 .157↔.151；用户唤醒后恢复。

## 2026-11-08 23:20 — 下一阶段：R4 传输基准完成 + 瘦身清单 K5 化（K4 延后，用户决策）

- 用户决策：按 K5 现有结果进入下一阶段，K4 真机延后。
- **R4 传输基准（真实端点，完成）**：tools/device_r4_check.lua 新增——weread.qq.com/web/ 未认证 GET、双后端交替 15×2、真实 DigiCert 链注入（leaf+intermediate+DigiCert Global Root G2，本机 openssl s_client 提取并 verify OK，未动设备系统 CA）、真域名走 DNS worker 路径、浏览器级 UA（取自插件 protocol.lua 的 WeRead.USER_AGENT）。结果：**30/30 成功、正文等长 525,412 字节**；中位 k4tls 546.6 ms vs LuaSec 643.1 ms（快 ~15%），均值 574.4 vs 672.6，p95 800.1 vs 866.1。此前静态 bench CLI 的 HTTP 500 系请求头形态问题（补浏览器 UA 后 200）。边界：这是真实端点传输层对比；完整同步业务验收（登录态全流程）仍属后续集成阶段。
- **瘦身清单 K5 化（完成，未部署）**：slim_manifest.json 按 K5 实测修订——keep_plugins 增 SSH.koplugin（远程恢复入口禁止移除）；keep_fonts 改逐文件白名单（CJK=NotoSansCJKsc-Regular.otf + KOReader 默认 Noto Sans 族 4 文件 + DroidSansMono + nerdfonts/symbols + README + 各保留字体许可证）。check_slim 门禁原硬编码 DroidSansFallback，先加 RED 测试（K5 NotoSansCJKsc 树过门禁）再修为接受 DroidSansFallback*/NotoSansCJK*；test_slim.py 合成树同步 K5 形态（含 main() 烟测断言翻转），**11/11 全绿**。K5 真实节省估算 ≈9.4 MiB（65.7M→~56M）：字体省 ~6.8M（弃 freefont/urw/NotoSerif/阿拉伯文/孟加拉文/天城文）、插件省 ~2.6M；K4 基线的 115-120M 预期与 K5 树不可比。实际替换设备树仍待用户确认。
- 设备实测补充：字体 25168KiB 构成（noto 20.6M 含 CJK 16M、freefont 2.8M、urw 0.8M、nerdfonts 0.8M、droid 0.1M）；插件 39 个共 4104KiB（weread_K4 1232KiB 最大）；weread.lua 设置与书籍 sdr 均无自定义字体键。

## 2026-11-08 22:48 — slim 部署上线 K5（可回滚）+ FreeSans 字形修复（冒烟回归闭环）

- 用户决策：先做 slim 部署，硬性要求可回滚；UI 冒烟通过后 commit/push。
- 部署方式与文档流程的差异：未走 slim_koreader.py 本机构建+rsync，而是 `tools/k5_slim_deploy.sh` 设备侧四段式（dryrun/execute/rollback/status）——mv 原树→`koreader.pre-slim-20261108`（原子改名即完整备份）、`cp -dpR` 同树副本（settings/weread.credentials.lua 仅查存在未读内容，weread/cache 用户数据原样）、按 manifest 派生清单手术删除（35 插件+32 字体，dry-run 先行核对计数与逐项清单）。原因：FAT 树含用户数据不宜整树网络往返、设备无 rsync、busybox 无 cp -a。execute 内置护栏：KOReader 运行拒绝、/mnt/us 写保护拒绝（USB 导出检测）、cp 失败自动还原、verify 门禁（keep 集/luajit md5/无泄漏/用户数据在位/luajit 冒烟）。
- 实测：95016→85536 KiB（-9.3M），VERIFY PASS；回滚=一条 `sh /tmp/k5_slim_deploy.sh rollback`（/tmp 重启即失，脚本可从仓内 tools/ 重传）。
- 设备 busybox 1.7.2 陷阱（新增工具箱条目）：无 cp -a（用 -dpR）、无 nohup/setsid（`trap "" HUP`+重定向脱离 SSH 会话，防休眠断连杀死部署进程）、无 grep -E、head 无 -c、ps w 不显示参数（进程核查必须扫 /proc/*/stat 的 comm）、`pgrep -f koreader` 永远命中 dropbear 自身（argv0=/mnt/us/koreader/dropbear）→ 设备侧 koreader 检测必须 comm 级匹配，部署脚本护栏已从 pgrep 改 comm 扫描。
- SSH 判定（用户问询）：KOReader 退出不断 SSH——dropbear 二进制虽在 KOReader 树内，但监听进程已 daemon 化（ppid=1），fork 服务连接不依赖磁盘文件；/etc/upstart 无 dropbear 单元，重启后 SSH 需经 KUAL 进一次 KOReader（SSH.koplugin 在保留白名单）拉起。本轮部署全程未重启。
- 冒烟回归与修复：菜单 ▸/▼/→ 变方框。根因（crash.log freetype 报错实证）：KOReader UI 回退链用 `freefont/FreeSans.ttf` 渲染几何符号（menu.lua ▸、numberpickerwidget ▼、readersearch ▾、toc/creoptions →），NotoSansCJKsc 无 U+25B8。修复=从备份树恢复 FreeSans.ttf(906K)+COPYING（md5 对账一致），放弃逐文件改 ">" 方案（需动 6+ 处上游源码且重建精简树即失）；净省 ~8.6M（树 92968 KiB）。`cache/fontlist/fontinfo.dat` 过期缓存已清（防字体菜单列出已删字体）。manifest、部署脚本、test_slim.py 三处同步（FreeSans 进白名单），11/11 全绿。
- crash.log 核实：无 crash——该文件是本构建的运行日志通道，会话 `Tearing down UIManager with exit code: 0` 干净结束；weread 全链路正常（QR 登录、书架同步 200 共 10157 本、时长上报成功）。"menu id not found: taps_and_gestures" 瘦身前已有 24 条，与部署无关（插件菜单挂接问题，留插件仓处理）。
- 遗留：备份树 `/mnt/us/koreader.pre-slim-20261108` 保留待用户决定删除；launch.flag 未动；K4 真机延后（用户决策）。

## 2026-11-08 23:55 — 双仓更新同步核查（借鉴评估）

- 背景：用户提示 K4weread 与 weread_K4.koplugin 有更新需借鉴。核查插件仓 v9.8 发版后 3 提交（b82d7ed / 44958b6 / 366b1f9）与 K4weread v0.1.1 发版批次（f9c00a9 / d6864ea）。
- 结论 1（核心）：插件仓 b82d7ed 已把 k4tls 绑定语义修复同步进 `weread/lib/k4tls.lua` + `transport.lua`，与本仓 `lua/` 副本 **diff 逐字节一致**——本仓无回移动作，唯一真源仍是本仓 `lua/`。
- 结论 2（协调项）：插件 `client.lua` 的 k4tls 注释仍写"默认后端 auto"（d644920 初版旧文），与 transport.lua 实际语义（默认 luasec）不符；本仓对插件仓只读且有并行在制品，已记入 INTEGRATION.md 协调项，翻闸集成阶段一并修正。K4weread f9c00a9 的 LOG 已复核 b82d7ed 返回形状与 client.lua pcall 判定一致。
- 结论 3（借鉴采纳）：K4weread v0.1.1 的跨仓契约护栏（check_marker_path --peer-repo 机器校验对端仓）模式可在本项目后续多仓集成时复用；366b1f9（QR 登录 Back 取消/Menu 退出）改善后续真机登录态验收操作（不再被迫等 300s 超时）。版本引用同步：AGENTS.md v9.7→v9.8、spec 护栏数 433→445；INTEGRATION.md 状态行更新（R4 传输基准已完成，登录态业务验收待集成阶段）。
- 边界：未触碰插件仓与 K4weread 仓任何文件（只读核查）；未翻默认后端。

## 2026-11-09 06:15 — 页脚/菜单“时间和电量之间”方框根因定谳 + FreeSerif 回补（第二轮字形修复）

- 用户报告菜单“时间和电量之间”有方框问号。取证链（全部设备实测）：
  1) readerfooter.lua 全量非 ASCII 码点提取 = 31 个（icons 前缀表 + 菜单行标签把每项图标嵌进文本）；
  2) **裸 FFI FreeType 探针**（新方法：绕开此前 bare luajit 下 ffi/freetype.lua 的 loadlib 引导失败，直接 dlopen 设备 libfreetype，FT_Get_Char_Index 出覆盖矩阵）——四面（NotoSans/CJKsc/symbols.ttf/FreeSans）× 34 码点；
  3) 结果：nerd 图标组（U+E790/EA5A/ECA8/ECA9/电池 E778-E783/EDA3）symbols.ttf 全部可用（⇒ 上一个疑点排除，回退链 [6] 位正常）；⇒/⁄⁄/⇄ CJKsc·FreeSans 兜底正常；**U+231A ⌚（时间）、U+23F3 ⏳、U+229B ⊛、U+291F/2920/293B/2956 四面全无覆盖**；
  4) 对备份树 FreeSerif.ttf 复测：除 nerd PUA 段外全部有字形（含 💡 U+1F4A1）——**FreeSerif 是上游回退链 [8] 里 Misc-Symbols/Supplemental-Arrows 区的唯一提供者**，瘦身裁掉它后这些符号全变方框；crash.log 里 FreeSerif 回退加载报错正是链路在尝试它的痕迹。
- 根因：slim manifest 首版只回补了 FreeSans（几何符号 ▸▼→），漏了 FreeSerif（Unicode 符号区字形）。用户看到的“时间和电量之间”方框 = 时间行/页脚时间项的 ⌚ U+231A（菜单“状态栏项目”行序：时间→章节进度→剩余页数→电量，时间行嵌字形恰在该区间）。
- 修复（与 FreeSans 同法，纯增量可逆）：从备份树 cp FreeSerif.ttf（1862212B，md5 ddf0ad73fbf815212c8c9268dadbcaa2 对账一致）→ 清 cache/fontlist/fontinfo.dat。待用户重启 KOReader 冒烟。
- 工具同步：slim_manifest.json keep_fonts+FreeSerif（comment 记两轮字形教训）、k5_slim_deploy.sh KEEP_FONTS 同步、test_slim.py keep/gone 断言翻转（FreeSansBold 仍排除），11/11 全绿。
- 体积口径闭合：fonts 25168→21192 KiB（省 ~4.0M，含两轮回补 2.7M）、plugins 4104→1376 KiB（省 ~2.7M）→ 瘦身面净省 ≈6.7M；整树 du 96112 vs 备份 95016 反升 1.1M = weread 数据目录 8.4M（10157 本书架缓存，用户数据不属瘦身面）持续增长 + FAT du 噪声。**新发现（记录不动）：l10n 目录 18.9M 全语言翻译文件是未来瘦身候选**，不在已批准 manifest 范围，未经用户确认不裁。

## 2026-11-09 — 独立代码审查修复批次（P0/P1 + 可选 P2，仅工作区，未提交）

- 背景：`~/reports/2026-11-09-K4WereadRust-独立代码审查报告.md` 的 1 项阻断 + 8 项重要发现；用户要求"进行修复，并更新报告"。本批次只改工作区，未 commit/push/发版，未动设备、插件仓与 K4weread 仓。
- P0：F-01 `tools/k5_slim_deploy.sh` 的 `verify()` 删掉与 `KEEP_FONTS` 自相矛盾的目录级泄漏断言（保留 `$LIST` 逐项判定），并加 `test_slim.py` 合成树回归；F-06 `README.md`/`docs/INTEGRATION.md`/`docs/SLIMMING.md` 状态对齐（R4 完成、K5 slim 已上线、v9.8、构建纪律、rsync 与脚本方案标注）。
- P1：F-02 空 POST/PUT/PATCH 补 `Content-Length: 0`；F-03 `url.rs` 按 `/?#` 分离 authority、剥离 fragment、入口收紧 host/path 校验；F-04 新增 `-18/-19` 确定性错误码族（Lua 文本不拼 detail、不命中重试词表）、改 `transport.rs` 两处映射；F-05 设备清单与 manifest 的生成期一致性门禁；F-07 `transport.last_reason` + 强制 k4tls 失败时的可检索 reason（不改 `request` 契约）；F-08 响应头改 lossy 解码，非 UTF-8 头不再整包失败。
- P2 取舍：F-13/15/16/20 已修；F-11/12/14/17/18 部分修（ABI 保留字段注释、pinning 措辞、UTF-8 字符边界截断、rollback 原子改名、`--locked`）；F-09（CI workflow 需交叉工具链托管方案与推送授权）、F-10（选择器单点化属接口变更）、F-19（TLS1.2 夹具是有意设计）暂缓；F-21 不改。逐条理由见报告 §9.1。
- 验证：`python3 -B tools/test_slim.py` 13/13（新增部署 verify 合成树、清单一致性 2 项）；`lua5.1 tools/test_k4tls_lua.lua` 67/67（新增词表不命中、强制后端诊断 2 项）；`luac -p` 全部 Lua 通过；`sh -n` 两个 shell 脚本通过；Python AST 4 文件通过；`rustfmt --emit stdout` 仅语法解析全部通过；IDE 诊断 0 条。20 个文件 +351/-48。
- 边界：**Rust 改动未经编译验证**——遵守"Rust 编译产物一律由 GitHub workflow 产出、本机不手工构建"的纪律，rustfmt 只做语法解析不替代类型/链接检查；需 CI 或授权环境跑 `cargo test --locked` / clippy / 交叉构建。`lua/` 与插件仓 `weread/lib/` 双副本尚未同步（插件仓只读、有并行在制品），故当前不翻默认后端。未改 `handoffs/`，交接文档未上传。
- 报告已追加 §9「修复与复核」，第 0–8 节原始审查结论保留为审计轨迹。

## 2026-11-09 — 登录态完整同步验收 PASS + 验收注入回退（Phase 1 集成阶段收尾）

- 为什么：k4tls 组件的最后一项验收——真实 KOReader 应用内、登录态业务链路（进度同步 + 阅读时长上报）走 k4tls 全程成功，才算 P1-A 集成验收闭环；验收手段是显式 env 注入（不翻默认值），完成后必须回退。
- 设备侧动作（K5，用户操作 + 本机核证）：11/08 22:29 注入 `export WEREAD_TLS_BACKEND=k4tls`（备份 `koreader.sh.pre-k4tls-accept`）；13:18 用户 KUAL 全新启动 KOReader（插件 v9.8）并触发同步；13:40 本机恢复 `cp -p` 备份，md5 一致（949c2f8b…），注入 GONE，默认 LuaSec 恢复，备份留设备作审计。
- 验收证据（crash.log 实录，/tmp/k5inspect/k5_accept_evidence.sh 采集）：`11/09 13:18:15 [WeRead] initialized`（注入后全新启动）→ `ffi.load: k4tls`（裸名失败）→ `ffi.load: plugins/weread_K4.koplugin/weread/lib/libk4tls.so`（插件同目录命中 = 生产形态位置；此两行仅在 env=k4tls 且 `is_available()` 被调用时出现，同时反证 env 传播）→ 1 秒内 `ProgressSync verified: positions_match` + `read report success: has_synckey=true`，零传输错误、无回退；设备 .so md5 `978b5739…` 与 R4 基准同一构建。
- 边界：`/proc/<pid>/maps` 驻留快照未及采集（同步后 KOReader 即退出），以 ffi.load 成功 + 成功请求链替代，结论不受影响；验收会话用的仍是修复前 .so 与插件仓旧 lua 副本（设备行为与本批次 Rust 修复无关）。
- 文档同步：INTEGRATION.md 状态行与结论边界、AGENTS.md 集成纪律与 Current Handoff Status、README.md 状态核对基线时态修正（"未提交"→ 已提交 27c8af3）。
- 翻闸前置不变：Rust 编译验证（CI/授权环境）+ 双仓 lua 副本同步 + 用户决策；默认后端 LuaSec 未动。

## 2026-11-09 — CI 门禁上线全绿（F-09 关闭）+ 首跑双失败修复链

- 为什么：审查报告 §9.3 剩余阻断之首——Rust 改动未经编译验证；且"构建产物由 CI 产出"的纪律缺承载点。用户授权经 GitHub CI 执行 Rust 编译。
- 改了什么：`.github/workflows/ci.yml`（26849bb）三并行 job——host-gates（cargo test --locked、clippy -D warnings、luac -p + lua5.1 全量、test_slim.py、sh -n）、bench-musleabihf（静态 k4tls-bench）、so-armel-glibc（armel glibc libk4tls.so）；rustc 固定 1.98.1；交叉工具链以本仓 release `cross-toolchains` 资产分发 + sha256 固化（musl 11e155a0…、armel d433aa16…），armel 工具链即产出设备已验收 .so 的同一构建器，config.toml 本机 linker 路径用 CARGO_TARGET_…_LINKER env 覆盖。
- 首跑失败与修复（run 42627407583）：host-gates ✗ clippy::manual_pattern_char_comparison（url.rs:18）→ 数组 pattern 等价改写；bench ✗ musl.cc 站点宕机（连接 4 连败）→ musl 官方包原样上传 release 资产、CI 改同源拉取（2d25c33）；二跑 404 → 资产上传时沿用了本地文件名，PATCH 改规范名 arm-linux-musleabihf-cross.tgz 后单 job 重跑。
- 验证：run 42629289540 **三 job 全绿**（host-gates 44s / so-armel 35s / bench 重跑过）。产物：k4tls-bench（armv7-musleabihf 静态，1,135,568 B，sha256 0e8ac5c5…）、libk4tls.so（armel 动态，1,186,764 B，sha256 edb6dc76…，与设备修复前版尺寸相同内容不同——CI 版含 27c8af3/2d25c33 语义修复）。
- 边界：CI 产物替换设备 .so 与本机 target/ 退役属用户决策；musl.cc 供应链从"直连上游"改为"上游原样 + sha256 固化的本仓资产"；actions 弃用警告（Node 20→24 强制升级、ubuntu-latest 将迁 Ubuntu 26）非阻断。
- 翻闸前置更新：Rust 编译验证**已过**；剩插件仓双副本 lua 同步 + 用户决策。

## 2026-11-09 — CI 产物替换设备 .so（用户授权）

- 为什么：CI 版 .so 含 27c8af3/2d25c33 全部语义修复（-18/-19 确定性错误码、空 POST Content-Length: 0、URL 分离），设备继续跑修复前旧版会让 CI 验证与设备实际脱节；用户明确授权替换。
- 设备动作（KOReader 未运行，FAT 原子性无运行时风险）：`cp -p` 备份旧版为同目录 `libk4tls.so.pre-ci`（md5 978b5739…，保留作回滚点）→ scp 推送 CI 产物 run 42629289540 artifact → 设备 md5 8b89393a… 与 CI 产物一致（1,186,764 B）。
- 验证：①本地预检 readelf——CI 产物动态符号版本上限 GLIBC_2.12，与设备 glibc 2.12.1 匹配；②设备 LuaJIT 生产路径探针（require weread.lib.k4tls → is_available → 插件同目录 dlopen）：`available: true`、`version: k4tls 0.1.0 rustls-0.23/ring`、rc=0；探针残留已清理。
- 边界：新 .so 暂不被使用（设备无 WEREAD_TLS_BACKEND 注入，默认 LuaSec）——替换只是把规范构建就位；真正走 k4tls 链路需翻闸或显式 env。旧 lua 副本（无 -18/-19 文本）与新 .so 向后兼容：-18/-19 文本会显示为 "k4tls unknown error -N"，属可接受降级，双仓 lua 同步后消除。

## 2026-11-09 — 测试方案 A 应用内冒烟 PASS（CI 产物 .so 生产链路实证）+ -2012 会话过期归因

- 为什么：CI 产物 .so（8b89393a…）替换设备旧版后尚未在真实业务链路运行过；测试 A 用与 13:18 验收相同手段（临时 env 注入、用后即撤）验证 CI .so 在生产路径（真实 KOReader + weread_K4 插件 + 正常入口）完整可用。用户确认入口口径：不从 K4weread 独立入口进；注入前预检 launch.flag 已确认 11/08 消费、目录为空。
- 设备侧动作：~14:52 UTC 注入 `export WEREAD_TLS_BACKEND=k4tls`（LC_ALL 行后，注释注明冒烟用途与撤除承诺）；用户 14:53:35 / 14:57:24 UTC 两次 KUAL 正常入口全新启动（14:51 会话为注入前 LuaSec 残留，开书验证后 14:51:24 关书退出）；14:58:16 UTC 收证据并当场移除注入，koreader.sh md5 恢复 949c2f8b…（与备份逐字节一致）。
- 传输层证据（crash.log 3483-3484）：`ffi.load: k4tls`（裸名失败）→ `ffi.load: plugins/weread_K4.koplugin/weread/lib/libk4tls.so`（命中）→ 4 次进度上传 POST /web/book/read + 后续每分钟阅读时长 tick 全部完成完整 TLS 往返（HTTP 200 + JSON 解析正常，tick elapsed 135-176ms），零传输层故障。
- 业务层归因（-2012 登录超时，error_kind=auth）：服务端以 errCode=-2012 拒绝写请求。crash.log 全史 -2012 共 20 行，其中 6 行在 8 月 LuaSec 时代（08/06、08/08、08/09），errMsg 与报文格式逐字一致——weread 会话空闲过期的既有模式，与 k4tls 无关（登录态约 13:1x 建立，14:57 已约 1.5h）。8 月已见「-2012 ERROR 与 verified 相邻出现」同款（833/834、842/843 行），故 verified 行不构成会话有效性强证据。
- 边界：进度同步 verified（positions_match）按既有语义容忍服务端拒绝；冒烟目标为传输层，业务写入以插件内重新登录为前提（LuaSec 下同样需要）。运行中进程 env 不受文件撤除影响，下次重启自然回到 LuaSec。
- 结论：**测试 A PASS**——CI 产物 .so 生产链路完整可用；默认 LuaSec 未动，翻闸前置不变（双仓 lua 同步 + 用户决策）。

## 2026-11-09 — 三副本 lua 同步完成（仓内 → 插件仓 → 设备，翻闸前置收口）

- 为什么：翻闸前置之一——设备 lua 副本仍是 11/08 旧版（缺 -18/-19 文本、词表豁免、last_reason），与 CI .so 的语义修复不配套；三副本对齐后 k4tls 语义层在设备完整生效。
- 同步内容：仅 `k4tls.lua`（30a9db1a）+ `transport.lua`（6346dd22）两个文件；diff 预检确认插件仓为纯旧版（无插件仓侧独立改动），插件仓工作区干净（HEAD 366b1f9，v9.8 出包收尾）后执行。
- 各步验证：①插件仓 cp 后 md5 双侧一致 + luac -p 通过，插件仓 git 恰好 M 这两个文件；②设备 scp 前 cp -p 备份（`k4tls.lua.pre-sync` / `transport.lua.pre-sync`，即旧版 3d17fd95/1ec9d2c7）；③推送后设备 md5 与仓内一致，-18/-19 与 last_reason 内容标记在位；④设备 LuaJIT 生产路径探针 rc=0——`is_available: true`、`error_text(-18)= "k4tls invalid or unsupported url"`（旧版此处为 "k4tls unknown error -18"，探针即新旧判别器）、-18 不拼 detail（词表豁免生效）、env=k4tls 时 `transport.resolve` 返回 reason=k4tls。
- 边界：插件仓这两个文件为未提交修改——是否提交由用户/并行会话决定（未代提交）；运行中 KOReader 不可见新 lua（require 缓存），下次启动生效；探针脚本留存设备 /tmp/k5inspect/（临时测试物）。
- 结论：翻闸前置「双仓 lua 同步」**完成**；剩用户翻闸决策。方案 B（注入浸泡）随时可开始。

## 2026-11-09 — 方案 B 浸泡开始（env 注入态，多日）

- 设备 `/mnt/us/koreader/koreader.sh` 已注入 `export WEREAD_TLS_BACKEND=k4tls`（LC_ALL 行后，注释注明"浸泡测试，翻闸决策后处置"），md5 `9ffc2f33…`（对照基线 949c2f8b… = 无注入恢复态）。
- 本轮浸泡栈位：CI 产物 .so（8b89393a…）+ 新版 lua（30a9db1a/6346dd22）——设备首次以完整语义栈跑 k4tls。
- 浸泡期证据口径：crash.log 每会话 `ffi.load` 两行 + ProgressSync/ReadReport 活动与错误、`last_reason` 诊断；已知前置：weread 会话已于 14:57 起过期（-2012），用户需在插件内重新登录，写请求才恢复。
- 回滚：删除注入两行即恢复 949c2f8b 基线（默认 LuaSec）。

## 2026-11-10 — lua 镜像再对齐插件仓 HEAD（k4tls.lua aa24b9c5 内置库优先）+ 两仓状态核实收口

- 为什么：插件仓 v9.10/v9.11 批次吸收了 k4tls.lua 上游复审项 🟢-8（ac34e11"内置库优先"：ffi.load 搜索顺序改为模块同目录 libk4tls.so 优先 → 全局名 "k4tls"，防系统同名旧库静默遮蔽），本仓 lua/ 镜像落后于插件仓 HEAD。
- 同步方式（TDD）：cp 插件仓 aa24b9c5 → `lua/k4tls.lua` 后先跑红基线——k4tls_spec.lua:151 加载顺序断言期望旧序（"k4tls" 先于模块路径），恰好 2 例失败；对调断言（模块路径在前）后 `lua5.1 tools/test_k4tls_lua.lua` 67 tests, 67 passed，`luac5.1 -p` 全绿。本仓独有的 -18/-19 词表测试保留，未整文件覆盖插件 spec。
- 设备侧口径：浸泡三件套冻结不动（k4tls.lua 30a9db1a / transport.lua 6346dd22 / libk4tls.so 8b89393a）——浸泡证据链以开始时组合为准；k4tls.lua shim 漂移（30a9db1a → aa24b9c5）列为翻闸后动作（设备上可证明 no-op：裸名 ffi.load 本来就失败）。
- 设备运行时同步：插件仓已发版 v9.10（898dde8）/ v9.11（66ca115，Latest，run 42825530616）；设备运行时差集 = qr_login.lua（3fd3861b，U17-1/U17-2 + v9.10 提示条并入 0.2s 延迟闭包）+ _meta/main 版本字面量。已挂持久监视器做外科手术式同步（仅推这 3 个文件，.pre-v911 备份，SOAK-INTACT 校验浸泡三件套未被触碰），等 K5（192.168.2.157:2222）上线窗口。
- 结论：翻闸前置维持"仅剩用户决策"；翻闸后动作清单新增"设备 shim 升 aa24b9c5 与 v9.11 对齐"。U17 真机验收（弱网扫码、二维码四角完整、Back/Menu 即时）待用户按键确认。

## 2026-11-11 — 翻闸落地核实 + 两设备 v9.12 收敛（浸泡/翻闸线闭环）

- 状态核实（一手证据）：K5 生产已翻闸（koreader.sh `export WEREAD_TLS_BACKEND=k4tls`，插件仓并行批次 2026-11-10 执行）且 shim 已升 aa24b9c5（内置库优先——加载只剩一行 `ffi.load: plugins/.../libk4tls.so`，裸名失败尝试消失，🟢-8 真机生效）；全日志 0 条真实 k4tls 错误（此前 73 条宽匹配均为 `fallback=false` INFO / credential_store 降级 WARN）。
- k4tls 生产链路实证（11-10 23:57 会话）：-2012 会话过期（传输层 HTTP 200 正常送达，认证层旧模式）→ account_cleared 停报 → QR 轮询遇 k4tls 弱网超时（os error 11）→ U17-1 语义接住，5 秒后登录完成 → account_cleared→relogin 同秒重启上报 → 23:59:04 read report success——浸泡期担心的弱网/会话过期场景全部按设计工作。
- 设备收敛：K5 = v9.12（qr_login 01dd9c19 / _meta 32ad6dd7 / main 16d675ab，与仓/Release 逐字一致）+ k4tls 生产 + shim aa24b9c5；K4（B00E）= v9.12 全量（无 .so，默认 LuaSec，k4tls 休眠），用户实测 D 系列通过（crash.log 0 ERROR，wantread 瞬态重试、relogin 链实证）。K4 部署项在插件仓交接文档闭环（done）。
- 浸泡线结论：浸泡三件套冻结口径随翻闸成为历史；R4 bench PASS（k4tls 0.837s vs LuaSec 0.866s）后用户批准翻闸，翻闸后设备无不良记录。
- 结论：**Phase 1 网络组件线全面闭环**（编译/CI/基准/真机/翻闸生产）。剩 D11 两观察点（用户日常）。

## 2026-11-11 · libk4tls.so release 资产化（weread_K4 分发链路打通）

- **为什么**：weread_K4.koplugin 的 libk4tls.so 无分发渠道（不在 git/Release zip，仅 11-08 手工部署 K5），用户拍板 .so 分发选 2（CI 交叉编译）——侦察发现本仓 CI 已有 so-armel-glibc job（F-09），落地为打通链路。
- **改了什么（本仓，仅 release 资产）**：release `libk4tls-v0.1.0`（tag + 资产 libk4tls.so）——CI 产物（run 42629289540 so-armel-glibc job），md5 `8b89393a` 与 K5/K4 现役 .so 逐字一致（现役即 CI artifact 来源实证）。无代码变更。
- **对端（weread_K4.koplugin c67d305）**：release.yml fetch（匿名下载 + sha256 固化 `edb6dc76…` 校验）+ build_release.sh 豁免/追加修正 + ci.yml 资产哨兵；本地全链模拟通过（zip 含 .so），CI 绿 run 43118438330。**weread_K4 下次发版起 Release zip 自带 libk4tls.so**。
- **验证记录**：CI 产物 vs 现役 md5 一致；两台真机（K5 0.837s / K4 0.833s vs LuaSec 0.866s/0.921s）功能与性能已验证。

## 2026-11-12 · 三仓代码审查复核意见落地（源码/文档/门禁，未构建、未发版）

- **依据**：`~/reports/2026-11-12-K4weread-三仓代码审查与项目一致性核查报告.md`（I-1…I-8 共 8 类不一致 + F-1…F-13 共 13 项优化）。本批**不改 Rust 运行时逻辑**（FFI 只补契约注释）、不递增版本、不打 tag、不 push、不构建、不上传资产。
- **改了什么（本仓）**：①`lua/transport.lua` 同步插件仓 `backend_explicit` 语义，`lua/spec/transport_spec.lua` 增 2 例覆盖「菜单显式选择优先于 env」（I-1）；②新增 `tools/check_lua_mirror.py` 并接入 `ci.yml`（I-1/F-11）；③`ci.yml` 补 `pull_request`、`permissions: contents: read`、三处 checkout 固定 SHA、非阻断 `cargo fmt --check` 观察（I-8/F-11）；④`README.md` 去掉重复的 HEAD 基线、统一翻闸/插件版本/「逐字节一致」声明范围、登记 `docs/RELEASE_ASSETS.md`（I-3/I-6/I-7）；⑤`AGENTS.md` 修正「R4 前不翻闸」「逐字节一致」「build.sh 含下载/校验」三处（I-6/I-7）；⑥`docs/INTEGRATION.md` 更正分发方向、标记 client.lua 注释已修、统一 K4 验收时间线、新增三仓协同表（I-5/I-6/I-7）；⑦新增 `docs/RELEASE_ASSETS.md`（资产指纹/兼容矩阵/依赖与同步顺序，I-4/F-10）；⑧`k4tls/build.sh` 增加 linker env 覆盖与可覆盖代理变量，`.cargo/config.toml` 注明绝对路径仅本机示例（F-13）；⑨`k4tls/src/lib.rs`、`src/tls.rs` 补 FFI 边界契约与残余风险注释（F-12）。
- **复核更正**：审查报告 I-4 曾写「本仓未记录 `edb6dc76…`」；实际 `LOG.md:146,201` 与 `docs/INTEGRATION.md:138` 以 8 位前缀记录，是只按 64 位完整值检索造成的漏检。报告已就地更正；I-4 结论（无完整值清单、无 Release 产出 workflow）不变。
- **如何验证（本机源码级实跑）**：两个 Lua 绑定镜像 sha256 与插件仓逐字一致（`a4e1c5ed…` / `f0155311…`）；`python3 tools/check_lua_mirror.py --peer-repo` 通过，负向（改一字节）按预期 FAIL；`lua5.1 tools/test_k4tls_lua.lua` **69/69**（改前基线 67/67，新增 2）；`luac5.1 -p $(git ls-files '*.lua')` 全过；`sh -n k4tls/build.sh tools/k5_slim_deploy.sh` 通过；`python3 -B tools/test_slim.py` **13/13 PASS**；新增 Python 脚本 `ast.parse` 通过。`cargo fmt --check` 仍有 6 处历史差异（`http.rs` / `lib.rs` / `transport.rs` / `url.rs`），故 CI 中为非阻断观察步骤。
- **未落实为代码改动的项**：F-12 的运行时加固（给 `k4tls_version` / `k4tls_free_response` 加 catch_unwind、把 `tls.rs` 的 `.expect` 换成 `Result`、补重复释放/空指针 FFI 用例）——按构建纪律本机不编译 Rust，无法确认，登记为需推送后由 CI 验证的单独批次；`k4tls_perform` 已有 catch_unwind 兜底，未观测到跨 FFI unwind 故障。F-11 的 fmt 硬门禁同理待全量格式化批次。
- **边界**：未跑 `cargo test` / `clippy`、未编译 `.so`、未连真机、未访问远端 Release；三仓 handoff 套件同批生成（本地 only，不入库）。跨仓改动范围、依赖关系与同步顺序见各仓 HANDOFF.md「三仓协同」节及 `docs/RELEASE_ASSETS.md` §4。

## 2026-11-12 19:25 — 字体裁剪第三轮回补（真书缺字）+ K5 暂回状态核实（未 commit）

- 用户报告 K5 真书缺字：crash.log freetype 报错 `NotoSansDevanagariUI-Regular.ttf`/`NotoSansBengaliUI-Regular.ttf` 加载失败（slim 首版裁剪，前两轮 FreeSans/FreeSerif 教训的延续）。
- 设备回补：从备份树 `koreader.pre-slim-20261108` 恢复 noto/NotoSansBengaliUI-{Bold,Regular}.ttf + NotoSansDevanagariUI-{Bold,Regular}.ttf 四文件（0.8M，md5 逐字对账一致），`cache/fontlist/fontinfo.dat` 已清（KOReader 下次启动生效）。阿拉伯文 4 文件（Naskh/ArabicUI）仍按 slim 口径裁剪、备份树可取。
- 工具三处同步（本仓）：`slim_manifest.json` keep_fonts + 注释教训③（净省 6.7M→5.9M）、`tools/k5_slim_deploy.sh` KEEP_FONTS、`tools/test_slim.py`（合成树源 + keep 断言各 +4）；`python3 -B tools/test_slim.py` **13/13 PASS**，`sh -n k5_slim_deploy.sh` 过。
- 顺带核实 K5 暂回状态（AGENTS.md 已记载「11-12 因 segfault 归因 A/B 暂回 luasec」）：koreader.sh 08:02 起 `export WEREAD_TLS_BACKEND=luasec`，插件目录 `libk4tls.so`（1.19MB CI 产物）+ `k4tls.lua` 完好，仅激活开关关闭；今日 K5 会话在 LuaSec 下仍于 18:42 死于挂起空窗（插件仓判读：k4tls 非会话死亡变量，与 11/09 空壳先于 k4tls 的证据闭环）。
- 边界：未 commit/push（工作区已有并行未提交批次，本批叠加待统一授权）；未动 Rust 代码；设备改动仅字体文件与缓存；K5 凭证/设置已在设备端备份（bak-20261112-19xx，内容零过境）。

## 2026-11-13 · F-12 加固落地（FFI 边界统一防护，b171dff）

- **为什么**：11-12 三仓审查报告 F-12 的三项建议中，version 已改静态字面量（消除 panic 路径），free_response 的 catch_unwind 与 tls.rs:36 的 .expect 改 Result 登记为「CI-verified batch」——本批即该批次。
- **改了什么**：①`k4tls_free_response` 包 catch_unwind（畸形指针/长度组合降级 no-op，双释放防御不变）②`tls.rs` 新 variant `TrustError::ProtocolVersions(String)`，`build_config` 的 `.expect` 改 map_err（rustls 规则变化时不再 panic，错误进 out.err）③`transport.rs` caller match 补 variant ④`tests/ffi_tests.rs` 补 3 例 F-12 回归（repeated free / 部分清理 residue / free(null)）。
- **如何验证**：cargo test 全绿（40 unit + 11 FFI，含 3 新用例）；cargo clippy --all-targets 0 告警；CI run `43573806467` success（host-gates/bench/so 三 job）。
- **边界**：不加 Lua 侧改动（k4tls.lua 的 ffi.gc+显式 free 契约不变）；与 K5 segfault 无关（catch_unwind 防 panic 不防 SIGSEGV——segfault 归因仍在 KOReader/电源层）。

## 2026-11-14 · 弱网超时重试修复（DeadlineStream EAGAIN 风暴，265f427+02c1601）

- **背景**：11-12/14 K4 崩溃循环期间弱网日志出现 `os error 11`（EAGAIN）风暴；定位为 2.6 世代内核 SO_RCVTIMEO 超时返回 EAGAIN 而非 ETIMEDOUT，`DeadlineStream.read/write/flush` 无重试循环，每次慢读直接中止整个请求。
- **修复**：`retry_on_timeout` 闭包循环——超时类错误（WouldBlock/TimedOut/EAGAIN/EWOULDBLOCK）在 total deadline 内持续重试，deadline 过期才上抛 TIMEOUT；deadline 语义不变（每 socket 操作预算 block、整请求硬期限 total）。
- **测试**：cargo test **69/69**（40 unit + 11 FFI + 18 transport）；新增 slow_chunked 用例（TLS 后 4 片响应按 2.5s 间隔慢发 > block 2s，验证重试直至完成）；stalled-body 用例断言随新语义更新（deadline 内等满而非 100ms 快速放弃）；hold server 修复（读空后不再 close——close 会以 EOF/HANDSHAKE_FAIL 呈现导致客户端永远到不了自身 deadline）；clippy --all-targets 0 告警；cargo fmt 全仓收尾（http/lib/tls/url/ffi_tests 行宽）。CI run `43582621298` 全绿。
- **真机 A/B 实证（K5，独立进程 -joff，SSH -R 隧道接 PC tls_fixture）**：新增 `/slowchunk` 路由（Content-Length:19 分 4 片、片间隔 2.5s，每次必触发 block 超时）——**旧 .so**：`read: Resource temporarily unavailable (os error 11)`，2055ms 中止（EAGAIN 风暴原样复现，与 K4 日志同款错误，旁证 2.6 世代内核行为一致）；**新 .so**：7645ms、HTTP 200、body `slow-chunk-fix-done` 19 字节完整——重试穿过 3×2.5s 分片间隔，耗时数学吻合（3×2500ms+握手）。
- **部署**：K5 `/mnt/us/koreader/plugins/weread_K4.koplugin/weread/lib/libk4tls.so` = CI 产物 md5 `fcc3f850`（旧版备份 `.bak-20261114`，mv 原子替换未 truncate 已映射文件）；FFI 冒烟 `k4tls 0.1.0 rustls-0.23/ring` 通过；设备测试残留已清（脚本/CA/rwfs k4tls-check 旧目录）。**K4（192.168.2.140:2222）连接拒绝离线，待上线后同法部署**（scp tmp+mv，md5 fcc3f850）。
- **fixture 入库**：`tls_fixture.py` 增 `/slowchunk` 路由（02c1601）——自签 CA→CA 签发服务证书链（rustls 拒绝 CaUsedAsEndEntity，裸自签证书不能当信任锚——复测时注意）。
- **边界**：Lua 侧零改动；不修复 SIGSEGV（K5 segfault 归因仍在 KOReader/电源层，catch_unwind 防 panic 不防 segfault）；不发版不打 tag（v0.1.0 组件无独立发版通道，.so 经插件仓 Release 分发——v9.13 发版时将携带本修复的 .so）。

## 2026-11-14 18:40 — K4 上线补部署（弱网修复 .so，md5 对账 + 真机 /slowchunk 实证）

- K4 实际地址漂移至 `192.168.2.151:2222`（DHCP，handoff 记录的 140 已失效；MX50 + FW 4.1.4 yoshi 确认身份）。KOReader 运行中（2 luajit reader.lua）。
- 部署：旧 .so（md5 `8b89393a` = 11-12 批次旧产物，含 EAGAIN 缺陷）备份为 `.bak-20261114` → CI 产物（`fcc3f850`）scp /tmp → mv 原子替换 → md5 对账通过。
- 验证：FFI 冒烟 `k4tls 0.1.0 rustls-0.23/ring` 通过；**真机 `/slowchunk`（4 片×2.5s 间隔 > block 2s）7645ms、HTTP 200、body `slow-chunk-fix-done` 19 字节完整**——修复版在 EAGAIN 风暴原始受害设备上弱网重试实证成功（K4 真机日志 11-12/14 即旧版失败的现场证据，不再回装旧版做 A/B）。
- 后端开关核实：K4 `koreader.sh` 无 `WEREAD_TLS_BACKEND` 注入、KOReader 全局 settings 无 `tls_backend` 键 → 当前默认 LuaSec。**不代开**：k4tls 激活留用户菜单勾选（菜单三增强已部署），勾选后才走新 .so。
- 边界：测试残留已清（脚本/CA）；未动 Lua/设置；K5 segfault A/B 与互踢观察不受影响。
