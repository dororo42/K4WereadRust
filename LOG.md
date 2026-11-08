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
