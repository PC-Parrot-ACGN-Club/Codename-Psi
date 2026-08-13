# 测试用例设计：连锁结算

**关联设计：** [连锁结算](../../development/design/chain-resolution.md)、[DEC-004](../../development/decision/settlement-timing-values.md)
**关联实现：** `crates/game_core`（`resolution`）

## 需求理解摘要

**功能：** 把一次锁定后的盘面推进到稳定状态，并产出每个连锁步的完整事实。
**测试性质：** 新功能
**本轮范围：** 扫描与清除、重力稳定、阶段计时与表现协议、报告完成。
**Test Basis：**
- [Confirmed] [连锁结算](../../development/design/chain-resolution.md)：五段状态机、隐藏行排除、阶段时长与三个行为。
- [Confirmed] [玩法设计 §3.5](../../gameplay.md)：阶段提交语义、表现不得推进规则、攻击只在 `ClearCommit` 逐步生效。
- [Confirmed] [Basic rules](https://puyonexus.com/wiki/Basic_rules)：四向连通达到四个即消除，垃圾通过邻接消除组清除。
**设计基线：** 结算阶段以整数 tick 提交，同一规则配置与盘面得到相同阶段时长。
**关键假设：**
- `ClearCommit` 是零时长边界，其余四态跨 tick 保存。
- 重力与分裂自由落体共用同一张下落格数时长表。
**待确认问题：**
- 消除预览时长与连锁重力参数为校准项（[DEC-004](../../development/decision/settlement-timing-values.md)）；校准后需同步更新以 tick 为测试数据的用例。

## 测试点清单

### Component — Rules

- 单组四连、多个同时成立的组、多色组与阈值以下的组。
- 一颗、多颗以及被多个消除组共享的邻接垃圾按坐标去重清除。
- 隐藏行中的球不参与连通组、不被相邻垃圾规则清除、不阻止 `all_clear`。
- 重力产生二连锁以上时，逐步报告的组大小、颜色数与最终盘面正确。
- `ClearPreview` 到期前盘面仍包含待消球；`ClearCommit` 才产生攻击；`Gravity` 到期才提交目标盘面。
- 无连锁与全消等边界结果正确。
- Fever 在结算过程中归零时完成当前连锁，并在 `Settlement` 退出。
- 推进 tick 的方式与是否存在表现消费者不改变规则阶段的 tick 序列（Concern: Determinism）。

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
