# 游戏基础设施运行架构

**状态：** v1
**主分类：** System  
**相关模块：** `core`、`client`、`net`、Bevy 应用运行时  
**关联文档：** [PRD](../../PRD.md)、[TDD](../../TDD.md)、[应用状态机 Spec](../component/application-state-machine.md)、[应用状态机协作 Contract](../contract/application-state-machine.md)

## 目标与范围

建立 Bevy 客户端运行骨架，提供顶层应用状态、启动准备、60Hz 固定规则调度、输入、数据、本地化、设置和既有 crate 边界。

## 架构职责

| 模块 | 职责 | 依赖方向 | 对外提供 |
| --- | --- | --- | --- |
| `core` | 保存纯规则领域类型、配置模型、统一游戏动作与后续确定性验证能力 | 无 Bevy、文件系统、窗口或网络依赖 | 规则数据模型、游戏动作、后续 `MatchState` |
| `client::app_state` | 保存顶层应用状态与合法迁移语义 | 依赖 Bevy `States` | `AppState`、状态迁移入口、当前状态 |
| `client` | 组织 Bevy 应用、输入、资源加载、本地化、设置、表现和本地功能 | `client → core`；需要时接入 `net` | Bevy App、运行时资源和调度 |
| `net` | 后续承载局域网会话、输入同步、状态校验和断线处理 | `net → core` | R2 网络会话能力 |
| Bevy | 提供窗口、主循环、调度、States、设备输入、资源、渲染、UI 和音频 | 由 `client` 组合 | 应用运行时能力 |

## 主流程

1. `main` 创建 Bevy `App`，注册引擎插件和项目根插件。
2. 应用状态机初始化为 `Boot`。
3. 设置与本地化分别完成启动解析；成功结果或安全默认值都标记为 `Resolved`。
4. 两项启动任务均为 `Resolved` 后，请求 `Boot → MainMenu`。
5. 后续 client 侧流程组件按应用状态机 Contract 请求顶层状态迁移。
6. 进入 `Match` 后，普通 `Update` 负责设备输入采集和表现更新。
7. Bevy 固定调度以 60Hz 消费量化后的 tick 输入并驱动规则状态。
8. 表现系统读取最近规则状态提供的可观察数据更新画面、UI 和音频。

## 启动准备

`Boot` 是首次进入可交互状态前的同步屏障，只等待以下运行时资源：

```text
UserSettings
Localization
```

启动状态使用明确的完成标记：

```rust
struct BootstrapStatus {
    settings: BootstrapTaskState,
    localization: BootstrapTaskState,
}

enum BootstrapTaskState {
    Pending,
    Resolved,
}
```

加载成功与使用安全默认值恢复都进入 `Resolved`。加载模块负责解析、fallback 和诊断；启动协调逻辑只判断：

```text
settings == Resolved
&& localization == Resolved
```

条件成立后提出 `Boot → MainMenu` 状态迁移请求。

因此，`MainMenu` 及其后续状态可以假定 `UserSettings` 与 `Localization` 已经可用。

## 状态与分支

以下表描述基础设施 System 的主流程；有效状态边及状态机语义以应用状态机 Spec 为准。

| 当前状态 | 触发 | 条件 | 下一状态 |
| --- | --- | --- | --- |
| `Boot` | 启动准备完成 | 设置与本地化均为 `Resolved` | `MainMenu` |
| `MainMenu` | 开始游戏 | 用户确认 | `ModeSelect` |
| `ModeSelect` | 模式确认 | 模式可进入 | `CharacterSelect` |
| `CharacterSelect` | 开局确认 | 开局数据已准备 | `Match` |
| `Match` | 本地暂停 | 当前模式允许暂停 | `Paused` |
| `Paused` | 继续 | 用户确认 | `Match` |
| `Match` | 比赛结束 | 已产生比赛结果 | `Result` |
| `Result` | 返回主菜单 | 用户确认 | `MainMenu` |

返回、退出、再赛等 UI 交互在表现领域设计时增加对应有效状态边与测试。

## 架构约束

- `main.rs` 保持薄入口，由项目根插件组合基础设施能力。
- `client::app_state` 是顶层应用阶段的单一状态所有者。
- 其它 client 侧组件通过状态迁移入口提出请求，不直接维护平行的顶层阶段字段。
- `core` 可在无窗口、无渲染、无网络、无文件系统环境中独立运行。
- `client` 负责设备输入、文件/资源路径、Bevy ECS 和本机设置。
- `net` 通过 `core` 的规则状态和游戏输入语义接入后续同步。
- 规则推进使用 60Hz 固定调度；普通更新与渲染帧率不直接决定规则 tick。
- 用户设置、渲染实体、音频、动画和网络 socket 不进入 `core` 的可回滚规则状态。
- 顶层 `AppState` 表达客户端宏观运行阶段；对局内部规则状态与网络连接生命周期使用各自的状态模型。

## 验收条件

- Linux 与 Windows 目标可以构建并启动 `client`。
- 应用初始化后进入 `Boot`；设置与本地化均 `Resolved` 后进入 `MainMenu`。
- `MainMenu` 及后续状态中 `UserSettings` 与 `Localization` 已可用。
- 顶层阶段由单一应用状态机管理，并可完成基础主路径状态切换。
- 对局规则入口挂接到 60Hz 固定调度；普通 `Update` 频率不会改变 fixed tick 计数。
- `client` 可以使用 `core`，`net` 可以使用 `core`，`core` 不获得 Bevy、窗口、文件系统或网络依赖。

## Test Basis

- [Confirmed] Issue #11：要求 Bevy 0.19.x 运行时、明确应用状态、60Hz 固定更新、输入/资源/设置能力和 `core` / `client` / `net` 边界。
- [Confirmed] TDD §2：定义 workspace 职责和 `client → core`、`net → core` 依赖方向。
- [Confirmed] TDD §3–§5：定义固定 tick、Bevy States、配置、本地化和安全默认值。
- [Confirmed] 当前审核结论：`Boot` 作为设置与本地化的启动同步屏障；`main.rs` 保持薄入口；应用状态机为独立 Component。
