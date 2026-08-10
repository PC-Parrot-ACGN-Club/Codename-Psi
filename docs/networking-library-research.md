# Bevy / Rust 联机库调研与 TDD 选型依据

**文档定位：** 立项时的联机选型调研与推荐依据，供 [TDD](TDD.md) R2 网络契约参考。原型与实现可调整版本或备选路线；定案后回写 TDD 与本文结论区。  
**调研日期：** 2026-08-10  
**决策范围：** R2 局域网双人 P2P BO3；首期 Linux、Windows（macOS 延后）；Bevy `0.19.x`；手动输入 `IP:端口`；60Hz 确定性模拟、回滚、状态校验与断线判负。

## 1. 结论

推荐采用 **`bevy_ggrs = "0.22"` + `ggrs = "0.13"` + GGRS 内置 UDP socket 与项目握手层**，并保持 `core` 为不依赖 Bevy 的确定性状态机。

该组合与当前联机需求匹配度最高：GGRS 原生面向 P2P 回滚；`bevy_ggrs 0.22` 明确对应 Bevy `0.19` 与 GGRS `0.13`，并只快照显式注册的 Bevy 组件和资源。[bevy_ggrs 文档](https://docs.rs/bevy_ggrs/0.22.0/bevy_ggrs/)

本项目的规则状态已经被设计为独立的 `core::MatchState`。R2 应让 GGRS 回滚该状态，客户端从回滚后的最新快照重建视觉层；对局版本、规则版本、角色和种子的握手，以及断线判负，均保留在 `net` 中实现。该边界避免把 UI、音频、socket 和渲染实体误纳入可回滚状态。

## 2. 需求基线与评估方法

| 需求 | 选型含义 |
| --- | --- |
| 双人、局域网、手动 `IP:端口` | 需要原生 UDP 或可替换的原生传输；无需信令、匹配、NAT 穿透。 |
| 操作型对战、60Hz | 输入应按 tick 传递，延迟与丢包通过预测、重发或输入延迟处理。 |
| 双端严格一致 | 输入同步优先于 ECS 状态复制；状态快照、恢复和 checksum 是必需能力。 |
| 对等双方 | 主机只负责监听、房间参数协商和判定流程；R2 不需要常驻权威服务器。 |
| 首期双桌面平台与 CI | 依赖必须声明支持 Bevy 0.19，且能覆盖 Linux/Windows 构建与无窗口同步测试。 |
| 断线判负 | 需要连接状态/超时事件；规则层接收该事件并终止比赛。 |

评分含义：`5` 为直接满足；`3` 为少量适配即可满足；`1` 为同步模型或版本不匹配。总分只用于排序，最终决策以原型验收为准。

| 方案 | Bevy 0.19 | P2P 回滚 | 手动 LAN | 与纯 `core` | 实施量（分高为少） | 适配度 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| GGRS + bevy_ggrs | 5 | 5 | 5 | 5 | 4 | **5 / 首选** |
| Lightyear | 5 | 4 | 3 | 4 | 2 | 3 / 备选 |
| Renet + bevy_renet | 5 | 1 | 5 | 5 | 3 | 3 / 精简备选 |
| Replicon + Renet | 5 | 1 | 3 | 2 | 2 | 2 / 架构偏离 |
| Quinnet | 1 | 1 | 4 | 4 | 3 | 1 / 版本不匹配 |
| Matchbox | 1 | 4 | 1 | 4 | 2 | 1 / 需求不匹配 |

## 3. 候选库逐项比对

### 3.1 GGRS `0.13` + bevy_ggrs `0.22` — 首选

**能力。** GGRS 是 Rust 的 GGPO 风格 P2P 回滚库，核心循环返回应用需要处理的请求，因此能由项目掌控快照、推进、恢复、checksum 与网络事件。[GGRS 项目文档](https://github.com/gschup/ggrs) `bevy_ggrs` 为 Bevy 接入提供固定帧推进、专用回滚调度，以及按注册项快照组件/资源的机制。[bevy_ggrs API 文档](https://docs.rs/bevy_ggrs/0.22.0/bevy_ggrs/)

**版本与技术优势。** `bevy_ggrs 0.22.0` 的兼容矩阵是 `Bevy 0.19` 与 `ggrs 0.13.0`；当前项目已锁定 Bevy `0.19` 和 GGRS `0.13`，只需补入该插件并锁定实际 patch 版本。[兼容矩阵](https://docs.rs/crate/bevy_ggrs/0.22.0) 其默认回滚频率为 60Hz，插件允许通过 `RollbackFrameRate` 显式设置；输入读取与回滚逻辑运行在独立调度中。[调度与配置文档](https://docs.rs/bevy_ggrs/0.22.0/bevy_ggrs/)

**项目适配。** 两个玩家、离散输入位集合、显式随机种子、纯整数状态与回放日志，均是回滚同步的理想输入。`MatchState: Clone` 可以成为快照边界；更推荐先把状态包装为一个已注册的 rollback resource，或在 GGRS 适配层实现快照/恢复，保持 `core` 无 Bevy 依赖。UDP 监听、加入、版本握手和断线超时属于 `net`，每端都运行相同逻辑。

**代价与风险。** 回滚期间表现层副作用必须隔离：音频、粒子、UI 提示只能由确认帧或可重放事件驱动。主机监听并不自动提供房间协议，握手包、`IP:端口` 解析、规则 hash 比对、端口错误提示和断线归因都需要项目实现。

**采用条件。** 执行 TDD 第 6 节的原型门槛：两本地端 1,000 tick，含预测与至少一次回滚，最终 checksum 一致；再完成两机 LAN BO3、版本不匹配拒绝和断线判负。通过后将版本写为精确版本，例如 `bevy_ggrs = "=0.22.0"`、`ggrs = "=0.13.0"`。

### 3.2 Lightyear `0.28` — 功能最广的备选

**能力。** Lightyear 是 Bevy 的模块化客户端/服务器联网库，提供 UDP、WebTransport、WebSocket、Steam 等传输，消息可靠性通道、输入缓冲与冗余发送、世界复制、预测/回滚、插值、带宽限制与诊断。[Lightyear 文档](https://docs.rs/lightyear/0.28.0/lightyear/)

**版本与技术优势。** `0.28` 明确支持 Bevy `0.19`，并依赖 `bevy_app` / `bevy_ecs 0.19`。[版本与依赖](https://docs.rs/crate/lightyear/0.28.0) 它的“只复制输入”的确定性复制模式可用于 lockstep 或 prediction/rollback；未来若项目扩展互联网、服务器权威、观战或跨平台 Web，迁移路径充足。

**项目适配。** 将 `core` 作为确定性模拟，传输每 tick 输入，并把主机作为轻量服务器，可以完成 R2。其输入缓存和冗余输入包有助于应对 UDP 丢包。

**代价与风险。** 本项目只有两名对等玩家和手动 LAN 连接，Lightyear 的服务器权威、实体复制、兴趣管理和多传输抽象超出范围。它要求将网络类型、协议与 Bevy 世界模型接入其框架；原型和调试面更大。适合产品路线已确定会在 R3 走服务器权威或互联网联机时采用。

**结论。** R2 保留为升级路线，不进入首轮原型；GGRS 原型未通过且团队需要服务器权威架构时启动替代原型。

### 3.3 Renet `2.0` + bevy_renet `5.0` — 轻量传输备选

**能力。** Renet 提供客户端/服务器连接管理、可靠有序、可靠无序与不可靠三类消息通道、分片重组；`renet_netcode` 提供认证与加密，传输层也允许自定义。[Renet 文档](https://docs.rs/renet/2.0.0/renet/) `bevy_renet` 提供 Bevy 的客户端/服务器插件、收发系统集和连接状态条件。[bevy_renet 文档](https://docs.rs/bevy_renet/5.0.0/bevy_renet/)

**版本与技术优势。** `bevy_renet 5.0` 的兼容矩阵对应 Bevy `0.19`，依赖 `renet 2.0.0`；它适合手工 `IP:端口` 的 host/client LAN 连接。[兼容矩阵](https://docs.rs/crate/bevy_renet/5.0.0) 对本项目可将对局输入走不可靠冗余通道，把握手、版本拒绝与赛果走可靠有序通道。

**项目适配。** 该方案和现有 `core` 边界高度兼容，网络层只需编码 `InputFrame`、握手和心跳；host 可同时运行 server 与 client。它让 R2 保持最小依赖集，并方便精确控制协议。

**代价与风险。** Renet 不提供回滚会话、输入预测、状态快照或 checksum 收敛机制。项目必须自行实现输入缓冲、预测、回滚窗口、状态保存/恢复与 desync 诊断；这复制了 GGRS 的核心工作，风险集中在最需要可靠性的同步层。

**结论。** 适合作为“固定输入延迟 lockstep”路线的传输层，前提是产品接受较高输入延迟并把回滚从 R2 范围移除；当前 PRD/TDD 的回滚目标下，优先级低于 GGRS。

### 3.4 bevy_replicon `0.41.1` + 传输插件 — 架构偏离

**能力。** Replicon 是服务器权威的 Bevy 世界复制库，覆盖远程事件、授权和自动组件复制；本体不绑定传输，常与 Renet 或 Quinnet 插件组合。[Replicon 文档](https://docs.rs/bevy_replicon/0.41.1/bevy_replicon/) 当前 `0.41.1` 依赖 Bevy `0.19`。[Cargo 元数据](https://docs.rs/crate/bevy_replicon/0.41.1/source/Cargo.toml)

**技术优势。** 若游戏演变为服务端权威的实体世界，它能显著减少复制样板代码，并提供成熟的客户端/服务器生命周期。

**项目适配。** 为 R2 的确定性棋盘状态逐字段复制会绕开“双方相同输入导出相同状态”的核心设计；主机也会成为权威服务器。可以仅用其可靠事件层，但价值很低，且 `core` 需要更深地映射成 Bevy 组件/资源。

**结论。** 不选。它的强项服务于服务器权威的状态复制，项目当前需要输入驱动的 P2P 回滚。

### 3.5 bevy_quinnet `0.19` — 当前 Bevy 版本不兼容

**能力。** Quinnet 是基于 QUIC 的 Bevy 网络库，提供 client/server、消息通道和示例；也提供 Replicon 传输集成。[Quinnet 文档](https://docs.rs/bevy_quinnet/0.19.0/bevy_quinnet/)

**版本事实。** 其 `0.19` 版本对应 Bevy `0.17`；已发布兼容矩阵没有 Bevy `0.19` 条目。[兼容矩阵](https://docs.rs/crate/bevy_quinnet/0.19.0)

**结论。** 不进入原型。QUIC 的连接可靠性与加密在未来互联网场景有价值，当前版本会迫使项目降级 Bevy 或维护未发布适配，代价高于收益。

### 3.6 Matchbox / bevy_matchbox `0.14` — 当前需求不匹配

**能力。** Matchbox 为 native 和 WASM 提供 WebRTC P2P socket、可靠/不可靠 data channel 和信令服务器，能与 GGRS 配合。[Matchbox 文档](https://docs.rs/bevy_matchbox/0.14.0/bevy_matchbox/)

**版本与场景。** `bevy_matchbox 0.14` 支持 Bevy `0.18`；其核心价值是浏览器 P2P、信令和 NAT 穿透。[兼容矩阵与说明](https://docs.rs/bevy_matchbox/0.14.0/bevy_matchbox/) R2 明确排除 NAT 穿透、互联网匹配和房间列表，且要求 Bevy 0.19。

**结论。** 不进入原型。后续若加入 Web/WASM 和互联网 P2P，Matchbox 可作为 GGRS 的 socket 替换候选。

## 4. 推荐的 R2 结构

```text
Bevy client
  ├─ 采集并量化本地输入 (60Hz)
  ├─ bevy_ggrs 固定帧调度
  │    └─ core::MatchState.step([P1Input, P2Input])
  ├─ 从最新回滚状态渲染 / 播放确认帧表现事件
  └─ net
       ├─ UDP socket（host bind / join connect）
       ├─ GGRS P2P session
       ├─ 握手：游戏版本、规则 hash、角色、seed、玩家槽位
       ├─ 状态校验交换与诊断
       └─ 心跳、超时、断线事件 → 比赛流程判负
```

GGRS `UdpNonBlockingSocket` 负责已开始会话的输入交换；`net` 在创建会话前完成 LAN 房间握手，并将对端地址和玩家槽位交给 `SessionBuilder`。控制协议需要与 GGRS 会话端口的生命周期统一设计，避免两个 socket 竞争同一端口。

数据职责如下：

| 数据 | 归属 | 回滚 | 传输 |
| --- | --- | --- | --- |
| `TickInput`（动作位与帧号） | `core` / GGRS 适配 | 是 | GGRS UDP |
| `MatchState`、RNG、计时器、垃圾/Fever 状态 | `core` | 是 | 本地快照；定期 checksum |
| 版本、规则 hash、角色、seed | `net` | 开局前锁定 | 可靠握手包 |
| 连接阶段、心跳、超时、断线原因 | `net` | 否 | 控制包 |
| 实体、动画、音频、UI、设置 | `client` | 否 | 不传输 |

## 5. TDD 应更新的决策与原型清单

1. 将“确切兼容版本待定”改为：**原型目标为 `bevy_ggrs = "=0.22.0"` 与 `ggrs = "=0.13.0"`**；任何升级均以兼容矩阵和首期双平台 CI 重新验证。
2. `net` 定义三个可单测接口：`HandshakeCodec`、`SessionDriver`、`ConnectionMonitor`。规则层只看 `TickInput` 和比赛终止事件。
3. 先做无窗口 `SyncTestSession`：固定 seed、预定输入日志、一次人为延迟/预测、回滚后最终 checksum 相同。该测试是网络技术栈的第一条红绿测试。
4. 再做双进程 UDP smoke test：host/join、成功握手、四类握手拒绝、输入往返、超时断线。最后才接入 Bevy UI 与两机验收。
5. 对声音、连锁特效、Fever 倒计时提示定义“确认帧触发”策略，并为重复播放建立回归测试；回滚不会改变规则结论，表现只从确认状态派生。

## 6. 决策记录

| 决策 | 结论 | 触发重新评估的条件 |
| --- | --- | --- |
| R2 同步模型 | GGRS P2P rollback | 原型 checksum 无法收敛，或 LAN 延迟下体验不可接受。 |
| Bevy 集成 | bevy_ggrs 0.22 | 首期双平台构建失败，或插件调度无法与独立 `core` 保持边界。 |
| 传输 | GGRS 内置 LAN UDP | 增加 Web、NAT 穿透或互联网房间时评估 Matchbox / Lightyear。 |
| 架构保留 | `core` 纯确定性，`net` 基础设施，`client` 表现层 | 产品转向服务器权威、观战或持久世界时评估 Lightyear / Replicon。 |
