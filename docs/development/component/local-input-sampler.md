# 本地输入采样器 Spec

**状态：** Confirmed
**主分类：** Component
**相关模块：** `client::input`
**关联文档：** [统一游戏动作与 Tick 输入 Spec](game-action-input.md)、[本地输入采样 Contract](../contract/local-input-sampling.md)、[TDD §3–§4](../../TDD.md)

## 目标

从 Bevy 提供的键盘和手柄状态中采样本地玩家输入，并在每个 60Hz fixed tick 生成原始逻辑动作集合。

## 数据模型

### press edge

press edge（按下沿）指一个物理输入从“未按下”变为“按下”的那一次状态转变，是采样的最小事件单位。

- 一次 press edge 对应一次逻辑按下，与该次按下持续多久无关。
- 保持按住不再产生新的 press edge；必须先回到未按下状态，下一次按下才构成新的 press edge。
- 松开不产生 press edge。
- press edge 是**事件**，“按住状态”（held / pressed）是**状态**：前者描述一次转变，后者描述某一时刻的取值。一次按下和松开如果都发生在两次采样之间，采样时读到的按住状态为“未按下”，但这次 press edge 依然成立。

采样器据此保留尚未提交给规则层的 press edge，见“一次性动作采样”。

### `PlayerInputBindings`

每名本地玩家拥有独立绑定配置，将物理输入映射到逻辑动作。

采样时使用当前 PlayerInputBindings。绑定编辑、重复绑定提示和覆盖交互由设置页面负责。

### 采样状态

LocalInputSampler 保存 fixed tick 输入采样所需的临时状态。

持续动作根据 fixed tick 到来时对应物理输入是否处于按下状态产生。

一次性动作需要记住自上一个 fixed tick 后发生、尚未提交给规则层的 press edge。一次 press edge 提交到某个 fixed tick 后，不会在后续 tick 因持续按住而重复产生。

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

- 时机：每帧执行一次，排在引擎完成本帧输入更新之后、本帧 fixed 调度之前。
- 输入：Bevy 键盘和手柄事件/状态，包含本帧的 press edge 信息，而不只是当前按住状态。
- 处理：根据当前 `PlayerInputBindings` 保存 fixed tick 采样所需的当前输入状态和尚未提交的一次性操作。
- 输出：可供本帧 fixed tick 采样的本地输入状态。
- 错误语义：未绑定的物理输入不产生逻辑动作。

一次性动作依据 press edge 记录，因此在同一帧内完成按下和松开的操作仍会留下待提交记录；只读取“当前是否按住”不满足本节要求。

### 设备与玩家绑定

- 键盘按 `PlayerInputBindings` 固定映射到对应本地玩家。
- 手柄在接入时分配到当前空闲的最小本地玩家槽位，该绑定在手柄保持连接期间不变；采样结果不依赖每帧的设备遍历顺序。
- 手柄断开时，清除该设备在采样状态中的全部残留：已按下的可配置动作、fixed 方向、UI 持有状态与 pause 持有状态，并释放其占用的玩家槽位。
- 断开不影响其它设备为同一玩家提供的输入：键盘按自身状态继续采样。
- 手柄重新接入按上述规则重新分配槽位，不恢复断开前的按下状态。

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

### 摇杆方向判定

左摇杆按阈值 `0.5` 转换为方向输入：分量绝对值超过阈值视为该方向处于按下状态，未超过视为未按下。摇杆方向在采样中与十字键、键盘方向键等价，多来源按上一节合并。

方向输入不提供连发（DAS/ARR）：持续保持方向只按持续动作语义在每个 fixed tick 产生一次逻辑动作，采样器不额外插入重复触发。规则层需要的移动重复节奏由玩法设计定义。

### 持续动作采样

Left、Right 和 SoftDrop 使用 fixed tick 边界的当前按下状态。

fixed tick 到来时对应物理输入处于按下状态
→ 本 tick 包含该动作

