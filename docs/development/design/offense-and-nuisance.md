# 得分、攻击与垃圾攻防

**相关模块：** `game_core::scoring`、`game_core::attack`、`game_core::nuisance`
**关联文档：** [玩法设计 §4](../../gameplay.md)、[连锁结算](chain-resolution.md)、[小局、BO3 与安全点](match-and-round.md)、[DEC-003](../decision/nuisance-queue-representation.md)

## 目标

把连锁事实换算成分数与攻击，完成余数携带、抵消、入队与垃圾落盘，并输出 Fever 量表与时间奖励所需的抵消事实。

## 数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `ScoreState` | 玩家分数 | 显示分数与不计入攻击的软降加分分开保存 |
| `AttackFraction` | 跨攻击携带的余数 | 整数或有理数表达，规则状态中不出现浮点 |
| `MarginState` | 目标分衰减状态 | 小局已过 tick 与 margin 整数表下标；`TP` 由下标查表取得 |
| `AttackIntent` | 安全点上尚未仲裁的攻击 | 精确整数与来源事实 |
| `PendingNuisance` | 一个盘面通道的待接收垃圾 | **单个精确整数**，上限由规则剖面声明并在配置校验中检查 |
| `NuisanceDropState` | 固定列序位置 | **每个盘面通道各一份**，随通道创建与销毁 |
| `OffsetFacts` | 一次抵消的事实 | 抵消数量、是否由有效连锁触发、攻守双方 |

队列是标量而非批次列表，理由与被否决的方案见 [DEC-003](../decision/nuisance-queue-representation.md)。UI 的分级图标是对该整数的投影，精确数字是唯一真相。

## 行为

### 连锁步计分

- 输入：一个 `ChainLinkFacts`、当前角色与盘面模式。
- 处理：

  ```text
  link_score = (10 × cleared_colored)
             × clamp(chain_power + color_bonus + group_bonus, 1, 999)
  ```

  `chain_power` 按角色、盘面模式与连锁步查已冻结的曲线，超过表尾时取表尾值；`color_bonus` 按本步颜色数查表；`group_bonus` 为本步各组倍率之和。
- 输出：本步分数。
- 错误语义：中间值使用检查过的整数宽度，配置校验保证合法数据不溢出。

软降加分只增加显示分数，不进入攻击换算。

### 分数换算攻击

- 输入：本连锁步分数、当前 `TP`、`AttackFraction`。
- 处理：按 `NP = SC / TP + NL` 取整得到整数垃圾量，余数留在攻击方状态并跨连锁步与跨落子延续。**逐 `ChainLink` 换算**，不在落子结束时一次换算。
- 输出：`AttackIntent`。
- 错误语义：换算不经任何对手类型系数调整。

余数携带使「逐步取整之和」与「整链一次取整」的总量恒等，因此按步拆分不改变结果，只满足[玩法设计 §3.5](../../gameplay.md) 对发布时机的要求。

### 抵消

- 输入：本方 `AttackIntent`、本方两个通道的 `PendingNuisance`。
- 处理：**先消耗活动通道的队列，溢出部分再消耗另一通道的队列**；两个通道都清空后仍有余量时形成送往对手的攻击。抵消只作用于安全点开始时已有的数量，本安全点新收到的攻击留到下一个安全点。
- 输出：`OffsetFacts` 与送出的残余攻击。
- 错误语义：队列不会减到负数。

### 垃圾落下

- 输入：活动通道的 `PendingNuisance`、`NuisanceDropState`。
- 处理：本次落子触发了至少一个连锁时不落下，玩家继续获得下一活动组；未触发连锁且队列非零时按单次上限取出一批落入活动盘面。整行先填；不足一行时按[玩法设计 §4.2](../../gameplay.md) 的分支推进列序——凑满整行后余 1 颗则下一次首颗从下一列开始，余 2 颗及以上则下一次首颗从上一颗所在列开始。
- 输出：落盘坐标与更新后的队列、列序位置。
- 错误语义：落下量不超过单次上限，余量留在队列。

垃圾落盘后进入[连锁结算](chain-resolution.md)的重力流程。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `ChainLinkFacts` | [连锁结算](chain-resolution.md) | 本主题 | 在对应 `ClearCommit` tick 到达 |
| `AttackIntent` | 本主题 | 安全点 | 由聚合根批量仲裁，本主题不直接写对手状态 |
| `OffsetFacts` | 本主题 | [Fever 循环](fever-mode.md) | 量表增长与时间奖励的依据 |
| 落盘坐标与队列总数 | 本主题 | 表现层 | 精确整数；图标分级由表现层投影 |

## 边界

- 本文不定义安全点上双方攻击的仲裁顺序（见[小局、BO3 与安全点](match-and-round.md#安全点)）。
- 本文不定义 Fever 通道的创建、冻结与合并（见[Fever 循环](fever-mode.md)）。
- 本文不定义 `CP` 曲线的产生与校验（见[连锁强度曲线](chain-power-curve.md)）。
- 本文不定义分数、`CB`/`GB` 表与 margin 表的具体数值（见[玩法设计 §4.1](../../gameplay.md)）。

## Test Basis

- [玩法设计 §4.1](../../gameplay.md)：计分公式、`CB`/`GB` 表、目标分与 margin、软降加分不计入换算、攻击换算不区分对手类型。
- [玩法设计 §4.2](../../gameplay.md)：连续抵消、单次落下上限与两种余数列顺分支。
- [Scoring](https://puyonexus.com/wiki/Scoring)：余数进位公式与 List of Chain Scores 逐链样本。
- [Issue #12](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/12)：要求得分、攻击余数、抵消、垃圾队列与落下。
