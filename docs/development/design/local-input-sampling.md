# 本地输入采样

**相关模块：** `client::input`、`game_core::input`
**关联文档：** [统一游戏动作与 Tick 输入](game-action-input.md)、[UI 交互动作](ui-action-input.md)、[固定频率规则调度](fixed-tick-simulation.md)、[TDD §3–§4](../../TDD.md)

## 目标

从 Bevy 提供的键盘和手柄状态中采样本地玩家输入，在每个 60Hz fixed tick 生成 raw `PlayerActions`，并交给 `game_core::input` 归一化。

## 数据模型

### press edge

press edge（按下沿）指一个物理输入从“未按下”变为“按下”的那一次状态转变，是采样的最小事件单位。

- 一次 press edge 对应一次逻辑按下，与该次按下持续多久无关。
- 保持按住不再产生新的 press edge；必须先回到未按下状态，下一次按下才构成新的 press edge。
- 松开不产生 press edge。
- press edge 是**事件**，“按住状态”（held / pressed）是**状态**：前者描述一次转变，后者描述某一时刻的取值。一次按下和松开如果都发生在两次采样之间，采样时读到的按住状态为“未按下”，但这次 press edge 依然成立。

### `PlayerInputBindings`

每名本地玩家拥有独立绑定配置，将物理输入映射到逻辑动作。采样时使用当前 `PlayerInputBindings`。

### 输入源

输入源指一名本地玩家当前唯一生效的物理设备类别：持有手柄时为该手柄，否则为键盘。

一名玩家在任一时刻只有一个输入源，另一类设备上的绑定完全不被读取。键盘是默认输入源，也是手柄断开后的回退——桌面端必然存在键盘。

### 采样状态

`LocalInputSampler` 保存 fixed tick 输入采样所需的临时状态：当前物理按下状态，以及自上一个 fixed tick 后发生、尚未提交给规则层的 press edge。采样状态中不存在没有对应在线设备的按下记录。

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

- 时机：每帧执行一次，排在引擎完成本帧输入更新之后、本帧 fixed 调度之前，使当帧输入对当帧 fixed tick 可见。
- 输入：Bevy 键盘和手柄事件/状态，包含本帧的 press edge 信息，而不只是当前按住状态。
- 处理：根据当前 `PlayerInputBindings` 保存 fixed tick 采样所需的当前输入状态和尚未提交的一次性操作。
- 输出：可供本帧 fixed tick 采样的本地输入状态。
- 错误语义：未绑定的物理输入不产生逻辑动作。

一次性动作依据 press edge 记录，因此在同一帧内完成按下和松开的操作仍会留下待提交记录。

### 设备与玩家绑定

