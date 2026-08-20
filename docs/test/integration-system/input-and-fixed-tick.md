# 测试用例设计：输入与固定调度

**关联设计：** [本地输入采样](../../development/design/local-input-sampling.md)、[固定频率规则调度](../../development/design/fixed-tick-simulation.md)、[统一游戏动作与 Tick 输入](../../development/design/game-action-input.md)、[应用状态机](../../development/design/application-state-machine.md)、[UI 交互动作](../../development/design/ui-action-input.md)

**关联实现：** `../../../crates/client`、`../../../crates/game_core`

## 需求理解摘要

**功能：** 验证采样器、`game_core`、生产主调度、固定规则 tick、应用状态与设备生命周期的协作。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** Component Integration 范围内的输入到规则路径、60Hz 调度、暂停边界和设备适配。
**Test Basis：**

- [Confirmed] [固定频率规则调度](../../development/design/fixed-tick-simulation.md)：60Hz、SystemSet 顺序、唯一规则路径与运行边界。
- [Confirmed] [本地输入采样](../../development/design/local-input-sampling.md)：采样协作时序、press edge 和设备绑定。
- [Confirmed] [应用状态机](../../development/design/application-state-machine.md)与[UI 交互动作](../../development/design/ui-action-input.md)：暂停请求及输入领域边界。

**设计基线：** 使用最小客户端 App 或生产主调度验证协作顺序；只有完整协作才能证明的行为留在本稿。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不重复纯动作值和纯采样器行为（见 [游戏动作与 Tick 输入](../component/game-actions.md)与[客户端输入](../component/client-input.md)），也不定义完整应用状态主路径（见 [应用生命周期](application-lifecycle.md)）。

## 测试点清单

