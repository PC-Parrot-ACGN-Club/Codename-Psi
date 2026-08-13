# 测试用例设计：确定性、快照与状态校验

**关联设计：** [确定性、快照与状态校验](../../development/design/determinism-and-snapshot.md)、[DEC-005](../../development/decision/color-sequence-derivation.md)
**关联实现：** `crates/game_core`（`determinism`、`verification_log`）

## 需求理解摘要

**功能：** 保证相同开局规格、根种子与逐 tick 输入得到相同状态，并提供快照、恢复、校验和与验证日志。
**测试性质：** 新功能
**本轮范围：** 随机流派生、快照与恢复、状态校验和、验证日志运行。
**Test Basis：**
- [Confirmed] [确定性、快照与状态校验](../../development/design/determinism-and-snapshot.md)：确定性约束、快照覆盖表与四个行为。
- [Confirmed] [TDD §3、§6](../../TDD.md)：固定 tick 推进、深拷贝快照与包含回滚的千 tick 校验和一致。
- [Confirmed] [DEC-005](../../development/decision/color-sequence-derivation.md)：两名玩家的随机流完全独立派生。
**设计基线：** 快照与校验和只在相同摘要、随机算法版本与状态编码版本之间可比。
**关键假设：**
- 规则事件不进入快照与校验和，产生事件所依赖的持久字段进入。
- 快照覆盖表中的每通道队列与列序位置、玩家级 Fever 时间、每等级无重复袋状态都必须进入，遗漏任一项都会使恢复不收敛。
**待确认问题：**
- 无。

## 测试点清单

### Component — Rules

- 随机流派生与前 N 个输出的 golden vectors（Concern: Determinism）。
- 两名玩家的同名流互不相同且互不影响（Concern: Determinism）。

### Component Integration — Rules；Match Flow

- 同一验证日志重复运行、以及在不同进程中运行，得到相同的 checkpoint 校验和（Concern: Determinism）。
- 在活动组、连锁中间、垃圾下落、Fever 中与局间五个阶段分别快照并恢复后继续推进，状态收敛（Concern: Determinism）。
- 从同一快照分叉不同输入使校验和分叉；恢复相同输入后再次收敛（Concern: Determinism）。
- 单个持久字段变化能改变校验和；事件队列的消费方式不改变校验和（Concern: Determinism）。
- 摘要、schema 版本或算法版本不匹配时拒绝恢复并返回 typed error。
- 任意结算阶段与任意 Fever tick 的快照恢复后，连锁报告、题面选择与剩余时间一致（Concern: Determinism）。

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
