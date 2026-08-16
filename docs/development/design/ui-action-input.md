# UI 交互动作

**相关模块：** `client::input`
**关联文档：** [统一游戏动作与 Tick 输入](game-action-input.md)、[本地输入采样](local-input-sampling.md)、[应用状态机](application-state-machine.md)、[TDD §3–§4](../../TDD.md)、[PRD §5.2](../../PRD.md)、[表现与 UI 设计](../../presentation.md)

## 目标

为 client 侧 UI 交互定义独立的逻辑动作类型 `UIAction`，表达焦点移动、当前选项确认与页面返回等语义。

## 数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `UIAction` | UI 交互领域消费的逻辑动作 | 与规则领域 `GameAction` 是两个不同的领域类型 |

```rust
enum UIAction {
    Left,
    Right,
    Up,
    Down,
    Confirm,
    Back,
}
```

语义：

- `Left` / `Right` / `Up` / `Down`：将当前 UI 焦点移动到对应方向的控件或选项；
- `Confirm`：确认当前选中的选项；
- `Back`：返回上一页面，或执行对应页面的返回行为。

## 物理绑定关系

`UIAction` 的六个成员都不拥有独立的物理绑定，每个成员都是某一路物理输入在 UI 上下文中的含义，由输入上下文决定产生哪个领域类型的动作：

- `Left` / `Right` / `Up` / `Down` 复用固定的物理方向输入，与 `GameAction::Left` / `GameAction::Right` 同源。
- `Confirm` / `Back` 复用玩家自己的旋转绑定，与 `GameAction::RotateCounterClockwise` / `GameAction::RotateClockwise` 同源。玩家改绑旋转键，菜单键随之改变。
- `GameAction` 中仅 `SoftDrop` / `HardDrop` / `RotateClockwise` / `RotateCounterClockwise` 保留用户可配置绑定，`UIAction` 不出现在可配置绑定列表中。

### 绑定来源表

绑定对每名本地玩家分别成立：

| 逻辑动作 | 来源 | P1 键盘 | P2 键盘 | 手柄 |
| --- | --- | --- | --- | --- |
| `Left` / `Right` | 固定 | `A` / `D` | `←` / `→` | 十字键、左摇杆对应方向 |
| `Up` / `Down` | 固定 | `W` / `S` | `↑` / `↓` | 十字键、左摇杆对应方向 |
| `Confirm` | `RotateCounterClockwise` 的绑定 | `J` | `Numpad1` | South |
| `Back` | `RotateClockwise` 的绑定 | `K` | `Numpad2` | East |

`Confirm` / `Back` 一列给出的是[默认输入绑定](user-settings.md#默认输入绑定)下的取值，随该绑定变化；其余各列不可配置。左摇杆的方向判定阈值与连发语义见[本地输入采样：摇杆方向判定](local-input-sampling.md#摇杆方向判定)。

`UIAction` 与规则动作取自同一个[输入源](local-input-sampling.md#输入源)：一名玩家持有手柄时，其键盘上的方向、确认与返回都不产生 `UIAction`。两名玩家都持有手柄时，键盘不驱动任何焦点环。

因为 `Confirm` / `Back` 没有独立后备键，旋转绑定不允许为空：[输入绑定冲突](user-settings.md#输入绑定冲突)拒绝任何会使某个动作失去绑定的编辑。

### 按键提示

各菜单页面在画面左下角与右下角常驻按键提示，左角属于 P1、右角属于 P2，各自给出该玩家当前 `Confirm` 与 `Back` 的实际按键名，取自该玩家的[输入源](local-input-sampling.md#输入源)。提示随绑定与设备变化即时更新，因此不看设置页也能知道当前如何确认与返回。

## 边界

- 本文不定义 `Pause`（见[应用状态机：协作](application-state-machine.md#协作)）。`Pause` 不是 `UIAction` 的成员，暂停输入也不产生 `UIAction`。
- 本文不定义规则领域动作（见[统一游戏动作与 Tick 输入](game-action-input.md)）。`UIAction` 只在 UI 输入上下文中被消费，不进入 `TickInputs` 或规则核心输入；与 `GameAction` 共享物理输入或位位置不改变各自领域类型的消费语义。
- 本文不定义设置页面键位配置 UI 的交互与展示方式（见[本机用户设置](user-settings.md)）。

## Test Basis

- [PRD §5.2](../../PRD.md)：每位玩家的输入映射包含确认和返回/暂停等界面交互，与左右、软降、硬降、旋转等对局动作并列。
- [TDD §3](../../TDD.md)：确认、返回、暂停等 client 交互由客户端输入上下文处理，不进入规则核心 tick 动作。
- [表现与 UI 设计](../../presentation.md)：焦点顺序、确认/返回在所有菜单中的键盘与手柄操作保持一致。
