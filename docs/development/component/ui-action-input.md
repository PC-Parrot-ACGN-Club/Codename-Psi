# UI 交互动作 Spec

**状态：** v1
**主分类：** Component
**相关模块：** `client::input`
**关联文档：** [统一游戏动作与 Tick 输入 Spec](game-action-input.md)、[应用状态机 Spec](application-state-machine.md)、[TDD §3–§4](../../TDD.md)、[PRD §5.2](../../PRD.md)、[presentation.md](../../presentation.md)

## 目标

为 client 侧 UI 交互定义独立的逻辑动作类型 `UIAction`，表达焦点移动、当前选项确认与页面返回等语义。

## 术语与数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `UIAction` | UI 交互领域消费的逻辑动作 | 与规则领域 `GameAction` 是两个不同的领域类型 |

### `UIAction`

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

## 与 `GameAction` 的领域关系

`GameAction`（见[统一游戏动作与 Tick 输入 Spec](game-action-input.md)）与 `UIAction` 是两个不同的领域类型，分别服务于规则输入与 UI 交互两个独立的消费上下文；两者可以复用相同物理按键绑定或位位置，但各自的消费语义保持独立，详见[统一游戏动作与 Tick 输入 Spec](game-action-input.md)。

## 待审核设计点

- [Inferred] 六个 `GameAction` 位与全部 `UIAction` 位之间的完整对应关系尚未确定，包括 `SoftDrop`、`HardDrop`、`RotateClockwise`、`RotateCounterClockwise` 对应哪个（或是否对应）`UIAction`，以及 `Confirm` / `Back` 使用的具体 bit 与完整 bit layout。
- [Inferred] `Pause` 不属于规则层 `GameAction`，可以参与客户端 `Match → Paused` 流程（见[应用状态机 Spec](application-state-machine.md)）；`Pause` 是否属于 `UIAction` 尚未决定，本轮不引入第三套 action 类型表达它。

## 不变量

- `UIAction` 只在 UI 输入上下文中被消费，不进入 `TickInputs` 或规则核心输入。
- `UIAction` 与 `GameAction` 是两个独立的领域类型，不共享同一个裸 action 类型；共享物理输入或位位置不改变各自领域类型的消费语义。

## 验收条件

- `UIAction` 至少能表达 `Left`、`Right`、`Up`、`Down`、`Confirm`、`Back` 六种 UI 交互语义。
- `UIAction` 用于焦点移动、当前选项确认和页面返回行为，不进入规则核心输入路径。
- 同一物理输入位在游戏上下文产生 `GameAction`、在 UI 上下文产生 `UIAction`，两者互不合并。

## Test Basis

- [Confirmed] PRD §5.2：每位玩家的输入映射包含确认和返回/暂停等界面交互，与左右、软降、硬降、旋转等对局动作并列。
- [Confirmed] TDD §3：确认、返回、暂停等 client 交互由客户端输入上下文处理，不进入规则核心 tick 动作。
- [Confirmed] presentation.md：焦点顺序、确认/返回在所有菜单中的键盘与手柄操作保持一致。
- [Confirmed] [统一游戏动作与 Tick 输入 Spec](game-action-input.md)：`Confirm`、`Back`、`Pause` 等界面交互语义属于 client 表现领域，不进入规则核心动作集合，可与对局动作共享物理按键绑定。
- [Confirmed] 当前审核结论：新增独立 `UIAction` 类型，与 `GameAction` 分属两个领域；至少包含 `Left`/`Right`/`Up`/`Down`/`Confirm`/`Back`；完整 bit 映射与 `Pause` 归属留待后续设计。
