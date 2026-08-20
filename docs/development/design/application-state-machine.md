# 应用状态机

**相关模块：** `client::app_state`、状态迁移请求方
**关联文档：** [游戏基础设施运行架构](../system/game-infrastructure-architecture.md)、[UI 交互动作](ui-action-input.md)、[页面导航与焦点](page-navigation.md)、[固定频率规则调度](fixed-tick-simulation.md)、[TDD §4](../../TDD.md)

## 目标

提供客户端唯一的顶层运行阶段状态，定义有效状态转移语义，并定义 client 侧组件如何提出迁移请求、状态机如何校验、去重、仲裁并提交迁移。

## 数据模型

```rust
enum AppState {
    Boot,
    MainMenu,
    ModeSelect,
    CharacterSelect,
    Settings,
    Match,
    Paused,
    Result,
}
```

`AppState` 直接使用 Bevy `States` 管理；迁移最终写入 `NextState<AppState>`。任一时刻只有一个当前 `AppState`，它是客户端顶层运行阶段的单一真相源。

```rust
struct AppTransitionRequest {
    target: AppState,
    cause: AppTransitionCause,
}
```

`cause` 表达迁移原因，用于诊断、冲突仲裁和消费者区分同一状态边上的不同语义，随已确认状态边扩展：

```text
BootstrapReady
StartGame
ModeConfirmed
CharacterConfirmed
BackRequested
SettingsOpened
SettingsClosed
PauseRequested
ResumeRequested
RestartRequested
MatchCompleted
RematchRequested
MatchAbandoned
ReturnToMainMenu
```

```rust
struct CommittedTransition {
    from: AppState,
    to: AppState,
    cause: AppTransitionCause,
}
```

`CommittedTransition` 由 Arbiter 在写入 `NextState<AppState>` 的同时记录，保存本次提交的迁移。同一状态边可以承载不同语义，消费者据此区分：`Paused → Match` 在 `ResumeRequested` 下继续既有对局，在 `RestartRequested` 下重建对局。消费者只读该记录，不据此推断尚未提交的迁移。

### 设置返回上下文

```rust
struct SettingsOrigin(AppState);
```

`Settings` 从 `MainMenu` 与 `Paused` 两处进入，返回目标不由状态边唯一决定。请求方在提出 `SettingsOpened` 前把来源状态写入 `SettingsOrigin`；`SettingsClosed` 的目标取自该值。来源状态只有 `MainMenu` 与 `Paused` 两种取值。

## 有效状态转移

| 当前状态 | 允许目标 | 对应 cause |
| --- | --- | --- |
| `Boot` | `MainMenu` | `BootstrapReady` |
| `MainMenu` | `ModeSelect`、`Settings` | `StartGame`、`SettingsOpened` |
| `ModeSelect` | `CharacterSelect`、`MainMenu` | `ModeConfirmed`、`BackRequested` |
| `CharacterSelect` | `Match`、`ModeSelect` | `CharacterConfirmed`、`BackRequested` |
| `Settings` | `MainMenu`、`Paused` | `SettingsClosed` |
| `Match` | `Paused`、`Result` | `PauseRequested`、`MatchCompleted` |
| `Paused` | `Match`、`Settings`、`MainMenu` | `ResumeRequested`、`RestartRequested`、`SettingsOpened`、`MatchAbandoned` |
| `Result` | `Match`、`MainMenu` | `RematchRequested`、`ReturnToMainMenu` |

增加有效状态边时同步增加状态迁移测试。

## 行为

### 初始化

应用状态机注册后，初始状态为 `Boot`。

### 查询

运行时组件可以读取当前唯一 `AppState`。

### 状态进入与退出

状态实际提交后，Bevy 的 `OnExit` / `OnEnter`、state-scoped systems 或等价状态调度负责激活对应消费者。

## 协作

| 参与者 | 职责 |
| --- | --- |
| 状态迁移请求方 | 在自己掌握的迁移前置条件成立后提出目标 `AppState` |
| `AppTransitionArbiter` | 校验状态边、合并重复请求、处理冲突，并作为 `NextState<AppState>` 的唯一写入者 |
| 状态消费者 | 在状态实际进入或退出后执行该阶段的运行时行为 |

迁移请求由掌握对应业务前置条件的 client 侧组件提出：

