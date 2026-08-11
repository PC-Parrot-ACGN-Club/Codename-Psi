# 应用状态机协作 Contract

**状态：** v1  
**主分类：** Component Integration  
**相关模块：** `client::app_state`、状态迁移请求方  
**关联文档：** [应用状态机 Spec](../component/application-state-machine.md)、[游戏基础设施运行架构](../system/game-infrastructure-architecture.md)

## 目的

定义 client 侧组件如何提出顶层状态迁移请求，以及状态机如何校验、去重、仲裁并提交迁移。

## 参与者与职责

| 参与者 | 职责 |
| --- | --- |
| 状态迁移请求方 | 在自己掌握的迁移前置条件成立后提出目标 `AppState` |
| `AppTransitionArbiter` | 校验状态边、合并重复请求、处理冲突，并作为 `NextState<AppState>` 的唯一写入者 |
| 状态消费者 | 在状态实际进入或退出后执行该阶段的运行时行为 |

Issue #11 当前明确的请求方是启动协调 system：当 `BootstrapStatus` 中设置与本地化均为 `Resolved` 时，请求 `Boot → MainMenu`。

后续任务增加状态边时，同时定义掌握该迁移前置条件的 client 侧请求方：
- 游戏玩法实现：对局结束等由 client 侧对局运行组件提出。
- 游戏表现领域：菜单、返回、暂停、再赛等由页面导航相关组件提出。
- 联机功能：需要影响 `AppState` 的联机流程由联机 client 集成组件提出。

## 数据契约

```rust
struct AppTransitionRequest {
    target: AppState,
    cause: AppTransitionCause,
}
```

`cause` 表达迁移原因，用于诊断和冲突仲裁。具体枚举随已确认状态边扩展。

当前或已确认会使用的原因包括：

```text
BootstrapReady
StartGame
ModeConfirmed
CharacterConfirmed
PauseRequested
ResumeRequested
MatchCompleted
ReturnToMainMenu
```

## 协作时序

1. 请求方确认自己负责的迁移前置条件已经成立。
2. 请求方先准备目标状态进入后需要读取的数据。
3. 请求方向 `AppTransitionArbiter` 提交 `AppTransitionRequest`。
4. Arbiter 根据当前 `AppState` 校验状态边，并处理同一提交周期中的其它请求。
5. Arbiter 将唯一有效目标写入 `NextState<AppState>`。
6. Bevy 提交状态变化。
7. 状态消费者在对应 `OnExit` / `OnEnter` 或等价调度中响应已经完成的迁移。

## 请求处理规则

### 合法性

- 目标等于当前状态时直接结束请求，不写入 NextState<AppState>，不产生非法状态边诊断。其余请求再进行有效状态边校验。
- 请求目标必须是当前 `AppState` 的有效状态边。
- 非法状态边不写入 `NextState`，当前状态保持不变，并产生开发诊断。

### 重复目标

同一提交周期内多个请求指向同一目标时合并为一次迁移。

```text
Match → Result
Match → Result
= 一次 Match → Result
```

### 冲突目标

同一提交周期出现不同目标时使用显式优先级。

当前已定义：

```text
MatchCompleted > PauseRequested
```

即比赛已经结束时直接进入 `Result`，不再进入 `Paused`。

没有已定义优先级的冲突请求全部拒绝，本周期保持当前状态并记录冲突诊断。新增可能冲突的状态边时必须同时补充仲裁规则。

## 请求方约束

- 请求方掌握迁移所需的业务前置条件；Arbiter 不重新实现这些业务判断。
- 请求方在提出迁移前准备目标状态必需的数据。
- 请求方不直接写 `NextState<AppState>`。
- 请求被拒绝时，请求方不得假定目标状态已经生效。
- 页面、对局、联机等后续功能自行决定内部模块结构；本 Contract 只要求其状态迁移出口遵守统一请求协议。

## 启动失败处理

启动协调只消费 Resolved 状态；具体加载、fallback 和诊断由对应模块负责。

## 验收条件

- 启动条件满足后，只通过统一请求协议完成 `Boot → MainMenu`。
- 请求方无法绕过 Arbiter 直接提交顶层状态。
- 相同目标请求只产生一次状态迁移。
- `MatchCompleted` 与 `PauseRequested` 同周期出现时进入 `Result`。
- 未定义优先级的不同目标冲突不会产生状态变化，并具有可观察诊断。
- 非法状态边不会改变当前 `AppState`。

## Test Basis

- [Confirmed] Issue #11：要求明确应用状态并通过 Bevy 调度切换。
- [Confirmed] TDD §4：使用 Bevy `States` 管理顶层应用阶段。
- [Confirmed] TDD §5：设置和文本解析失败时使用安全默认值并提供诊断。
- [Confirmed] 当前审核结论：状态迁移请求经统一 Arbiter；`MatchCompleted > PauseRequested`;迁移请求方由掌握对应业务前置条件的组件承担。
