# 游戏基础设施运行架构

**相关模块：** `game_core`、`client`、`net`、Bevy 应用运行时
**关联文档：** [PRD](../../PRD.md)、[TDD](../../TDD.md)、[应用状态机](../design/application-state-machine.md)、[固定频率规则调度](../design/fixed-tick-simulation.md)

## 目标与范围

建立 Bevy 客户端运行骨架，提供顶层应用状态、启动准备、60Hz 固定规则调度、输入、数据、本地化、设置和既有 crate 边界。

## 模块职责

| 模块 | 职责 | 依赖方向 | 对外提供 |
| --- | --- | --- | --- |
| `game_core` | 保存纯规则领域类型、配置模型、统一游戏动作与确定性验证能力 | 无 Bevy、文件系统、窗口或网络依赖 | 规则数据模型、游戏动作、`MatchState` |
| `client::app_state` | 保存顶层应用状态与合法迁移语义 | 依赖 Bevy `States` | `AppState`、状态迁移入口、当前状态 |
| `client` | 组织 Bevy 应用、输入、资源加载、本地化、设置、表现和本地功能 | `client → game_core`；需要时接入 `net` | Bevy App、运行时资源和调度 |
| `net` | 承载局域网会话、输入同步、状态校验和断线处理 | `net → game_core` | R2 网络会话能力 |
| Bevy | 提供窗口、主循环、调度、States、设备输入、资源、渲染、UI 和音频 | 由 `client` 组合 | 应用运行时能力 |

## 主流程

1. `main` 创建 Bevy `App`，注册引擎插件和项目根插件。
2. 应用状态机初始化为 `Boot`。
3. 设置与本地化分别完成启动解析；两者都是启动后即可用的数据类别，加载成败都标记为 `Resolved`。
4. 两项启动任务均为 `Resolved` 后，请求 `Boot → MainMenu`。
5. 后续 client 侧流程组件按[应用状态机](../design/application-state-machine.md)请求顶层状态迁移。
6. 进入 `Match` 后，普通 `Update` 负责设备输入采集和表现更新。
7. Bevy 固定调度以 60Hz 消费量化后的 tick 输入并驱动规则状态。
8. 表现系统读取最近规则状态提供的可观察数据更新画面、UI 和音频。
9. 进入 `Paused` 后对局 simulation 停止；恢复 `Match` 后从暂停前已有的规则状态继续推进。

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

屏障只等待这两项，因为它们缺失后都仍有确定行为：用户设置回到首次运行初值，文本目录按替补链呈现为 key。加载成功与加载失败都进入 `Resolved`。规则数据不参与屏障，它按[阻断级](../design/runtime-data-loading.md#失败分级)在 Match 入口处置。启动协调逻辑只判断：

```text
settings == Resolved
&& localization == Resolved
```

启动资源经由异步资源加载路径读取时，`Pending` 表示该项尚未得到结果。为保证屏障不会因资源始终不返回而永久停留在 `Boot`，启动准备设置 `5s` 超时：超时的启动任务按加载失败处理并进入 `Resolved`，同时保留超时诊断。因此 `Boot` 在任何加载结果下都会在有限时间内释放。

条件成立后提出 `Boot → MainMenu` 状态迁移请求。`MainMenu` 及其后续状态可以假定 `UserSettings` 与 `Localization` 已经可用。

## 启动验收

生产客户端与自动化 startup smoke 复用同一个项目根插件：

```text
Production client
DefaultPlugins
+ 项目根插件

Automated startup smoke
minimal Bevy runtime
+ 项目根插件
```

启动验收分为三个面：

**Production build** — 在 Linux 构建并链接真实 production client，覆盖真实客户端依赖，包括生产 Bevy plugin 配置（`DefaultPlugins` + 项目根插件）。

**Automated startup smoke** — 在 Linux 运行复用项目根插件、无真实窗口依赖的最小 Bevy App smoke，验证以下路径：

```text
创建 Bevy App
→ 注册与生产客户端相同的项目根插件
→ AppState 初始化为 Boot
→ UserSettings 完成 bootstrap resolution
→ Localization 完成 bootstrap resolution
→ 两项均 Resolved
→ Boot → MainMenu
→ 测试正常结束
```

**真实窗口启动** — 前两项分别在无窗口 runtime 中运行、以及只编译链接不运行，都不覆盖 `DefaultPlugins` 的运行时初始化、窗口后端和平台动态依赖，因此这是独立的验收面。

该项以**有界启动运行**验收：启动真实 `psi` 二进制，在到达 `MainMenu` 后的一个确定帧自动退出，以进程退出码、到达 `MainMenu` 的可观察标记和运行期无资源缺失诊断三者共同作为结论。缺少后两项时，停留在 `Boot` 的构建与全部数据加载失败的运行都会得到相同的成功退出码（资产根解析见[版本化运行数据加载：资产根](../design/runtime-data-loading.md#资产根)）。有界退出由 Bevy 的 `bevy_ci_testing` 能力提供，经 `client` 的 `ci_testing` feature 开启，退出帧由 `CI_TESTING_CONFIG` 指向的配置决定；该 feature 不进入发布构建。

执行环境需要虚拟显示与软件渲染后端（`xvfb` 与 Mesa 的软件 Vulkan 驱动），单次执行的安装与运行开销明显高于其余测试。因此该项只在手动触发的发布工作流中运行，不进入拉取请求门禁，与 [TDD §7.2](../../TDD.md) 的额度取舍一致。

## 边界

- 本文不定义有效状态边与状态机语义（见[应用状态机](../design/application-state-machine.md)）。
- 本文不定义 fixed schedule 的阶段划分与运行条件（见[固定频率规则调度](../design/fixed-tick-simulation.md)）。
- 本文不定义资源解析、失败分级与诊断的具体语义（见[版本化运行数据加载](../design/runtime-data-loading.md)）。
- `main.rs` 保持薄入口，由项目根插件组合基础设施能力。
- `client::app_state` 是顶层应用阶段的单一状态所有者；其它 client 侧组件通过状态迁移入口提出请求。
- `game_core` 可在无窗口、无渲染、无网络、无文件系统环境中独立运行；`client` 负责设备输入、文件/资源路径、Bevy ECS 和本机设置；`net` 通过 `game_core` 的规则状态和游戏输入语义接入同步。
- 用户设置、渲染实体、音频、动画和网络 socket 不进入 `game_core` 的可回滚规则状态。
- 顶层 `AppState` 表达客户端宏观运行阶段；对局内部规则状态与网络连接生命周期使用各自的状态模型。

## Test Basis

- [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)：要求 Bevy 0.19.x 运行时、明确应用状态、60Hz 固定更新、输入/资源/设置能力和 `game_core` / `client` / `net` 边界。
- [TDD §2](../../TDD.md)：定义 workspace 职责和 `client → game_core`、`net → game_core` 依赖方向。
- [TDD §3–§5](../../TDD.md)：定义固定 tick、Bevy States、配置、本地化和安全默认值。
- [TDD §7.2](../../TDD.md)：CI 只在目标为 `main` 的 pull request 上运行，发布工作流手动触发。
