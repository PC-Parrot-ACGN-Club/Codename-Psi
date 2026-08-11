# 本地输入采样器 Spec

**状态：** Confirmed
**主分类：** Component
**相关模块：** `client::input`
**关联文档：** [统一游戏动作与 Tick 输入 Spec](game-action-input.md)、[本地输入采样 Contract](../contract/local-input-sampling.md)、[TDD §3–§4](../../TDD.md)

## 目标

从 Bevy 提供的键盘和手柄状态中采样本地玩家输入，并在每个 60Hz fixed tick 生成原始逻辑动作集合。

## 数据模型

### `PlayerInputBindings`

每名本地玩家拥有独立绑定配置，将物理输入映射到逻辑动作。

采样时使用当前 PlayerInputBindings。绑定编辑、重复绑定提示和覆盖交互由设置页面负责。

### 采样状态

LocalInputSampler 保存 fixed tick 输入采样所需的临时状态。

持续动作根据 fixed tick 到来时对应物理输入是否处于按下状态产生。

一次性动作需要记住自上一个 fixed tick 后发生、尚未提交给规则层的按下操作。一次按下提交到某个 fixed tick 后，不会在后续 tick 因持续按住而重复产生。

### 动作语义

| 动作                     | fixed tick 输出语义           |
| ---------------------- |-------------------------------|
| Left                   | fixed tick 到来时仍按住则产生 |
| Right                  | fixed tick 到来时仍按住则产生 |
| SoftDrop               | fixed tick 到来时仍按住则产生 |
| HardDrop               | 每次物理按下产生一次          |
| RotateClockwise        | 每次物理按下产生一次          |
| RotateCounterClockwise | 每次物理按下产生一次          |

## 行为

### 捕获物理输入

- 输入：Bevy 键盘和手柄事件/状态。
- 处理：根据当前 `PlayerInputBindings` 保存 fixed tick 采样所需的当前输入状态和尚未提交的一次性操作。
- 输出：可供下一个 fixed tick 采样的本地输入状态。
- 错误语义：未绑定的物理输入不产生逻辑动作。

### 合并多个物理输入源

多个物理输入源可以映射为同一个逻辑动作。

例如：

```text
Keyboard A
Gamepad Left
→ Left
```

只要任一物理来源在当前 fixed tick 满足该动作的产生条件，raw PlayerActions 中包含一次对应逻辑动作。

相同逻辑动作的多个物理来源合并为同一动作。

### 持续动作采样

Left、Right 和 SoftDrop 使用 fixed tick 边界的当前按下状态。

fixed tick 到来时对应物理输入处于按下状态
→ 本 tick 包含该动作

持续按住时，连续 fixed tick 都会产生对应逻辑动作。

在两个 fixed tick 之间完成按下和松开的持续动作不会产生规则输入。

### 一次性动作采样

HardDrop、RotateClockwise 和 RotateCounterClockwise 每次物理按下产生一次逻辑动作。

自上一个 fixed tick 后发生尚未提交的按下操作
→ 本 tick 包含该动作
→ 该次操作完成提交

一次按下即使发生并结束于两个 fixed tick 之间，也会在下一个 fixed tick 产生一次动作。

持续按住不会在后续 fixed tick 自动重复产生该动作。

### 生成原始逻辑动作

每个 fixed tick，采样器为每名本地玩家生成一个 raw `PlayerActions`。

raw `PlayerActions` 可以暂时同时包含互斥语义，例如：

```text
Left + Right
SoftDrop + HardDrop
RotateClockwise + RotateCounterClockwise
```

这些逻辑组合由 `game_core::input` 的统一归一化规则处理。

## 不变量

- 设备 ID、Bevy 输入类型和物理按键语义只存在于 client。
- P1/P2 等本地玩家使用独立 `PlayerInputBindings`。
- 同一逻辑动作可以由多个物理输入源共同产生。
- 一次性动作的每次物理按下最多提交到一个 fixed tick。
- 一次性动作发生在两个 fixed tick 之间时仍能被下一 fixed tick 采样。
- 采样器不决定 Left / Right、软降 / 硬降或双旋转冲突的最终逻辑结果。
- 普通 Update 的执行频率和渲染帧率不改变上述 fixed tick 输入语义。

## 验收条件

- 键盘和常见手柄都能通过绑定产生六个规则动作。
- P1/P2 使用不同绑定时互不覆盖。
- 两个物理输入源同时映射同一动作时，本 tick 只得到该逻辑动作。
- 持续按住 Left、Right 或 SoftDrop 时，连续 fixed tick 都能采样到对应动作。
- 在两个 fixed tick 之间完成按下和松开的 Left、Right 或 SoftDrop 不产生规则输入。
- 每次按下 HardDrop 或旋转动作只在一个 fixed tick 中产生一次。
- 持续按住 HardDrop 或旋转输入不会在后续 fixed tick 自动重复产生动作。
- 一个完整发生在两个 fixed tick 之间的 HardDrop 或旋转按下操作可以在下一个 fixed tick 被采样。
- 采样器输出可以直接交给 game_core::input 归一化，不包含设备相关数据。

## Test Basis

- [Confirmed] Issue #11：要求键盘与常见手柄转换为项目逻辑动作，P1/P2 独立映射。
- [Confirmed] TDD §3：规则核心只接收量化到 tick 的动作。
- [Confirmed] TDD §4：键盘与手柄映射由客户端维护为可序列化动作表，每名本地玩家独立。
- [Confirmed] 当前审核结论：本地输入处理作为独立采样 Component；持续动作按 fixed tick 边界的当前按下状态采样；硬降与旋转每次物理按下产生一次且不会因持续按住重复；逻辑动作冲突统一交给 game_core::input。
