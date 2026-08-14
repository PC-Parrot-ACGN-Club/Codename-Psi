# 小局、BO3 与安全点

**相关模块：** `game_core::match_state`、`game_core::round`、`game_core::player`、`game_core::view`、`client::simulation`
**关联文档：** [玩法设计 §6](../../gameplay.md)、[规则配置与开局规格冻结](rule-configuration.md)、[固定频率规则调度](fixed-tick-simulation.md)

## 目标

把两名玩家的规则状态组合成唯一的对局聚合根，以一个入口同步消费双方动作，定义跨玩家结算的确定性边界，并推进小局与 BO3。

## 数据模型

```text
MatchState
├─ spec: LockedMatchSpec
├─ match_tick
├─ phase
│  ├─ RoundIntro { round, remaining_ticks }
│  ├─ Playing(RoundState)
│  ├─ RoundOutro { result, remaining_ticks }
│  └─ Completed(MatchOutcome)
├─ wins: [u8; 2]
├─ round_index / draw_attempt
├─ round_history
└─ rng

RoundState
├─ round_tick
├─ players: [PlayerBattleState; 2]
├─ pending_settlement: [Option<PlayerSettlement>; 2]
└─ outcome: Option<RoundOutcome>
```

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `MatchState` | 规则聚合根 | 跨玩家写操作的唯一所有者 |
| `PlayerBattleState` | 单名玩家的全部规则状态 | 可独立推进操控与单盘结算，但不能直接改对手状态或宣布胜负 |
| `RoundOutcome` | 小局结果 | `Decided(slot)` 或 `Draw` |
| `MatchOutcome` | 比赛结果 | 只能是某一方达到两胜 |
| `draw_attempt` | 同一局号的重打次数 | 参与 RNG 派生 |

## 行为

### 一个规则 tick

- 输入：恰好包含两个 participant slot 的 `TickInputs`。
- 处理：
  1. 校验槽位数量；`match_tick` 加一。
  2. `RoundIntro` / `RoundOutro` 只推进确定性计时并忽略玩法动作；倒计时与结果停留的时长来自规则剖面。
  3. `Playing` 中从同一 tick 的起始快照推进双方各自的动作与阶段。
  4. 收集双方本 tick 到达的 `PlayerSettlement`，按下节顺序执行安全点。
  5. 小局结束时形成 `RoundOutcome` 并更新胜场；某方达到两胜时形成 `MatchOutcome`，否则建立下一小局。
- 输出：`MatchStepReport`；事件按固定类别与 participant slot 排序。
- 错误语义：槽位数量错误时拒绝且不修改任何状态。

任一玩家先进入计算不代表其规则事实优先；跨玩家结果只在安全点的批处理中生效。

### 安全点

安全点是双方本次规则结果均已形成、允许攻防仲裁或模式切换的确定性边界。到达安全点后按以下固定顺序解释已经形成的事实：

```text
1. 收齐双方 ChainReport
2. 仲裁攻击与抵消
3. 应用全消与 Fever 转换意图
4. 应用应落垃圾并完成其稳定
5. 检查失败
6. 形成 RoundOutcome
```

该顺序由单个纯函数或状态表持有，不依赖 Bevy system 的注册顺序。第 2 步先收齐双方的 `AttackIntent` 再批量结算，因此 participant slot 的迭代顺序不影响结果。

第 2 步与第 4 步都以进入安全点时的队列数量为输入：本安全点新收到的攻击既不被本次抵消，也不进入本次落下批次，而是留到下一个安全点。

### 失败判定

- 输入：安全点第 5 步的双方状态。
- 处理：判负只有一种检查——生成下一活动组时出生列上格已被占据。普通盘与 Fever 盘使用同一条规则。
- 输出：`PlayerDefeated` 与 `RoundOutcome`。
- 错误语义：同一批检查中双方条件同时成立时记为 `Draw`——双方胜场不变，`round_index` 不前进，`draw_attempt` 加一并重打同一局号。一方先失败、另一方在后续 tick 才失败按正常胜负结算。

和局不产生比赛级平局，也不设重打次数上限。

### 小局初始化

- 输入：`LockedMatchSpec`、`round_index`、`draw_attempt`。
- 处理：从根种子与这两个编号独立派生各命名 RNG 流，不使用墙钟或系统熵；重置双方盘面、分数、攻击余数、Fever 量表、Fever 时间与队列；保留 BO3 胜场与双方角色。
- 输出：新的 `RoundState`。
- 错误语义：重打必须得到与上一次不同的球序，否则确定性 AI 会逐字复现同一场同时失败。

`RoundIntro` 结束时同时为双方开放第一个可操作 tick。

### 完成态

- 输入：`Completed` 状态下的 `step` 调用。
- 处理：只推进 `match_tick`。
- 输出：不含任何事件的 `MatchStepReport`。
- 错误语义：不返回错误，也不再改变胜场或结果。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `LockedMatchSpec` | [开局规格冻结](rule-configuration.md) | 本主题 | 整场不可变 |
| `TickInputs` | `client::simulation` | 本主题 | 每个固定 tick 恰好一次 |
| `MatchStepReport` | 本主题 | `client::simulation` | 缓存最新一份供表现读取 |
| `MatchView` | 本主题 | 表现层 | 两块盘、活动组与 NEXT、精确垃圾、Fever、分数、角色与胜场的只读投影，不是平行状态 |
| `MatchEnded` | 本主题 | `client::app_state` | 由 client 转换为既有的比赛完成迁移原因 |

`client::simulation` 在固定调度的规则阶段调用 `MatchState::step`，是一层薄桥接，不持有规则判定。

## 边界

- 本文不定义单盘的操控、结算、攻防与 Fever 内部规则（见[盘面与活动组操控](board-and-falling-group.md)、[连锁结算](chain-resolution.md)、[得分、攻击与垃圾攻防](offense-and-nuisance.md)、[Fever 循环](fever-mode.md)）。
- 本文不定义固定调度的阶段划分与运行条件（见[固定频率规则调度](fixed-tick-simulation.md)）。
- 本文不定义菜单、角色选择、赛果渲染与暂停 UI（见[应用状态机](application-state-machine.md)、[表现与 UI 设计](../../presentation.md)）。
- 本文不定义快照编码与 checksum 算法（见[玩法设计 §6.2](../../gameplay.md)）。

## Test Basis

- [玩法设计 §6.1](../../gameplay.md)：BO3、同时失败判和且比分不变、重打得到不同随机序列、比赛由某方两胜结束。
- [玩法设计 §6.2](../../gameplay.md)：AI、本地双人与网络会话复用同一个双槽位入口。
- [TDD §3](../../TDD.md)：规则以固定 60Hz tick 推进，规则状态可复制并产生稳定校验和。
- [Issue #12](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/12)：要求双方同 tick 仲裁、小局胜负、BO3 与 client 规则桥。
