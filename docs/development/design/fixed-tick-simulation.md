# 固定频率规则调度

**相关模块：** Bevy Schedule、`client::input`、`client::simulation`、`game_core::MatchState`
**关联文档：** [应用状态机](application-state-machine.md)、[本地输入采样](local-input-sampling.md)、[游戏基础设施运行架构](../system/game-infrastructure-architecture.md)、[TDD §3–§4](../../TDD.md)

## 目标

建立固定 60Hz 的规则推进路径，使规则时间基准独立于普通 `Update`、窗口刷新率和渲染帧率。本文定义 fixed schedule 中输入准备与规则推进的先后关系，以及规则状态只能由 fixed 规则路径推进的约束。

## 数据模型

| 数据 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `TickInputs` | 输入阶段 | 规则阶段 | 当前 fixed tick 的全部参与者逻辑输入 |
| rule state | `game_core` | simulation / 后续消费者 | 只由 fixed 规则路径修改的对局状态 |

## Fixed System Set

固定调度只定义两个有序阶段：

```text
FixedGameSet::Input
    → FixedGameSet::Rules
```

**`FixedGameSet::Input`** 负责在当前 fixed tick 内完成规则输入准备，该阶段完成时必须已经形成完整 `TickInputs`。

**`FixedGameSet::Rules`** 消费当前 tick 的 `TickInputs` 并推进规则状态一次。玩法实现在该阶段接入 `MatchState.step(...)` 或等价规则入口。

fixed schedule 的规则频率配置为 60Hz。

## 协作

| 参与者 | 提供 | 依赖 |
| --- | --- | --- |
| Bevy fixed schedule | 60Hz 固定执行机会 | Bevy 时间与调度 |
| `client::input` | 当前 fixed tick 的 `TickInputs` | 本地采样、AI 或网络输入 |
| `client::simulation` | fixed schedule 中的阶段组织与规则调用桥 | `game_core::input`、规则状态 |
| `game_core::MatchState` | 确定性规则状态推进 | 当前规则状态、当前 tick 输入 |

### 协作时序

1. 引擎完成本帧设备输入更新后、进入 fixed 调度之前，客户端执行一次设备采样，把本帧的物理按下状态与尚未提交的 press edge 写入采样状态。
2. fixed tick 到来时进入 `FixedGameSet::Input`。
3. 输入阶段收集当前 participant slots 的 canonical `PlayerActions` 并形成 `TickInputs`。
4. `FixedGameSet::Input` 完成后进入 `FixedGameSet::Rules`。
5. 规则阶段使用本 tick 的 `TickInputs` 推进规则状态一次。
6. fixed tick 结束。
7. 下一次 fixed tick 重复第 2–6 步。
8. 普通 `Update` 处理与规则推进无关的客户端工作，排在本帧全部 fixed tick 之后。

设备采样每帧执行一次。同一帧补跑多个 fixed tick 时，这些 tick 共享该帧的采样结果：持续动作在每个 tick 重复成立，一次性动作只在其中第一个 tick 提交一次。

## 运行边界

`FixedGameSet::Input` 与 `FixedGameSet::Rules` 只在 `AppState::Match` 中执行。

进入 `AppState::Paused` 后，对局模拟停止：两个 SystemSet 均不再执行。从 `Paused` 返回 `Match` 后，对局模拟从暂停前已有的规则状态继续推进。

实现可以使用 Bevy 的 run condition、状态调度或其它等价机制表达上述运行边界，只需满足该可观察语义。

## 边界

- 本文不定义规则输入的采样方式（见[本地输入采样](local-input-sampling.md)）。普通 `Update` 不承担规则输入采样。
- 本文不定义规则计时与网络同步所需的时间/帧标识，由引入对应需求的领域定义。
- 影响胜负或规则结果的状态只由 `FixedGameSet::Rules` 推进；普通 `Update`、渲染和 UI 不直接推进规则状态，其执行次数变化不改变 fixed tick 的规则语义。

## Test Basis

- [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)：要求建立 60Hz 游戏规则固定更新路径，使渲染帧率与规则推进独立。
- [TDD §3](../../TDD.md)：规则以 60Hz fixed tick 运行，规则核心消费已经量化到 tick 的动作。
- [TDD §4](../../TDD.md)：对局模拟使用固定调度；设备输入采集排在引擎输入更新之后、固定调度之前；UI、音频和渲染使用普通更新调度。
- [应用状态机](application-state-machine.md)：`AppState` 包含 `Match` 与 `Paused`，`Match ⇄ Paused` 为有效状态边。
