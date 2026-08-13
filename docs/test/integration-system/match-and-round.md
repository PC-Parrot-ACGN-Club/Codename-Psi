# 测试用例设计：小局、BO3 与安全点

**关联设计：** [小局、BO3 与安全点](../../development/design/match-and-round.md)
**关联实现：** `crates/game_core`（`match_state`、`round`、`player`、`view`）

## 需求理解摘要

**功能：** 以唯一入口同步消费双方动作，在安全点仲裁跨玩家结果，并推进小局与 BO3。
**测试性质：** 新功能
**本轮范围：** 安全点的六步顺序、失败判定与同时失败、小局初始化、完成态，以及需要两名玩家才能证明的攻防与结算行为。
**Test Basis：**
- [Confirmed] [小局、BO3 与安全点](../../development/design/match-and-round.md)：聚合模型、安全点顺序与五个行为。
- [Confirmed] [玩法设计 §6.1](../../gameplay.md)：BO3、同时失败判和、重打得到不同随机序列、比赛由某方两胜结束。
- [Confirmed] [得分、攻击与垃圾攻防](../../development/design/offense-and-nuisance.md)：双方只抵消进入安全点时已有的队列数量。
**设计基线：** 跨玩家写操作只发生在聚合根，participant slot 的迭代顺序不影响结果。
**关键假设：**
- 失败判定只有一种检查，普通盘与 Fever 盘共用。
- 每局 RNG 由根种子、局号与重打次数独立派生。
**待确认问题：**
- 局间倒计时与结果停留的时长由规则剖面配置，属校准项；校准后需同步更新以其为测试数据的用例。

## 测试点清单

### Component Integration — Match Flow

- 双方在同一 tick 到达不同或相同安全点时，迭代顺序不影响最终状态。
- 普通盘与 Fever 盘的出生失败都按同一条规则结束小局。
- 同一安全点双方失败得到 `Draw`、胜场不变、局号不前进；错开一个 tick 则按正常胜负结算。
- 和局重打的球序与上一次不同；`MatchOutcome` 始终由某方两胜产生（Concern: Determinism）。
- 2:0 与 2:1 两种 BO3 走向；角色选择跨局保留，局内状态正确重置。
- `RoundIntro` 与 `RoundOutro` 忽略玩法动作，首个开放 tick 对双方对称。
- `MatchEnded` 恰好产生一次；完成态继续调用 `step` 只推进总 tick 且不产生事件。

### Component Integration — Rules；Match Flow

- 一名玩家处于结算阶段而另一名操控活动组时，两方状态各自正确推进。
- 双方同安全点攻击、双方各有旧队列、单方与双方无连锁的组合矩阵。
- participant slot 对调后，镜像初始状态得到镜像结果。

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
