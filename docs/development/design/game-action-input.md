# 统一游戏动作与 Tick 输入

**相关模块：** `game_core::input`
**关联文档：** [UI 交互动作](ui-action-input.md)、[本地输入采样](local-input-sampling.md)、[TDD §3–§4](../../TDD.md)、[PRD §5.2](../../PRD.md)

## 目标

定义规则层消费的逻辑游戏动作，以及一个规则 tick 中各参与者的输入。

## 数据模型

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

### `PlayerActions`

`PlayerActions` 使用固定宽度位集合表达本 tick 哪些逻辑动作成立，不记录动作来自持续按住、press edge、键盘、手柄、AI 还是网络。本地玩家、AI 和网络输入可以产生相同的 `PlayerActions`。

#### 位编码

底层类型为 `u8`，bit 编号如下：

| bit | 动作 |
| --- | --- |
| 0 | `Left` |
| 1 | `Right` |
| 2 | `SoftDrop` |
| 3 | `HardDrop` |
| 4 | `RotateClockwise` |
| 5 | `RotateCounterClockwise` |

bit 6–7 为保留位，始终为 0。

该编码是稳定格式：确定性验证日志、快照校验和与网络输入编码都依赖它。因此 bit 编号不随 `GameAction` 声明顺序的调整而改变；新增逻辑动作时使用保留位，不重排既有 bit。

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

R1/R2 产品对局使用前两个 participant slots；输入模型固定支持最多 8 个参与者，包括本地玩家、AI 或网络参与者。相同规则状态与相同 `TickInputs` 具有相同逻辑语义。

`players` 与 `len` 为私有字段，槽位通过访问器读取：有效槽位返回该槽位的 `PlayerActions`，超出 `len` 的槽位返回“无该参与者”。因此“参与者不存在”与“该参与者本 tick 无动作”在调用方看到的类型上已经区分开。

尾部 `PlayerActions::EMPTY` 是规范化要求而非语义：它保证相同逻辑输入具有唯一字节表示，使相等性、哈希、校验和与网络编码不受尾部残留影响。字段私有化后尾部不对外可见。

## 行为

### 构造参与者输入

- 输入：0–8 个按 participant slot 排列的 `PlayerActions`。
- 处理：复制到 `players[0..len]`，其余槽位填充 `PlayerActions::EMPTY`。
- 输出：`TickInputs`。
- 错误语义：超过 8 个参与者时拒绝构造。

### 逻辑动作归一化

`PlayerActions` 在进入规则消费前应用统一冲突规则，规则不因输入来源改变。

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

其它动作组合保持原样。

### 连续动作

连续多个 tick 都包含同一动作时，规则核心按连续 tick 序列解释持续输入。

```text
tick 100: SoftDrop
tick 101: SoftDrop
tick 102: SoftDrop
```

规则据此保持软降状态。

### 一次性动作

硬降和旋转是否只在一次物理按压中出现一个 tick，由输入生产方负责。`game_core` 只消费已经形成的 `PlayerActions`。

## 边界

- 本文不定义物理设备、按键语义与采样方式（见[本地输入采样](local-input-sampling.md)）。`game_core::input` 不包含 Bevy `KeyCode`、Gamepad ID 或窗口焦点。
- 本文不定义 UI 交互动作（见[UI 交互动作](ui-action-input.md)）。`Confirm`、`Back`、`Pause` 属于 client 表现领域，不进入规则核心动作集合。两个领域可以复用相同物理按键绑定或位位置，各自的消费语义保持独立。
- 本文不定义多个动作在同一 tick 中的规则执行顺序（见[玩法设计](../../gameplay.md)）。

## Test Basis

- [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)：要求为 P1、P2、AI 和网络输入建立统一游戏动作入口。
- [TDD §3](../../TDD.md)：规则核心只接收已经量化到 tick 的游戏动作。
- [PRD §5.2](../../PRD.md)：定义左右、软降、硬降、顺/逆时针旋转、确认和返回/暂停等玩家操作。
