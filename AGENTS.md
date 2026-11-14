# K4WereadRust

K4weread Phase 1 工程仓（评估报告 §七 分期）：**无 fork 的 Rust 增益 + 配置级瘦身**。
上游报告：`~/reports/K4weread激进形态评估_KOReader裁剪+weread主程序化+Rust加持-2026-11-08.md`。

## 范围（Phase 1 = S1 + S3 解耦部分）

| 交付物 | 内容 | 目录 |
|---|---|---|
| P1-A rustls 网络组件 | Rust 传输组件（bench=musl 静态 / FFI .so=armel glibc 动态，依赖 getauxval 包装与 -z defs/-z nodelete）+ LuaJIT FFI，替换 weread client.lua 传输层；rustls+ring+webpki-roots；**仓库默认后端 LuaSec**；设备翻闸由 env / 菜单决定（2026-11-10 K5 翻闸，11-12 因 segfault 归因 A/B 暂回 luasec，见 `LOG.md`）；`auto` 不是回滚开关 | `k4tls/` `lua/` `docs/INTEGRATION.md` |
| P1-A bench | 握手/整请求基准 CLI（设备可跑）+ host bench；性能持平即换（R4） | `k4tls/src/bin/` |
| P1-B 配置级瘦身 | 插件禁用清单（35→3）+ 字体裁剪（68.4M→CJK+西文），176.3M→~115-120M（S1 真实值；~70M 需叠加 S2），纯打包层不碰代码 | `tools/slim_koreader.py` `docs/SLIMMING.md` |

不在本仓：Phase 2 fork 裁剪（触发条件未满足）、weread 业务逻辑 Rust 重写（评估报告 §四判定不做）。

## 工作法

- **ub-mode**（~/.zcode/skills/ub-mode）：First Make it Work → Then Make it Right → Follow Practice。TDD、可验证单元切分、提交信息说清"为什么、改了什么、如何验证"、动作日志记入 `LOG.md`（追加式）。
- **orbital_handoff**（~/.qoder/skills/orbital_handoff）：`handoffs/` 是本地交接通道——**永不 commit/push/上传**。每批次结束 CREATE 套件并更新下方指针。

## 项目事实（接手必读）

- 构建配方沿 k4refresh（~/work/k4refresh，v0.1.10 真机实证）：target `armv7-unknown-linux-musleabihf`，cdylib+rlib，release profile `opt-level="s"` + lto + strip，panic 不设 abort（FFI 边界 catch_unwind，内部异常返回 -99，不拖死宿主 KOReader）。
- K4 设备：MX50 单核 Cortex-A8，250MB RAM，FW 4.1.4；KOReader v2026.07.1（K4 世代冻结）；k4refresh 硬浮点 musl 静态二进制真机可跑 → musleabihf 是已验证目标。
- weread_K4.koplugin（`~/weread_K4.koplugin`，已发版 **v9.12**；v9.13 候选未发版）：`weread/lib/client.lua` 是 FFI 接入点（socket.http + LuaSec + socketutil 超时/限额语义）；B-3 mock 网关 `dev/mock_gateway.py` 把 `https://*.weread.qq.com` 重定向到测试源，可驱动真实传输层。**本仓 `lua/k4tls.lua`、`lua/transport.lua` 两个绑定文件与插件仓 `weread/lib/` 逐字节一致**（`tools/check_lua_mirror.py` 机器校验，插件仓为源）；`lua/spec/` 因本仓用 `tools/test_k4tls_lua.lua`（Lua 5.1 shim）、插件仓用 busted 而各自维护，只做语义对齐、**不**逐字节一致。2026-11-12 曾因插件仓新增 `backend_explicit` 而漂移，已同步并加门禁（I-1）。该仓有并行在制品，只读不改需协调。
- 集成纪律：client.lua 改动落在 weread_K4.koplugin 仓（445 spec 是护栏），本仓提供 FFI 绑定 + 补丁 + 文档；R4 bench PASS 后用户批准翻闸——2026-11-10 起 K5 生产 k4tls（koreader.sh env 常态注入，shim aa24b9c5），浸泡线闭环，LuaSec 保留为显式回退（`WEREAD_TLS_BACKEND=luasec`）。
- ring 交叉编译需要 musl 目标 C 编译器（本机 x-tools 只有 gnu-eabi）——`k4tls/build.sh` **只检查**交叉 gcc 是否存在，缺失时打印下载提示并 `exit 3`；真正的下载与 sha256 固化校验在 `.github/workflows/ci.yml` 的 bench / so 两个 job（2026-11-12 审查 I-7 更正）。`build.sh` 另导出 `CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABI_LINKER`，覆盖 `.cargo/config.toml` 中仅作本机示例的绝对路径。GitHub TLS 失败走 LAN 代理 `http://192.168.2.153:16492`（可用 `K4_LAN_PROXY` 覆盖）。
- 跨仓依赖与同步顺序（资产 → 插件包 → 设备）见 `docs/RELEASE_ASSETS.md` §4；三仓改动范围见各仓 HANDOFF.md 的「三仓协同」节。
- rustup 在 `~/.cargo/bin`（PATH 未含），armv7-musleabihf std 已安装。
- 设备 SSH：`root@<IP>:2222`（KOReader dropbear），设备间歇在线（DHCP 漂移 + USB 拔插）。

## Current Handoff

- Latest: `handoffs/2026-11-14-180409-k4wereadrust-weak-network-fix/HANDOFF.md`
- Branch: main @ 76636f3
- Status: 修复批次完成（弱网重试修复已推+CI绿+K5已部署实证+K4待部署；下一步见 HANDOFF「立即下一步」）

任何 agent 开始工作前，先读上面的 HANDOFF.md。这份指针由 orbital_handoff 维护，手工交接时请同步更新。
