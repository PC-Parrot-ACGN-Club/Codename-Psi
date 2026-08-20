# Fever 循环

**相关模块：** `game_core::fever`
**关联文档：** [玩法设计 §5](../../gameplay.md)、[得分、攻击与垃圾攻防](offense-and-nuisance.md)、[小局、BO3 与安全点](match-and-round.md)

## 目标

在普通落子、连锁与攻防能力之上建立完整的 Fever 生命周期：量表、进场、题面、时间、等级升降、全消奖励、垃圾隔离与退场。活动组操控与连锁算法复用既有主题，不建立 Fever 专用副本。

## 数据模型

```text
PlayerBattleState
├─ active_channel: Normal | Fever
├─ normal: FieldChannel
├─ fever:  FieldChannel
├─ fever_gauge
├─ fever_time_ticks
└─ fever_session: Option<FeverSession>
     ├─ target_level
     ├─ current_puzzle_id
     └─ puzzle_bags
```

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `FieldChannel` | 一块盘面及其待接收垃圾与列序状态 | 任一时刻只有活动通道接受落子；非活动通道**冻结**——不落下垃圾，但仍可被抵消 |
| `FeverGauge` | 进入 Fever 的进度 | `0..=capacity`；只由已确认的抵消事实增加，一个安全点最多增加一格 |
| `fever_time_ticks` | Fever 可用时间 | **玩家级持久值**，不属于会话；不处于 Fever 时同样存在并可被奖励累加，clamp 在剖面声明的上下限内。显示秒数是向下取整的只读投影 |
| `FeverLevel` | 当前题面目标连锁 | 所有升降结果 clamp 到剖面声明的等级域 |
| `puzzle_bags` | 每个等级各一个无重复袋 | 袋空时按显式 RNG 流重新装填；袋状态进入快照 |
| `FeverResult` | 一次题面的结果 | 实际连锁、目标连锁、是否全消 |

时间不放在会话内，因为不在 Fever 时的全消同样为其累加（[玩法设计 §5.3](../../gameplay.md)）。

## 行为

### 量表与时间奖励

- 输入：安全点上的 `OffsetFacts`。
- 处理：有效连锁抵消了待接收垃圾时量表加一格。**己方连锁被对手抵消时，己方 `fever_time_ticks` 加 1 秒**——奖励归被抵消的攻击方。Fever 中的时间奖励在**最后一个连锁步开始消除动画的 tick** 发放，不等到 `Settlement`。
- 输出：更新后的量表与时间。
- 错误语义：全部时间奖励 clamp 到上限；量表满后不再增加。

### 进入 Fever

- 输入：安全点上的量表、玩家级 `fever_time_ticks`。
- 处理：量表满时把 `active_channel` 切为 Fever 并重置量表，冻结普通通道，以 `fever_time_ticks` 起算会话，按剖面声明的基准等级确定初始目标等级，再从对应等级的袋中取出题面装载到 Fever 盘。
- 输出：`FeverEntered` 与已装载的 Fever 盘。
- 错误语义：只在当前落子完整结算后的安全点检查量表。

下一活动组仍通过统一的供给流程生成。

### 题面循环

- 输入：一次 Fever 落子的 `ChainReport` 与当前目标等级。
- 处理：

  ```text
  达标        → next = actual + 1
  Fever 全消  → next = actual + 2
  差 1        → next = target
  差 2        → next = actual - 1
  差 ≥ 3      → next = actual - 2
  ```

  结果 clamp 到等级域后，从该等级的袋中确定性取出下一题并替换 Fever 盘。替换不区分达标与否——每次安全点结算（含 0 连）都执行本流程。
- 输出：新的目标等级与题面，以及 `FeverPuzzleAdvanced`。
- 错误语义：时间已耗尽时不再装载题面。

### 全消

- 输入：`FieldFacts.all_clear` 与当前通道。
- 处理：普通盘全消时**立即在普通盘上装载一个预设 4 连题面**，并为 `fever_time_ticks` 加 5 秒；Fever 内全消时下一题目标按 `actual + 2` 且加 5 秒；全消同时进入 Fever 时首题目标加 2 且加 5 秒。
- 输出：`FeverTransitionIntent`。
- 错误语义：多个效果由同一张优先级表一次结算，系统执行顺序不改变结果。

### 退出 Fever

- 输入：归零的时钟或已完成的结算。
- 处理：时钟扣减不因结算演出暂停；归零时标记退出待处理，已开始的连锁结算到 `Settlement` 安全点后再退出。退出时把 Fever 通道的队列数量并入普通通道并清零，普通通道恢复为活动通道，Fever 会话与其列序状态一并丢弃。归零瞬间未能抵消且任一队列有垃圾时，翻回后立即触发一次落下——该次落下同样遵守单次上限，余量留在队列。
- 输出：`FeverExited` 与合并后的队列。
- 错误语义：Fever 盘的失败与普通盘同规则，在生成活动组时检查出生列上格，不因退出而规避。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `OffsetFacts` | [得分与攻防](offense-and-nuisance.md) | 本主题 | 量表与时间奖励的唯一来源 |
| `FeverTransitionIntent` | 本主题 | 安全点 | 在攻防仲裁之后统一应用 |
| 活动通道 | 本主题 | [得分与攻防](offense-and-nuisance.md) | 决定抵消先消耗哪个队列、垃圾落到哪块盘面 |
| 剩余时间与题面等级 | 本主题 | 表现层 | 只读投影，显示秒数向下取整 |

## 边界

- 本文不定义抵消与垃圾落下的算法（见[得分、攻击与垃圾攻防](offense-and-nuisance.md)）。
- 本文不定义连锁扫描与阶段计时（见[连锁结算](chain-resolution.md)）。
- 本文不定义安全点上各类意图的应用顺序（见[小局、BO3 与安全点](match-and-round.md#安全点)）。
- 本文不定义题面内容本身与其合法性约束（见[规则配置与开局规格冻结](rule-configuration.md)）。

## Test Basis

- [玩法设计 §5.1](../../gameplay.md)：七格量表、时间范围、进入条件、+1 秒归被抵消的攻击方、Fever 时间为玩家级持久值。
- [玩法设计 §5.2](../../gameplay.md)：双通道与冻结语义、题面等级域、奖励时间发放时刻、归零翻盘与立即落下。
- [玩法设计 §5.3](../../gameplay.md)：三种全消组合的效果。
- [Fever (rule)](https://puyonexus.com/wiki/Fever_%28rule%29)：Fever 1/2 主机/PC 列的计时、题面下限、翻盘与立即落下行为。
- [Issue #12](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/12)：要求量表、题面、时间、升降级、全消与退出。
