# 固定频率规则调度 Contract

**状态：** Confirmed
**主分类：** Component Integration
**相关模块：** Bevy Schedule、`client::input`、`client::simulation`、后续 `game_core::MatchState`
**关联文档：** [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)、[应用状态机 Spec](../component/application-state-machine.md)、[游戏基础设施运行架构](../system/game-infrastructure-architecture.md)、[TDD §3–§4](../../TDD.md)

## 目的

建立固定 60Hz 的规则推进路径，使规则时间基准独立于普通 `Update`、窗口刷新率和渲染帧率。

该 Contract 只定义 fixed schedule 中输入准备与规则推进的先后关系，以及规则状态只能由 fixed 规则路径推进的约束。

## 参与者与职责

| 参与者 | 提供 | 依赖 |
| --- | --- | --- |
| Bevy fixed schedule | 60Hz 固定执行机会 | Bevy 时间与调度 |
| `client::input` | 当前 fixed tick 的 `TickInputs` | 本地采样、AI 或后续网络输入 |
| `client::simulation` | fixed schedule 中的阶段组织与规则调用桥 | `game_core::input`、后续规则状态 |
| `game_core::MatchState`（后续） | 确定性规则状态推进 | 当前规则状态、当前 tick 输入 |

## 数据契约

| 数据 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `TickInputs` | 输入阶段 | 规则阶段 | 当前 fixed tick 的全部参与者逻辑输入 |
| rule state | `game_core` | simulation / 后续消费者 | 只由 fixed 规则路径修改的对局状态 |

## 协作时序

1. 引擎完成本帧设备输入更新后、进入 fixed 调度之前，客户端执行一次设备采样，把本帧的物理按下状态与尚未提交的 press edge 写入采样状态。
2. fixed tick 到来时进入 `FixedGameSet::Input`。
3. 输入阶段收集当前 participant slots 的 canonical `PlayerActions` 并形成 `TickInputs`。
4. `FixedGameSet::Input` 完成后进入 `FixedGameSet::Rules`。
5. 规则阶段使用本 tick 的 `TickInputs` 推进规则状态一次。
6. fixed tick 结束。
7. 下一次 fixed tick 重复第 2–6 步。
8. 普通 `Update` 处理与规则推进无关的客户端工作，排在本帧全部 fixed tick 之后。

设备采样每帧执行一次。同一帧补跑多个 fixed tick 时，这些 tick 共享该帧的采样结果：持续动作在每个 tick 重复成立，一次性动作只在其中第一个 tick 提交一次。

## Fixed System Set

固定调度只定义两个有序阶段：

```text
FixedGameSet::Input
    → FixedGameSet::Rules
```

### `FixedGameSet::Input`

负责在当前 fixed tick 内完成规则输入准备。

该阶段完成时必须已经形成完整 `TickInputs`。

### `FixedGameSet::Rules`

消费当前 tick 的 `TickInputs` 并推进规则状态一次。

后续玩法实现可以在该阶段接入 `MatchState.step(...)` 或等价规则入口。

如果未来出现必须在同一 fixed tick 中、且明确要求位于规则推进之后执行的新职责，再由对应设计增加新的 SystemSet；Issue #11 不预留空阶段。

## 运行边界

- `FixedGameSet::Input` 与 `FixedGameSet::Rules` 只在 `AppState::Match` 中执行。

### Pause 行为

- 进入 `AppState::Paused` 后，对局模拟停止：`FixedGameSet::Input` 与 `FixedGameSet::Rules` 不再执行。
- 从 `Paused` 返回 `Match` 后，对局模拟从暂停前已有的规则状态继续推进。

实现可以使用 Bevy 的 run condition、状态调度或其它等价机制表达上述运行边界，只需满足该可观察语义。

## 调度约束

- fixed schedule 的规则频率配置为 60Hz。
- `FixedGameSet::Input` 先于 `FixedGameSet::Rules` 执行。
- 每个 fixed tick 只形成并消费一次对应的 `TickInputs`。
- 影响胜负或规则结果的状态只由 `FixedGameSet::Rules` 推进。
- 设备采样排在引擎输入更新之后、本帧全部 fixed tick 之前，且每帧只执行一次。
- 普通 `Update` 不承担规则输入采样。
- 普通 `Update`、渲染和 UI 不直接推进规则状态。
- 普通 `Update` 的执行次数可以变化，不改变 fixed tick 的规则语义。
- 规则计时和网络同步所需的时间/帧标识由引入对应需求的领域定义

## 双方承诺

- 输入阶段：在规则阶段开始前提供完整的当前 tick 输入。
- 规则阶段：每个 fixed tick 使用一次当前 `TickInputs` 推进规则状态。
- simulation：通过 Bevy 调度保证 `Input → Rules` 的顺序。
- 其它客户端系统：不绕过 fixed 规则路径修改规则事实。

## 验收条件

- 客户端 fixed schedule 配置为 60Hz。
- 同一帧内发生的物理输入，在该帧的 fixed tick 中即可被输入阶段观察到，不延后到下一帧。
- 每个 fixed tick 中，输入阶段先完成 `TickInputs`，规则阶段随后执行一次。
- 普通 `Update` 执行频率变化不会改变相同 fixed 时间范围内的规则推进语义。
- 普通 `Update`、渲染和 UI 无法直接推进规则状态。
- 后续 `MatchState` 可以直接接入 `FixedGameSet::Rules`，无需修改输入与调度边界。
- `FixedGameSet::Input` 与 `FixedGameSet::Rules` 只在 `AppState::Match` 中执行。
- 进入 `AppState::Paused` 后，两个 SystemSet 均不再执行。
- 从 `Paused` 返回 `Match` 后，规则状态从暂停前的状态继续推进，不因暂停而重置或跳变。

## Test Basis

- [Confirmed] Issue #11：要求建立 60Hz 游戏规则固定更新路径，使渲染帧率与规则推进独立。
- [Confirmed] TDD §3：规则以 60Hz fixed tick 运行，规则核心消费已经量化到 tick 的动作。
- [Confirmed] TDD §4：对局模拟使用固定调度；设备输入采集排在引擎输入更新之后、固定调度之前；UI、音频和渲染使用普通更新调度。
- [Confirmed] [应用状态机 Spec](../component/application-state-machine.md)：`AppState` 包含 `Match` 与 `Paused`，`Match ⇄ Paused` 为有效状态边。
- [Confirmed] 当前审核结论：fixed schedule 只固定 `Input → Rules` 两段；测试实现方式不进入 Contract；`FixedGameSet::Input` 与 `FixedGameSet::Rules` 只在 `AppState::Match` 中执行；`Paused` 停止对局模拟，恢复后从暂停前状态继续推进。
- [Inferred] 待确认设计结论：设备采样必须排在引擎输入更新之后、本帧 fixed 调度之前。原协作时序把设备采样放在普通 `Update`，而 Bevy 主调度中 fixed 调度先于 `Update` 执行，该时序无法实现，规则 tick 只能读到上一帧的设备状态。本条修订后，采样点由实现选择具体调度位置，Contract 只约束相对顺序与「每帧一次」。
