# 测试用例设计：得分、攻击与垃圾攻防

**关联设计：** [得分、攻击与垃圾攻防](../../development/design/offense-and-nuisance.md)、[连锁强度曲线](../../development/design/chain-power-curve.md)、[DEC-003](../../development/decision/nuisance-queue-representation.md)
**关联实现：** `crates/game_core`（`scoring`、`attack`、`nuisance`）

## 需求理解摘要

**功能：** 把连锁事实换算成分数与攻击，完成余数携带、抵消与垃圾落盘。
**测试性质：** 新功能
**本轮范围：** 连锁步计分、分数换算攻击、单方抵消与垃圾落下；双方安全点仲裁由聚合根测试稿覆盖。
**Test Basis：**
- [Confirmed] [得分、攻击与垃圾攻防](../../development/design/offense-and-nuisance.md)：数据模型与四个行为。
- [Confirmed] [玩法设计 §4.1](../../gameplay.md)：计分公式、`CB`/`GB` 表、目标分与 margin、软降加分不计入换算。
- [Confirmed] [玩法设计 §4.2](../../gameplay.md)：连续抵消、单次落下上限与两种余数列顺分支。
- [Confirmed] [Scoring](https://puyonexus.com/wiki/Scoring)：余数进位公式与 List of Chain Scores 逐链样本。
**设计基线：** 攻击换算不区分对手类型；待接收垃圾为每通道一个精确整数。
**关键假设：**
- 余数携带使逐 link 换算与整链一次换算的总量恒等，因此两者可互为 golden 样本。
- `MarginState` 只持有整数表下标，`TP` 由查表取得。
**待确认问题：**
- 无。

## 测试点清单

### Component — Rules

- 计分公式的单链、多组、多色与倍率 clamp 样例（Concern: Content Validation）。
- 逐 `ChainLink` 换算的累计结果与整链一次换算逐点相等。
- 两名角色的普通盘与 Fever 盘曲线查询、表尾行为，以及交换角色后攻击结果随 profile 变化。
- 跨多次攻击的余数守恒；margin 阶段推进只改变表下标。
- 连续抵消、未连锁落下、单次上限 30、整行填充与两种余数列顺分支。

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