- sampler 输出经 `game_core` 归一化为 canonical input（TC-001）。
- 60Hz、`Input → Rules` 顺序、单次消费与确定性（TC-002～TC-005；TC-005 Concern: Determinism）。
- simulation 仅在 Match 运行，并在暂停后停止、恢复后延续（TC-006～TC-008）。
- 手柄 Start 与键盘 Escape 的暂停请求路径（TC-009～TC-010）。
- 生产主调度的同帧可见性、单帧多 tick、press edge 与设备生命周期（TC-011～TC-015）。
- 两个本地玩家的键盘默认绑定各自驱动自己的槽位（TC-016）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 场景 / 协作路径 | sampler 到 canonical input、暂停触发和生产调度 | TC-001、TC-006～TC-012 |
| 变形测试 | Update 次数变化时 fixed 规则结果保持一致 | TC-005 |
| 边界值分析 | 帧边界、单帧 tick 数和同帧 press/release | TC-011～TC-013 |
| 错误猜测 | 设备断开残留与接入顺序变化 | TC-014～TC-015 |
| 场景 / 协作路径 | 本地双人下两套键盘默认绑定各自到达自己的槽位 | TC-016 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | sampler 输出经 game_core 归一化形成 canonical input | P0 | Component Integration | — | Input；Client | 最小 sampler 与 game_core 输入协作路径可运行 | 采样可形成冲突组合的物理输入，并将 raw `PlayerActions` 交给 game_core 归一化 | 双旋转；软降+硬降 | game_core 收到 sampler 的 raw 输出，并分别形成无旋转、仅 HardDrop 的 canonical `PlayerActions` | [Confirmed] [本地输入采样：协作](../../development/design/local-input-sampling.md#协作) |
| TC-002 | fixed schedule 配置为 60Hz | P0 | Component Integration | — | Client | 最小客户端 app 注册 simulation 能力 | 读取 fixed schedule 的频率配置 | frequency=`60Hz` | fixed schedule 的配置频率为 60Hz；本用例不通过累计浮点时间推导 tick 数 | [Confirmed] [固定频率规则调度：Fixed System Set](../../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-003 | 每个 fixed tick 严格执行 Input 后 Rules | P0 | Component Integration | — | Input；Client | Input/Rules 运行标记可观测，app 已处于 `AppState::Match` | 推进多个受控 fixed tick | 3 ticks | 观察序列为 `[Input, Rules] × 3`，每个 Rules 都能读取同 tick Input 产物 | [Confirmed] [固定频率规则调度：Fixed System Set](../../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-004 | 每个 fixed tick 只形成并消费一次 TickInputs | P0 | Component Integration | — | Input；Client | 输入生产与规则消费次数及本 tick 标记可观测，app 已处于 `AppState::Match` | 推进少量受控 fixed tick | 3 个带可区分输入标记的 tick | 生产数=消费数=3；每个标记恰好消费一次；无跨 tick 复用或漏用 | [Confirmed] [固定频率规则调度：Fixed System Set](../../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-005 | 相同初始规则状态与量化输入产生相同 fixed 规则结果 | P0 | Component Integration | Determinism | Client | 两个 app 处于相同初始 `AppState::Match` 与规则状态，使用相同量化输入序列 | 以不同数量的普通 Update 穿插执行相同数量的受控 fixed tick | 相同的 6 tick canonical input；不同 Update 交错序列 | 额外 Update 不推进规则；两者消费相同输入序列后得到相同 tick 数与规则状态/checksum | [Confirmed] [固定频率规则调度：Fixed System Set](../../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-006 | fixed tick 仅在 `AppState::Match` 执行，全部非 Match 状态均不产生 Input/Rules | P0 | Component Integration | — | Client | 最小客户端 app 注册状态机与 simulation 能力，Input/Rules 执行次数可观测 | 分别在每个非 Match 状态执行受控 fixed tick，再在 Match 执行受控 fixed tick | `Boot`、`MainMenu`、`ModeSelect`、`CharacterSelect`、`Settings`、`Paused`、`Result`；`Match` | 七个非 Match 状态下 Input/Rules 执行次数均为 0；Match 下两个阶段均按受控 tick 数执行 | [Confirmed] [固定频率规则调度：运行边界](../../development/design/fixed-tick-simulation.md#运行边界) |
| TC-007 | `Match → Paused` 后对局 simulation 立即停止 | P0 | Component Integration | — | Client | 当前 `Match`，Input/Rules 执行计数器已运行若干 tick | 提交 `Paused` 请求并运行状态提交，随后提供若干 fixed 执行机会 | 转移前计数=N；转移后 3 个 fixed 执行机会 | 状态转移当拍起不再产生新的 Input/Rules 执行；转移后计数保持为 N | [Confirmed] [固定频率规则调度：运行边界](../../development/design/fixed-tick-simulation.md#运行边界) |
| TC-008 | `Paused → Match` 恢复后规则状态从暂停前继续推进 | P0 | Component Integration | — | Client | Match 中已推进至可观察的非初始规则状态 S，并记录已消费 tick 数 | 转移至 `Paused`、停留若干受控 fixed tick、转移回 `Match` 并再推进若干 tick | 暂停前状态=S；暂停期间 3 ticks；恢复后 3 ticks | 恢复起点为暂停前状态 S，暂停期间没有重置或跳变；恢复后 tick 计数从暂停前继续累加 | [Confirmed] [固定频率规则调度：运行边界](../../development/design/fixed-tick-simulation.md#运行边界) |
| TC-009 | `Match` 语境下固定 Start 按键由 `client::input` 直接提出 `PauseRequested` | P0 | Component Integration | — | Input；Client | 当前 `AppState::Match`；`client::input` 使用固定手柄 Start 按键；状态迁移结果可观测 | 采样到手柄 Start 按键 press edge 并运行状态提交周期 | 手柄 Start press edge，`AppState::Match` | 当前状态提交为 `Paused`；该触发不产生 `UIAction` 或 `GameAction`（生命周期效果由 TC-007 覆盖）；迁移请求和内部协作类型由实现决定 | [Confirmed] [应用状态机：协作](../../development/design/application-state-machine.md#协作)；[UI 交互动作：边界](../../development/design/ui-action-input.md#边界) |
| TC-010 | 键盘 `Escape` 只在 `Match` 下提出 `Pause` | P1 | Component Integration | — | Input；Client | 最小 Bevy App 已注册项目根插件 | 分别在 `MainMenu` 与 `Match` 状态下按下 `Escape` | `AppState::MainMenu`；`AppState::Match` | `MainMenu` 下不提出 `PauseRequested`，不产生 `UIAction`，且该次按下不滞留到进入 `Match` 后生效；`Match` 下提出 `PauseRequested` 并进入 `Paused` | [Confirmed] [应用状态机：协作](../../development/design/application-state-machine.md#协作) |
| TC-011 | 生产主调度下同帧输入进入当帧 fixed tick | P0 | Component Integration | — | Input；Client | 最小客户端 app 使用生产主调度（不手动驱动 fixed schedule），虚拟时间可受控推进，当前 `AppState::Match` | 在一帧内注入已绑定输入的按下并推进一帧，读取该帧 fixed tick 的 `TickInputs`；随后释放并同样推进一帧 | 每帧推进 `1/60s`；持续动作 `Left` | 按下所在帧的 fixed tick 已包含该动作，释放所在帧的 fixed tick 不再包含；两者均不延后到下一帧；本用例不规定采样系统的具体调度点 | [Inferred] [固定频率规则调度：协作时序](../../development/design/fixed-tick-simulation.md#协作时序)、[Fixed System Set](../../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-012 | 单帧补跑多个 fixed tick 时共享该帧采样结果 | P1 | Component Integration | — | Input；Client | 同 TC-011，且可让单帧推进多个 fixed tick | 在一帧内保持一个持续动作并产生一次尚未提交的一次性动作，推进使该帧运行三个 fixed tick | 单帧推进 `3/60s`；保持 `Left` 并产生一次 `HardDrop` press edge | 三个 tick 均含 `Left`；`HardDrop` 只出现在第一个 tick，后两个 tick 不重复产生 | [Inferred] [固定频率规则调度：协作时序](../../development/design/fixed-tick-simulation.md#协作时序) |
| TC-013 | 设备适配层保留同帧内完成的 press edge | P0 | Component Integration | — | Input；Client | 最小客户端 app 使用真实 `ButtonInput` 与生产设备捕获路径 | 在同一帧内对一次性动作的默认绑定执行按下并松开，随后推进 fixed tick | 参数化 `HardDrop`、`RotateClockwise`、`RotateCounterClockwise`；每组在同一帧内完成 press 与 release | 每组在随后的 fixed tick 产生一次对应动作；采样依据 press edge 而非采样时刻的按住状态；持续按住不因此重复产生 | [Confirmed] [本地输入采样：一次性动作采样](../../development/design/local-input-sampling.md#一次性动作采样)；[Inferred] [捕获物理输入](../../development/design/local-input-sampling.md#捕获物理输入) |
| TC-014 | 手柄断开清除该设备在采样状态中的残留 | P1 | Component Integration | — | Input；Client | 已接入手柄并绑定到某本地玩家槽位，采样结果可观测 | 参数化三种断开情形后继续推进 fixed tick | 按住方向时断开；无输入时断开；断开后重新接入且不按任何键 | 断开后的 fixed tick 不再产生该方向动作；无输入断开不改变其它玩家的采样结果；重连不带入断开前的按下状态 | [Inferred] [本地输入采样：设备与玩家绑定](../../development/design/local-input-sampling.md#设备与玩家绑定) |
| TC-015 | 手柄接入顺序变化不改变已绑定玩家的槽位 | P2 | Component Integration | — | Input；Client | 两个手柄可分别接入与断开，各自可注入可区分输入 | 依次接入两个手柄，断开先接入的一个，再接入第三个，并在各阶段采样 | pad A→最小空闲槽位、pad B→次一槽位；A 断开后接入 pad C | B 在 A 断开后保持原槽位；C 取得空出的槽位；采样结果不随设备遍历顺序改变 | [Inferred] [本地输入采样：设备与玩家绑定](../../development/design/local-input-sampling.md#设备与玩家绑定) |
| TC-016 | 本地双人下每个玩家的键盘默认绑定只驱动自己的槽位 | P0 | Component Integration | — | Input；Client | 生产主调度、本地双人模式、真实规则实例已进入可操作阶段 | 分别按下 P1 与 P2 的默认键，各推进一帧 | P1 固定方向 `KeyA`；P2 固定方向 `ArrowLeft` 与 P2 自己的旋转绑定 `Numpad1` | 按下者槽位的 canonical input 含对应动作且活动组横移一格；另一槽位的 canonical input 为空且活动组不动 | [Confirmed] [本地输入采样：设备与玩家绑定](../../development/design/local-input-sampling.md#设备与玩家绑定) |

## 风险查漏

调度频率、阶段顺序、单次消费、暂停生命周期、同帧输入、edge 保留和设备槽位稳定性均有直接用例。

