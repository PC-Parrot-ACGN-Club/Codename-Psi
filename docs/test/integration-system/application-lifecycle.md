# 测试用例设计：应用生命周期

**关联设计：** [应用状态机](../../development/design/application-state-machine.md)、[游戏基础设施运行架构](../../development/system/game-infrastructure-architecture.md)、[版本化运行数据加载](../../development/design/runtime-data-loading.md)、[本地化运行时](../../development/design/localization-runtime.md)

**关联实现：** `../../../crates/client`

## 需求理解摘要

**功能：** 验证 Bevy 应用状态的初始化、提交、生命周期、请求仲裁、启动屏障和基础系统主路径。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** 状态机 Component Integration 与已装配客户端的基础状态 System 路径。
**Test Basis：**

- [Confirmed] [应用状态机](../../development/design/application-state-machine.md)：初始化、请求处理、生命周期、仲裁和主状态边。
- [Confirmed] [游戏基础设施运行架构](../../development/system/game-infrastructure-architecture.md)：启动屏障与基础运行主流程。
- [Confirmed] [版本化运行数据加载](../../development/design/runtime-data-loading.md)与[本地化运行时](../../development/design/localization-runtime.md)：启动失败分级和降级结果。

**设计基线：** 以最小 Bevy App 验证状态协作，只将必须由完整客户端证明的状态主路径提升到 System。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不重复纯状态表判断（见 [应用状态表](../component/application-state-table.md)），也不定义 fixed simulation 的暂停效果（见 [输入与固定调度](input-and-fixed-tick.md)）或真实窗口启动（见 [构建与启动](build-and-startup.md)）。

## 测试点清单

