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

物理绑定分为固定绑定与可配置绑定两类，由设置系统（`client::settings`）在绑定编辑时遵循：

- `UIAction` 的六个成员全部使用固定绑定，不出现在可配置绑定列表中。
- `Left` / `Right` 与 `GameAction::Left` / `GameAction::Right` 复用同一路物理方向输入，由输入上下文决定产生哪个领域类型的动作，不是各自独立的配置项。
- `GameAction` 中仅 `SoftDrop` / `HardDrop` / `RotateClockwise` / `RotateCounterClockwise` 保留用户可配置绑定。

### 固定绑定表

固定绑定对每名本地玩家分别成立：

| 逻辑动作 | P1 键盘 | P2 键盘 | 手柄 |
| --- | --- | --- | --- |
| `Left` / `Right` | `A` / `D` | `←` / `→` | 十字键、左摇杆对应方向 |
| `Up` / `Down` | `W` / `S` | `↑` / `↓` | 十字键、左摇杆对应方向 |
| `Confirm` | `Space` | `Enter` | South |
| `Back` | `LeftShift` | `RightShift` | East |

左摇杆的方向判定阈值与连发语义见[本地输入采样：摇杆方向判定](local-input-sampling.md#摇杆方向判定)。

### 全局 `Escape`

`Escape` 是不区分玩家的全局键：在 `AppState::Match` 下触发 `Pause`，在其它可返回页面下等价于 `Back`。两个上下文互斥，因此不产生歧义。

## 边界

- 本文不定义 `Pause`（见[应用状态机：请求处理](application-state-machine.md#请求处理)）。`Pause` 不是 `UIAction` 的成员。
- 本文不定义规则领域动作（见[统一游戏动作与 Tick 输入](game-action-input.md)）。`UIAction` 只在 UI 输入上下文中被消费，不进入 `TickInputs` 或规则核心输入；与 `GameAction` 共享物理输入或位位置不改变各自领域类型的消费语义。
- 本文不定义设置页面键位配置 UI 的交互与展示方式（见[本机用户设置](user-settings.md)）。

## Test Basis

- [PRD §5.2](../../PRD.md)：每位玩家的输入映射包含确认和返回/暂停等界面交互，与左右、软降、硬降、旋转等对局动作并列。
- [TDD §3](../../TDD.md)：确认、返回、暂停等 client 交互由客户端输入上下文处理，不进入规则核心 tick 动作。
- [表现与 UI 设计](../../presentation.md)：焦点顺序、确认/返回在所有菜单中的键盘与手柄操作保持一致。
