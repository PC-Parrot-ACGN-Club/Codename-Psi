# 测试用例设计：对局实例生命周期

**关联设计：** [固定频率规则调度](../../development/design/fixed-tick-simulation.md)、[应用状态机](../../development/design/application-state-machine.md)、[页面导航与焦点](../../development/design/page-navigation.md)

**关联实现：** `../../../crates/client`

## 需求理解摘要

**功能：** 验证进入 `Match` 时按迁移 cause 新建、重建或保留规则实例，退出时释放资源，以及本地 BO3 的完整可玩路径。
**测试性质：** 新功能
**本轮范围：** client 侧对局实例的创建、保留、重建与释放，以及单人与本地双人的端到端 BO3。
**Test Basis：**

- [Confirmed] [固定频率规则调度：对局实例生命周期](../../development/design/fixed-tick-simulation.md#对局实例生命周期)：四个 cause 的实例处置、种子与比分。
- [Confirmed] [应用状态机](../../development/design/application-state-machine.md)：`CommittedTransition` 记录本次迁移的 cause。
- [Confirmed] [PRD §8](../../PRD.md)：R1 需完成一场本地 BO3 并回到赛果页。

**设计基线：** 实例处置由已提交迁移的 cause 唯一决定；暂停只停止推进、不改变实例。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义 fixed schedule 的运行边界与暂停后 tick 停止、恢复延续（见 [输入与固定调度](input-and-fixed-tick.md) 的 TC-006～TC-008）、状态边合法性与仲裁（见 [应用生命周期](application-lifecycle.md)）、`MatchState` 内部的小局与 BO3 推进（见 [小局、BO3 与安全点](match-and-round.md)），也不定义 HUD 内容（见 [表现运行时](presentation-runtime.md)）。

## 测试点清单

### Component Integration — Match Flow

- 四个 cause 分别新建、重建、以新种子新建与保留实例（TC-001～TC-004）。
- 退出 `Match` 按目标状态决定是否释放实例与随对局资源（TC-005～TC-006）。
- 创建失败不留下半初始化实例（TC-007）。
- 比赛结束只进入一次 `Result`（TC-008）。

### System — Match Flow

- 单人与本地双人从主菜单完成 BO3 并回到赛果（TC-009；Concern: Smoke）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 判定表 | cause × 实例处置、种子与比分 | TC-001～TC-004 |
| 状态迁移 | 退出 `Match` 的两类目标状态 | TC-005～TC-006 |
| 错误猜测 | 冻结失败与结束 tick 后继续推进 | TC-007～TC-008 |
| 场景 / 协作路径 | 两种模式的完整对局流程 | TC-009 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | `CharacterConfirmed` 进入 Match 时按冻结规格新建实例 | P0 | Component Integration | — | Match Flow；Client | 最小客户端 app，已完成开局规格冻结 | 以 `CharacterConfirmed` 提交 `CharacterSelect → Match` 并运行进入处理 | 冻结种子 `seed=7`；两个 participant slot | 产生可推进的规则实例；实例使用冻结的 `LockedMatchSpec` 与 `seed=7`；`wins` 为 `0:0`；`match_tick` 为 0 | [Confirmed] [固定频率规则调度：对局实例生命周期](../../development/design/fixed-tick-simulation.md#对局实例生命周期) |
| TC-002 | `ResumeRequested` 保留既有实例 | P0 | Component Integration | Determinism | Match Flow；Client | Match 中已推进至非初始状态并记录状态校验和 | 迁移到 `Paused`，再以 `ResumeRequested` 迁回 `Match` | 暂停前 `match_tick=N`、校验和 `C`、`wins=1:0` | 实例未被替换；恢复后 `match_tick` 与校验和仍为 `N` 与 `C`；`wins` 保持 `1:0` | [Confirmed] [固定频率规则调度：对局实例生命周期](../../development/design/fixed-tick-simulation.md#对局实例生命周期) |
| TC-003 | `RestartRequested` 用同一规格与同一种子重建实例 | P0 | Component Integration | Determinism | Match Flow；Client | Match 中已推进若干 tick 且 `wins=1:0` | 迁移到 `Paused`，以 `RestartRequested` 迁回 `Match` | 同一 `LockedMatchSpec`，种子保持 `seed=7` | 产生新实例；`LockedMatchSpec` 与种子与重建前相同；`wins` 归零为 `0:0`；`match_tick` 归零；以相同输入推进得到与本场开局相同的球序 | [Confirmed] [固定频率规则调度：对局实例生命周期](../../development/design/fixed-tick-simulation.md#对局实例生命周期) |
| TC-004 | `RematchRequested` 重新冻结并使用新种子 | P1 | Component Integration | Determinism | Match Flow；Client | 已进入 `Result` 且上一场使用 `seed=7` | 以 `RematchRequested` 提交 `Result → Match` 并运行进入处理 | 上一场 `seed=7`；再战产生新的本地种子 | 产生新实例；模式、双方角色与规则剖面与上一场相同；种子不等于 `seed=7`；`wins` 为 `0:0`；相同输入下首手球序与上一场不同 | [Confirmed] [固定频率规则调度：对局实例生命周期](../../development/design/fixed-tick-simulation.md#对局实例生命周期)；[应用状态机：协作](../../development/design/application-state-machine.md#协作) |
| TC-005 | 退出 Match 且目标不是 Paused 时释放对局资源 | P1 | Component Integration | — | Match Flow；Client | Match 中已存在实例、AI 计划状态与随对局存在的表现资源 | 参数化以两种 cause 退出 Match | `MatchCompleted`（→`Result`）；`MatchAbandoned`（→`MainMenu`） | 两种情况均释放规则实例、AI 计划状态与随对局存在的表现资源；不残留可推进的旧实例 | [Confirmed] [固定频率规则调度：对局实例生命周期](../../development/design/fixed-tick-simulation.md#对局实例生命周期) |
| TC-006 | `Match → Paused → Match` 不释放对局资源 | P1 | Component Integration | — | Match Flow；Client | Match 中已存在实例与随对局存在的表现资源 | 迁移到 `Paused` 后再迁回 `Match` | `PauseRequested` 后 `ResumeRequested` | 全程不触发释放；表现资源在 `Paused` 期间保持存在；实例标识与迁移前相同 | [Confirmed] [固定频率规则调度：对局实例生命周期](../../development/design/fixed-tick-simulation.md#对局实例生命周期) |
| TC-007 | 实例创建失败时不留下半初始化对局 | P1 | Component Integration | — | Match Flow；Client | 可注入使创建失败的条件 | 以 `CharacterConfirmed` 进入 Match，创建过程失败 | 冻结失败或实例构建失败 | 不存在可推进的实例；`FixedGameSet::Rules` 不推进任何规则状态；失败原因可观察；后续一次成功创建仍能正常开始对局 | [Confirmed] [固定频率规则调度：对局实例生命周期](../../development/design/fixed-tick-simulation.md#对局实例生命周期) |
| TC-008 | 比赛结束只进入一次 Result | P1 | Component Integration | — | Match Flow；Client | Match 中一方即将达到两胜 | 推进到比赛结束 tick，并在其后继续提供若干 fixed 执行机会 | 结束后 10 个 fixed 执行机会 | `Match → Result` 只提交一次；后续执行机会不再产生迁移请求；`Result` 的 `OnEnter` 只触发一次 | [Confirmed] [应用状态机：请求处理](../../development/design/application-state-machine.md#请求处理)；[小局、BO3 与安全点：完成态](../../development/design/match-and-round.md#完成态) |
| TC-009 | 单人与本地双人均可从主菜单完成 BO3 并回到赛果 | P0 | System | Smoke | Match Flow；Client | 已装配客户端，规则数据可用 | 分别以两种模式走完整流程：主菜单 → 模式 → 选角 → BO3 → 赛果 → 返回主菜单 | 单人（P1 + AI）；本地双人（P1 + P2）；每种模式打到某一方两胜 | 两种模式均能完成 BO3；某一方达到两胜后进入 `Result` 并显示获胜方与局分；「返回主菜单」回到 `MainMenu`；全程只有一个当前 `AppState`；单人模式下 AI 只控制自己的槽位 | [Confirmed] [PRD §8](../../PRD.md)；[页面导航与焦点：页面与迁移](../../development/design/page-navigation.md#页面与迁移) |

## 风险查漏

四个 cause 的实例处置、种子与比分、资源释放、创建失败与结束去重均有直接用例；暂停期间的 tick 停止由 `integration-system/input-and-fixed-tick::TC-007` 覆盖。
