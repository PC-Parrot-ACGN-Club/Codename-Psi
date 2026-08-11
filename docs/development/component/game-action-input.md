# 统一游戏动作与 Tick 输入 Spec

**状态：** Confirmed  
**主分类：** Component  
**相关模块：** `core::input`  
**关联文档：** [UI 交互动作 Spec](ui-action-input.md)、[TDD §3–§4](../../TDD.md)、[PRD §5.2](../../PRD.md)

## 目标

定义规则层消费的逻辑游戏动作，以及一个规则 tick 中各参与者的输入。

## 术语与数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `GameAction` | 会被规则核心直接解释的逻辑动作 | 与键盘、手柄、AI 和网络来源解耦 |
| `PlayerActions` | 一名参与者在一个 tick 中成立的动作集合 | 使用位集合表达，可同时包含多个动作 |
| participant slot | 本场比赛中稳定的参与者位置 | slot 编号在整场比赛中保持稳定 |
| `TickInputs` | 一个规则 tick 的全部参与者输入 | 最多 8 个参与者，使用前 `len` 个槽位 |

### `GameAction`

```rust
enum GameAction {
    Left,
    Right,
    SoftDrop,
    HardDrop,
    RotateClockwise,
    RotateCounterClockwise,
}
```

`GameAction` / `PlayerActions` 表达规则领域输入；client UI 输入使用独立的 `UIAction` 语义（见[UI 交互动作 Spec](ui-action-input.md)）。`Confirm`、`Back`、`Pause` 等界面交互语义属于 client 表现领域，不进入规则核心动作集合。`GameAction` 与 `UIAction` 可以在不同输入上下文中复用相同物理按键绑定或位位置，但各自的消费语义保持独立。

### `PlayerActions`

`PlayerActions` 使用固定宽度位集合表达。

该类型只表达“本 tick 哪些逻辑动作成立”，不记录动作来自持续按住、按下沿、键盘、手柄、AI 或网络。

具体 Rust 底层整数类型与 bit 编号在确定日志或网络稳定编码需求时再锁定。

### `TickInputs`

```rust
pub const MAX_PLAYERS: usize = 8;

pub struct TickInputs {
    players: [PlayerActions; MAX_PLAYERS],
    len: u8,
}
```

语义：

```text
players[0..len] = 当前 tick 的有效参与者输入
players[len..8] = PlayerActions::EMPTY
0 <= len <= 8
```

当前 R1/R2 产品对局使用前两个 participant slots；core 输入模型固定支持最多 8 个参与者，包括本地玩家、AI 或后续网络参与者。

## 行为

### 构造参与者输入

- 输入：0–8 个按 participant slot 排列的 `PlayerActions`。
- 处理：复制到 `players[0..len]`，其余槽位填充 `PlayerActions::EMPTY`。
- 输出：`TickInputs`。
- 错误语义：超过 8 个参与者时拒绝构造。

### 逻辑动作归一化

`PlayerActions` 在进入规则消费前应用统一冲突规则。

#### 水平方向冲突

```text
Left + Right
→ clear Left
→ clear Right
```

结果为该 tick 不产生水平移动方向。

#### 旋转方向冲突

```text
RotateClockwise + RotateCounterClockwise
→ clear both
```

结果为该 tick 不产生旋转方向。

#### 下落冲突

```text
SoftDrop + HardDrop
→ HardDrop
```

硬降优先于软降。

其它动作组合保持原样。多个动作在同一 tick 中的具体规则执行顺序由玩法规则设计定义。

### 连续动作

连续多个 tick 都包含同一动作时，规则核心按连续 tick 序列解释持续输入。

例如：

```text
tick 100: SoftDrop
tick 101: SoftDrop
tick 102: SoftDrop
```

规则可以据此保持软降状态。core 不需要额外的 held 标记。

### 一次性动作

硬降和旋转是否只在一次物理按压中出现一个 tick，由输入生产方负责。

core 只消费已经形成的 `PlayerActions`，不区分动作来源或采样方式。

## 不变量

- `core::input` 不包含 Bevy `KeyCode`、Gamepad ID、窗口焦点或设备信息。
- `PlayerActions` 只表达当前 tick 的逻辑动作集合。
- participant slot 在一场比赛中保持稳定身份。
- `players[len..MAX_PLAYERS]` 始终为 `PlayerActions::EMPTY`。
- 相同规则状态与相同 `TickInputs` 具有相同逻辑语义。
- 本地玩家、AI 和网络输入可以产生相同的 `PlayerActions`。
- 逻辑动作冲突使用同一套归一化规则，不因输入来源改变。

## 验收条件

- 可以在无 Bevy 环境中构造最多 8 个参与者的 `TickInputs`。
- 当前双人对局使用 `players[0]`、`players[1]`，其余槽位为空。
- 4 人或 8 人 AI 仿真无需修改输入类型。
- `Left + Right` 归一化为空方向。
- `RotateClockwise + RotateCounterClockwise` 归一化为空旋转。
- `SoftDrop + HardDrop` 归一化为 `HardDrop`。
- 连续 tick 可以通过重复出现的逻辑动作表达持续输入。
- `PlayerActions` 可复制、比较，并为后续稳定日志或网络编码保留实现空间。

## 待审核设计点

- [Inferred] exact bit encoding 留到确定性日志或网络协议需要稳定编码时锁定。

## Test Basis

- [Confirmed] Issue #11：要求为 P1、P2、AI 和后续网络输入建立统一游戏动作入口。
- [Confirmed] TDD §3：规则核心只接收已经量化到 tick 的游戏动作。
- [Confirmed] PRD §5.2：定义左右、软降、硬降、顺/逆时针旋转、确认和返回/暂停等玩家操作。
- [Confirmed] 当前审核结论：core 逻辑动作只包含六个对局动作；`PlayerActions` 使用位集合；逻辑输入不区分 held / edge；输入模型固定支持最多 8 个 participant slots；逻辑冲突按本文规则归一化。
