# K4WereadRust

K4weread Phase 1 工程仓（评估报告 §七 分期）：**无 fork 的 Rust 增益 + 配置级瘦身**。
上游报告：`~/reports/K4weread激进形态评估_KOReader裁剪+weread主程序化+Rust加持-2026-11-08.md`。

## 范围（Phase 1 = S1 + S3 解耦部分）

| 交付物 | 内容 | 目录 |
|---|---|---|
| P1-A rustls 网络组件 | Rust 传输组件（bench=musl 静态 / FFI .so=armel glibc 动态，依赖 getauxval 包装与 -z defs/-z nodelete）+ LuaJIT FFI，替换 weread client.lua 传输层；rustls+ring+webpki-roots；**默认后端 LuaSec，R4 前不翻闸（auto 不是回滚开关）** | `k4tls/` `lua/` `docs/INTEGRATION.md` |
| P1-A bench | 握手/整请求基准 CLI（设备可跑）+ host bench；性能持平即换（R4） | `k4tls/src/bin/` |
| P1-B 配置级瘦身 | 插件禁用清单（35→3）+ 字体裁剪（68.4M→CJK+西文），176.3M→~115-120M（S1 真实值；~70M 需叠加 S2），纯打包层不碰代码 | `tools/slim_koreader.py` `docs/SLIMMING.md` |

不在本仓：Phase 2 fork 裁剪（触发条件未满足）、weread 业务逻辑 Rust 重写（评估报告 §四判定不做）。

## 工作法

- **ub-mode**（~/.zcode/skills/ub-mode）：First Make it Work → Then Make it Right → Follow Practice。TDD、可验证单元切分、提交信息说清"为什么、改了什么、如何验证"、动作日志记入 `LOG.md`（追加式）。
- **orbital_handoff**（~/.qoder/skills/orbital_handoff）：`handoffs/` 是本地交接通道——**永不 commit/push/上传**。每批次结束 CREATE 套件并更新下方指针。

## 项目事实（接手必读）

- 构建配方沿 k4refresh（~/work/k4refresh，v0.1.10 真机实证）：target `armv7-unknown-linux-musleabihf`，cdylib+rlib，release profile `opt-level="s"` + lto + strip，panic 不设 abort（FFI 边界 catch_unwind，内部异常返回 -99，不拖死宿主 KOReader）。
- K4 设备：MX50 单核 Cortex-A8，250MB RAM，FW 4.1.4；KOReader v2026.07.1（K4 世代冻结）；k4refresh 硬浮点 musl 静态二进制真机可跑 → musleabihf 是已验证目标。
- weread_K4.koplugin（~/weread_K4.koplugin，v9.7 设备运行中）：`weread/lib/client.lua` 是 FFI 接入点（socket.http + LuaSec + socketutil 超时/限额语义）；B-3 mock 网关 `dev/mock_gateway.py` 把 `https://*.weread.qq.com` 重定向到测试源，可驱动真实传输层。
- 集成纪律：client.lua 改动落在 weread_K4.koplugin 仓（433 spec 是护栏），本仓提供 FFI 绑定 + 补丁 + 文档；传输后端默认 `luasec`，显式 `auto`/`k4tls` 才尝试候选库，真机 bench 通过前不得翻默认值（R2/R4）。
- ring 交叉编译需要 musl 目标 C 编译器（本机 x-tools 只有 gnu-eabi）——`k4tls/build.sh` 含下载/校验步骤；GitHub TLS 失败走 LAN 代理 `http://192.168.2.153:16492`。
- rustup 在 `~/.cargo/bin`（PATH 未含），armv7-musleabihf std 已安装。
- 设备 SSH：`root@<IP>:2222`（KOReader dropbear），设备间歇在线（DHCP 漂移 + USB 拔插）。

## Current Handoff

- Latest: `handoffs/2026-11-08-181521-k4wereadrust-phase1/HANDOFF.md`（文末含 2026-11-08 晚间追加：K5 实测结局与更正，以其为准）
- Branch: main @ e7ea207（68a5e48 传输硬化 / 3c48aa4 瘦身守卫 / e7ea207 夹具与文档；主仓无远端仅本地提交）
- Status: K5 临时目录传输链全绿（-joff）、KUAL 扩展已装 K5；R4 业务基准、K4 真机、瘦身部署、翻闸/发版待条件或授权

任何 agent 开始工作前，先读上面的 HANDOFF.md。这份指针由 orbital_handoff 维护，手工交接时请同步更新。