- `BootstrapReady`：启动协调 system 在 `BootstrapStatus` 的设置与本地化均为 `Resolved` 时提出。启动协调只消费 `Resolved` 状态，加载、失败分级和诊断由对应模块负责。
- `PauseRequested`：由 `client::input` 在 `AppState == Match` 时识别固定绑定的暂停输入（手柄 Start 按键、键盘 `Escape`，两者不区分玩家）后直接提出。其它状态下同一输入不提出任何迁移请求，也不产生 `UIAction`；该次按下沿就地丢弃，不滞留到下一次进入 `Match`。
- `CharacterConfirmed`：由角色选择流程在规则数据可用并完成开局规格冻结后提出。规则数据为 `Failed` 时不提出该请求，理由见[版本化运行数据加载 · 失败分级](runtime-data-loading.md#失败分级)。
- `RematchRequested`：由赛果页面在完成同配置新开局规格冻结后提出，冻结要求与 `CharacterConfirmed` 相同。
- `StartGame`、`BackRequested`、`SettingsOpened`、`SettingsClosed`、`ResumeRequested`、`RestartRequested`、`MatchAbandoned`、`ReturnToMainMenu`：由对应页面的导航组件在玩家确认该页面动作后提出（见[页面导航与焦点](page-navigation.md)）。

### 协作时序

1. 请求方确认自己负责的迁移前置条件已经成立。
2. 请求方先准备目标状态进入后需要读取的数据。
3. 请求方向 `AppTransitionArbiter` 提交 `AppTransitionRequest`。
4. Arbiter 根据当前 `AppState` 校验状态边，并处理同一提交周期中的其它请求。
5. Arbiter 将唯一有效目标写入 `NextState<AppState>`。
6. Bevy 提交状态变化。
7. 状态消费者在对应 `OnExit` / `OnEnter` 或等价调度中响应已经完成的迁移。

### 请求处理

**同状态请求。** 目标等于当前状态时直接结束请求，保持当前状态，不写入 `NextState<AppState>`，不触发额外 `OnExit` / `OnEnter`，不产生非法状态边诊断。

**合法性。** 其余请求按有效状态转移表校验。非法状态边不写入 `NextState`，当前状态保持不变，并产生开发诊断。

**重复目标。** 同一提交周期内多个请求指向同一目标时合并为一次迁移。

```text
Match → Result
Match → Result
= 一次 Match → Result
```

**冲突目标。** 同一提交周期出现不同目标时使用显式优先级：

```text
MatchCompleted > PauseRequested
```

即比赛已经结束时直接进入 `Result`，不再进入 `Paused`。没有已定义优先级的冲突请求全部拒绝，本周期保持当前状态并记录冲突诊断。新增可能冲突的状态边时同时补充仲裁规则。

### 请求方约束

- 请求方掌握迁移所需的业务前置条件；Arbiter 不重新实现这些业务判断。
- 请求方在提出迁移前准备目标状态必需的数据。
- 请求方不直接写 `NextState<AppState>`，也不维护平行的顶层阶段字段。
- 请求被拒绝时，请求方不得假定目标状态已经生效。

## 边界

- 本文不定义资源加载、设置修改、比赛结果生成与规则推进。
- 本文不定义页面内部的焦点顺序、控件导航与页面实体清理（见[页面导航与焦点](page-navigation.md)）。
- 本文不定义各 cause 下对局实例如何创建、保留或重建（见[固定频率规则调度：对局实例生命周期](fixed-tick-simulation.md#对局实例生命周期)）。
- 本文不定义对局内部规则状态（见[固定频率规则调度](fixed-tick-simulation.md)），它独立于 `AppState`。
- 本文不定义网络连接生命周期（见[游戏基础设施运行架构](../system/game-infrastructure-architecture.md)）。网络连接使用独立状态类型，不向 `AppState` 平铺连接状态。
- 本文不定义页面、对局、联机等功能的内部模块结构；只要求其状态迁移出口遵守统一请求协议。

## Test Basis

- [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)：要求启动、菜单、模式选择、角色选择、对局、暂停和赛果应用状态，并能通过 Bevy 调度切换。
- [TDD §4](../../TDD.md)：使用 Bevy `States` 管理顶层应用阶段。
- [TDD §5](../../TDD.md)：设置和文本解析失败时使用安全默认值并提供诊断。
- [Issue #13](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/13)：要求设置页、暂停、重开、退出与再来一局可由玩家操作完成。
- [表现与 UI 设计 §5](../../presentation.md)：定义主菜单、模式选择、角色选择、设置、暂停与赛果各自的主操作。