- 键盘的两套固定布局按玩家编号归属：槽位 0 取一套，槽位 1 取另一套，与谁正在使用键盘无关。
- 手柄在接入时分配到当前空闲的最小本地玩家槽位，该绑定在手柄保持连接期间不变；采样结果不依赖每帧的设备遍历顺序。
- 分配即刻改变该玩家的[输入源](#输入源)，对局中同样如此；释放槽位同样即刻回退到键盘。
- 设备停止为一名玩家生效时，清除它在采样状态中的全部残留：已按下的可配置动作、fixed 方向与 UI 持有状态。手柄断开与键盘被手柄取代都属于这种情形——不再被读取的设备无法再报告松开。
- 手柄重新接入按上述规则重新分配槽位，不恢复断开前的按下状态。
- 暂停输入同样按输入源判定：没有任何玩家以键盘为输入源时，键盘上的暂停键不产生暂停。

一名玩家的输入源被手柄占据时，其键盘绑定不产生任何逻辑动作，两个设备类别不并行生效。

### 合并多个物理输入源

同一输入源内的多个物理位可以映射为同一个逻辑动作。

```text
DPadLeft
LeftStickX < -0.5
→ Left
```

只要任一物理来源在当前 fixed tick 满足该动作的产生条件，raw `PlayerActions` 中包含一次对应逻辑动作。相同逻辑语义的多个物理来源不属于冲突。跨设备类别不发生合并：另一类设备此刻并不为该玩家产生输入。

### 摇杆方向判定

左摇杆按阈值 `0.5` 转换为方向输入：分量绝对值超过阈值视为该方向处于按下状态，未超过视为未按下。摇杆方向在采样中与十字键、键盘方向键等价，多来源按上一节合并。该阈值同时适用于 `GameAction` 与 [`UIAction`](ui-action-input.md) 的方向判定。

方向输入不提供连发（DAS/ARR）：持续保持方向只按持续动作语义在每个 fixed tick 产生一次逻辑动作，采样器不额外插入重复触发。

### 持续动作采样

Left、Right 和 SoftDrop 使用 fixed tick 边界的当前按下状态。

```text
fixed tick 到来时对应物理输入处于按下状态
→ 本 tick 包含该动作
```

持续按住时，连续 fixed tick 都会产生对应逻辑动作。在两个 fixed tick 之间完成按下和松开的持续动作不会产生规则输入。

### 一次性动作采样

HardDrop、RotateClockwise 和 RotateCounterClockwise 每次物理按下产生一次逻辑动作。

```text
自上一个 fixed tick 后发生尚未提交的 press edge
→ 本 tick 包含该动作
→ 该次 press edge 完成提交
```

一次 press edge 即使其按下和松开都发生在两个 fixed tick 之间，也会在下一个 fixed tick 产生一次动作。每次 press edge 最多提交到一个 fixed tick，持续按住不会在后续 fixed tick 自动重复产生该动作。

### 生成原始逻辑动作

每个 fixed tick，采样器为每名本地玩家生成一个 raw `PlayerActions`。raw `PlayerActions` 可以同时包含互斥语义：

```text
Left + Right
SoftDrop + HardDrop
RotateClockwise + RotateCounterClockwise
```

这些逻辑组合由 `game_core::input` 的统一归一化规则处理。

## 协作

| 数据 | 生产方 | 消费方 | 语义 |
| --- | --- | --- | --- |
| `PlayerInputBindings` | 设置系统 | `LocalInputSampler` | 每名本地玩家的物理输入映射 |
| [输入源](#输入源) | 本主题 | [页面导航与焦点](page-navigation.md)、[UI 交互动作](ui-action-input.md) | 每名本地玩家当前生效的设备类别 |
| raw `PlayerActions` | `LocalInputSampler` | `game_core::input` | 当前 fixed tick 采样到的逻辑动作，可包含互斥组合 |
| canonical `PlayerActions` | `game_core::input` | 后续输入装配阶段 | 应用统一冲突规则后的规则输入 |

1. 每帧在引擎输入更新之后、fixed 调度之前捕获一次输入状态和尚未提交的一次性操作。
2. fixed tick 到来时，采样器根据当前物理按下状态、pending press edge 和当前生效绑定生成每名本地玩家的 raw `PlayerActions`。
3. 相同逻辑动作的多个物理来源在采样阶段合并。
4. raw `PlayerActions` 交给 `game_core::input` 进行逻辑动作归一化。

## 边界

- 本文不定义互斥逻辑组合的最终含义（见[统一游戏动作与 Tick 输入：逻辑动作归一化](game-action-input.md#逻辑动作归一化)）。采样端只报告该 tick 采样到哪些逻辑动作。
- 本文不定义绑定编辑、重复物理按键提示与覆盖交互（见[本机用户设置](user-settings.md)）。
- 本文不定义固定绑定的具体键位（见[UI 交互动作：绑定来源表](ui-action-input.md#绑定来源表)）。
- 本文不定义方向移动的重复节奏（见[玩法设计](../../gameplay.md)）。
- 设备 ID、Bevy 输入类型和物理按键语义只存在于 client，不进入 `game_core`。
- 普通 `Update` 的执行频率和渲染帧率不改变本文的 fixed tick 输入语义。

## Test Basis

- [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)：要求键盘与常见手柄转换为项目逻辑动作，P1/P2 独立映射，并为 P1、P2、AI、网络提供统一逻辑动作入口。
- [TDD §3](../../TDD.md)：规则核心只接收量化到 tick 的动作。
- [TDD §4](../../TDD.md)：键盘与手柄映射由客户端维护为可序列化动作表，每名本地玩家独立。