持续按住时，连续 fixed tick 都会产生对应逻辑动作。

在两个 fixed tick 之间完成按下和松开的持续动作不会产生规则输入。

### 一次性动作采样

HardDrop、RotateClockwise 和 RotateCounterClockwise 每次物理按下产生一次逻辑动作。

自上一个 fixed tick 后发生尚未提交的 press edge
→ 本 tick 包含该动作
→ 该次 press edge 完成提交

一次 press edge 即使其按下和松开都发生在两个 fixed tick 之间，也会在下一个 fixed tick 产生一次动作。

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
- 采样在每帧固定执行一次并早于该帧的 fixed tick，采样与其消费之间不隔帧。
- 采样状态中不存在没有对应在线设备的按下记录。
- 设备断开后，该设备贡献的任何动作不会在后续 fixed tick 继续产生。
- 普通 Update 的执行频率和渲染帧率不改变上述 fixed tick 输入语义。

## 验收条件

- 键盘和常见手柄都能通过绑定产生六个规则动作。
- P1/P2 使用不同绑定时互不覆盖。
- 两个物理输入源同时映射同一动作时，本 tick 只得到该逻辑动作。
- 持续按住 Left、Right 或 SoftDrop 时，连续 fixed tick 都能采样到对应动作。
- 在两个 fixed tick 之间完成按下和松开的 Left、Right 或 SoftDrop 不产生规则输入。
- 每次按下 HardDrop 或旋转动作只在一个 fixed tick 中产生一次。
- 持续按住 HardDrop 或旋转输入不会在后续 fixed tick 自动重复产生动作。
- 一个按下和松开都发生在两个 fixed tick 之间的 HardDrop 或旋转 press edge 可以在下一个 fixed tick 被采样。
- 采样器输出可以直接交给 game_core::input 归一化，不包含设备相关数据。
- 左摇杆分量绝对值超过 `0.5` 时对应方向成立，未超过时不成立，且与十字键、键盘方向合并为同一逻辑动作。
- 持续保持方向输入时，采样器不产生连发，每个 fixed tick 只按持续动作语义产生一次动作。
- 同一帧内按下的一次性动作在该帧的 fixed tick 即可产生动作，不延后一帧。
- 玩家按住方向时拔出手柄，后续 fixed tick 不再产生该方向动作。
- 手柄在无输入状态下断开，不改变其它玩家的采样结果。
- 手柄重连后可以重新产生动作，且不带入断开前的按下状态。
- 两个手柄的接入顺序变化不会把已绑定玩家的输入交换到另一槽位。

## Test Basis

- [Confirmed] Issue #11：要求键盘与常见手柄转换为项目逻辑动作，P1/P2 独立映射。
- [Confirmed] TDD §3：规则核心只接收量化到 tick 的动作。
- [Confirmed] TDD §4：键盘与手柄映射由客户端维护为可序列化动作表，每名本地玩家独立。
- [Confirmed] 当前审核结论：本地输入处理作为独立采样 Component；持续动作按 fixed tick 边界的当前按下状态采样；硬降与旋转每次物理按下产生一次且不会因持续按住重复；逻辑动作冲突统一交给 game_core::input。
- [Confirmed] 当前审核结论：左摇杆方向阈值 `0.5`，与十字键、键盘方向等价合并；采样器不提供方向连发。
- [Inferred] 待确认设计结论：采样每帧一次且早于本帧 fixed tick；一次性动作依据 press edge 而非按住状态记录。原 Spec 未规定采样时机，实现把采样放在普通 `Update`，落在 fixed 调度之后，导致输入延后一帧且同帧内完成的短按被丢弃。
- [Inferred] 待确认设计结论：手柄按接入顺序绑定到空闲的最小玩家槽位并在连接期间保持稳定；断开时清除该设备的全部残留按下状态。原 Spec 未定义设备断开、重连与 device↔player 绑定，实现按每帧设备遍历顺序分配槽位，且断开后保留已按下的方向。
