# 应用状态机 Spec

**状态：** Confirmed 
**主分类：** Component  
**相关模块：** `client::app_state`  
**关联文档：** [游戏基础设施运行架构](../system/game-infrastructure-architecture.md)、[应用状态机协作 Contract](../contract/application-state-machine.md)、[TDD](../../TDD.md)、[PRD](../../PRD.md)

## 目标

提供客户端唯一的顶层运行阶段状态，并定义有效状态转移语义。

## 数据模型

```rust
enum AppState {
    Boot,
    MainMenu,
    ModeSelect,
    CharacterSelect,
    Match,
    Paused,
    Result,
}
```

`AppState` 直接使用 Bevy `States` 管理；迁移通过项目状态迁移入口最终写入 `NextState<AppState>`。

## 有效状态转移

Issue #11 固定以下基础状态边：

| 当前状态 | 允许目标 |
| --- | --- |
| `Boot` | `MainMenu` |
| `MainMenu` | `ModeSelect` |
| `ModeSelect` | `CharacterSelect` |
| `CharacterSelect` | `Match` |
| `Match` | `Paused`、`Result` |
| `Paused` | `Match` |
| `Result` | `MainMenu` |

返回、退出、再赛等由 UI 交互产生的状态边在表现领域任务中增加；每次增加有效状态边时同步增加状态迁移测试。

## 行为

### 初始化

应用状态机注册后，初始状态为 `Boot`。

### 查询

运行时组件可以读取当前唯一 `AppState`。

### 请求迁移

状态迁移请求由协作 Contract 定义的统一入口接收。

- 有效状态边进入待提交状态。
- 无效状态边被拒绝，当前状态保持不变。

### 同状态请求

当目标状态等于当前状态时：

```text
current == target
→ 保持当前状态
```

同状态请求直接结束，不写入 NextState<AppState>，不触发额外 `OnExit` / `OnEnter`。

### 状态进入与退出

状态实际提交后，Bevy 的 `OnExit` / `OnEnter`、state-scoped systems 或等价状态调度负责激活对应消费者。

## 不变量

- 任一时刻只有一个当前顶层 `AppState`。
- `AppState` 是客户端顶层运行阶段的单一真相源。
- 无效状态边不会改变当前状态。
- 同状态请求不产生新的状态生命周期。
- 状态机不加载资源、不修改设置、不生成比赛结果、不推进规则。
- 目标状态需要的业务数据由迁移请求方在请求前准备。
- 对局内部规则状态独立于 `AppState`。

## 网络状态边界

网络连接生命周期与 `AppState` 的关系由[游戏基础设施运行架构](../system/game-infrastructure-architecture.md)的架构约束定义；本 Spec 的有效状态边表不需要为网络连接状态预留空间，联机任务根据页面和连接流程决定具体网络状态集合。

## 验收条件

- 初始化后当前状态为 `Boot`。
- 基础有效状态表中的每条边均可完成迁移。
- 表外状态边请求不会改变当前状态。
- 同状态请求不触发额外退出/进入生命周期。
- 状态变化能够触发对应 Bevy 状态调度。
- 增加表现领域状态边时，可以只扩展有效边与测试，不修改状态机总体结构。
- 网络连接生命周期可以独立演进，不需要向 `AppState` 平铺连接状态。

## Test Basis

- [Confirmed] Issue #11：要求启动、菜单、模式选择、角色选择、对局、暂停和赛果应用状态，并能通过 Bevy 调度切换。
- [Confirmed] TDD §4：使用 Bevy `States` 管理应用状态。
- [Confirmed] 当前审核结论：复用 Bevy `States` / `NextState`；UI 交互状态边由表现领域扩展；同状态请求为 no-op；网络连接生命周期使用独立状态类型。
