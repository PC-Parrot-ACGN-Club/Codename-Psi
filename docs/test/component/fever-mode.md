# 测试用例设计：Fever 循环

**关联设计：** [Fever 循环](../../development/design/fever-mode.md)
**关联实现：** `crates/game_core`（`fever`）

## 需求理解摘要

**功能：** 量表、进场、题面循环、时间奖励、全消组合与退场的完整 Fever 生命周期。
**测试性质：** 新功能
**本轮范围：** 单名玩家的 Fever 状态推进；跨玩家的时间奖励来源由聚合根测试稿构造。
**Test Basis：**
- [Confirmed] [Fever 循环](../../development/design/fever-mode.md)：双通道模型、玩家级时间与五个行为。
- [Confirmed] [玩法设计 §5.1、§5.2、§5.3](../../gameplay.md)：量表与进入条件、双通道与冻结语义、奖励发放时刻、三种全消组合。
- [Confirmed] [Fever (rule)](https://puyonexus.com/wiki/Fever_%%28rule%%29)：Fever 1/2 主机/PC 列的计时、题面下限与翻盘行为。
**设计基线：** Fever 时间是玩家级持久值，冻结的队列不落垃圾但可被抵消。
**关键假设：**
- 题面按等级独立无重复袋选择，袋状态属于规则状态。
- 归零翻盘的立即落下遵守单次上限。
**待确认问题：**
- 无。

## 测试点清单

### Component — Rules

- 量表从 0 填满进入 Fever；一个安全点最多增加一格；时间不超过上限。
- 不在 Fever 时全消也能累加 Fever 时间并 clamp 到上限。
- 时间奖励归被抵消的攻击方，抵消方不获得时间。
- 时间奖励在最后一个连锁步开始消除动画的 tick 发放。
- 普通盘全消立即在普通盘上装载预设 4 连题面。
- 等级域内各目标等级的题面都能加载且合法（Concern: Content Validation）。
- 达标、全消、差 1、差 2、差 3 及以上五个分支的下一等级正确。
- 双盘与双队列的冻结、合并与归零后立即落下。

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