- 状态初始化、同状态 no-op、合法迁移生命周期、请求去重与仲裁（TC-001～TC-005）。
- 启动任务组合、降级失败与超时释放屏障（TC-006～TC-007、TC-009；Concern: Smoke）。
- Boot 到 Result 再回主菜单的完整基础状态路径（TC-008；System、Concern: Smoke）。
- 阻断级规则数据不可用时拒绝 Match 请求（TC-010；Concern: Content Validation）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 状态迁移 | 初始化、同状态、合法迁移、仲裁和完整主路径 | TC-001～TC-005、TC-008 |
| 判定表 | 启动任务 Pending/Resolved 组合 | TC-006 |
| 场景 / 协作路径 | 加载失败、超时和开局阻断 | TC-007、TC-009～TC-010 |
| 错误猜测 | 重复迁移请求与永不返回的启动资源 | TC-004、TC-009 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 注册应用状态机后初始状态为 Boot | P0 | Component Integration | — | Client | 最小 Bevy `App` 注册状态能力 | 完成初始化/首个必要 schedule | 无 | 当前唯一 `AppState=Boot` | [Confirmed] [应用状态机：初始化](../../development/design/application-state-machine.md#初始化) |
| TC-002 | 同状态请求为 no-op 且不触发生命周期 | P1 | Component Integration | — | Client | 最小 Bevy App 为 OnExit/OnEnter 安装计数器 | 对七种状态分别请求自身并运行状态提交周期 | Boot→Boot … Result→Result | 当前状态保持；进入/退出计数不增；无非法边诊断 | [Confirmed] [应用状态机：请求处理](../../development/design/application-state-machine.md#请求处理) |
| TC-003 | 合法迁移实际提交后各触发一次退出与进入生命周期 | P1 | Component Integration | — | Client | 最小 Bevy App 为源/目标状态注册生命周期观察 | 提交一条合法边并运行完整状态提交周期 | MainMenu→ModeSelect | 当前唯一状态变为 ModeSelect；OnExit(MainMenu)=1，OnEnter(ModeSelect)=1；对应运行阶段在进入后激活 | [Confirmed] [应用状态机：协作时序](../../development/design/application-state-machine.md#协作时序) |
| TC-004 | 同周期重复目标请求合并为一次迁移 | P1 | Component Integration | — | Client | 当前 Match，生命周期计数器已注册 | 同周期提交两份 Result 请求 | 两个 `MatchCompleted` | 仅写入/提交一次 Match→Result；生命周期各触发一次 | [Confirmed] [应用状态机：请求处理](../../development/design/application-state-machine.md#请求处理) |
| TC-005 | MatchCompleted 与 PauseRequested 同周期时 Result 获胜 | P1 | Component Integration | — | Client | 当前 Match | 同周期以两种顺序提交两个请求 | Result/MatchCompleted；Paused/PauseRequested | 两种顺序最终均进入 Result，未进入 Paused | [Confirmed] [应用状态机：请求处理](../../development/design/application-state-machine.md#请求处理) |
| TC-006 | Boot 仅在设置与本地化均 Resolved 时请求 MainMenu | P0 | Component Integration | Smoke | Client | 当前 Boot，可设置 `BootstrapStatus` | 覆盖四种 Pending/Resolved 组合并运行协调 system | PP、PR、RP、RR | 前三组保持 Boot 且无迁移请求；RR 只产生一次 Boot→MainMenu 请求 | [Confirmed] [游戏基础设施运行架构：启动准备](../../development/system/game-infrastructure-architecture.md#启动准备) |
| TC-007 | 设置与本地化加载失败仍解除启动屏障 | P0 | Component Integration | Smoke；Content Validation | Configuration；Client | 最小启动 app，设置与 catalog 均使用失败 fixture | 完成两类加载与启动协调 | malformed settings；missing/unsupported locale catalog | 两项均保留原始诊断并标记 Resolved；应用进入 MainMenu；设置查询得到完整默认设置；两个 catalog 均不可用时文本查询返回 key 本身 | [Confirmed] [游戏基础设施运行架构：启动准备](../../development/system/game-infrastructure-architecture.md#启动准备)；[版本化运行数据加载：失败分级](../../development/design/runtime-data-loading.md#失败分级)；[本地化运行时：查询文本](../../development/design/localization-runtime.md#查询文本) |
| TC-008 | 完整基础状态主路径保持单一顶层状态与对应运行阶段 | P0 | System | Smoke | Client；Match Flow | 已装配客户端，启动资源可 resolved，已确认的状态迁移请求均可触发 | 完成启动并依次触发开始、模式确认、角色确认、暂停、继续、比赛结束、返回主菜单 | Boot→MainMenu→ModeSelect→CharacterSelect→Match→Paused→Match→Result→MainMenu | 每步仅有一个当前 AppState；对应状态的运行阶段在进入后激活；本用例不规定 ModeSelect、CharacterSelect、Result 等状态的业务数据结构或具体内容 | [Confirmed] [应用状态机：有效状态转移](../../development/design/application-state-machine.md#有效状态转移) |
| TC-009 | 启动资源超时后仍进入 `Resolved` 并释放屏障 | P1 | Component Integration | Smoke | Client；Configuration | 最小 Bevy App，启动资源加载不返回结果 | 推进应用直到超过启动超时 | 加载超时 `5s` | 两项启动任务均进入 `Resolved`；设置得到完整默认设置，文本查询返回 key 本身；保留超时诊断；`Boot → MainMenu` 完成，应用不停留在 `Boot` | [Confirmed] [游戏基础设施运行架构：启动准备](../../development/system/game-infrastructure-architecture.md#启动准备) |
| TC-010 | 规则数据为 Failed 时不提出进入 Match 的迁移请求 | P0 | Component Integration | Content Validation | Configuration；Client | 最小 app 已到达可提出开局请求的流程点，规则数据 resolution 可注入 | 分别注入 `Loaded` 与 `Failed` 的规则数据并运行开局请求 system | `Failed(UnsupportedSchema)`；等价有效规则数据 | `Failed` 时不产生 `MatchStartRequested`、`AppState` 不变且失败原因可观察；`Loaded` 时产生一次请求 | [Confirmed] [应用状态机：协作](../../development/design/application-state-machine.md#协作)；[版本化运行数据加载：失败分级](../../development/design/runtime-data-loading.md#失败分级) |

## 风险查漏

单一顶层状态、生命周期次数、重复请求、优先级、启动屏障、降级和开局阻断均有直接用例。

