# 测试用例设计：AI 参与者

**关联设计：** [AI 参与者](../../development/design/ai-player.md)、[DEC-006](../../development/decision/ai-baseline.md)
**关联实现：** `crates/client`（`ai`、`simulation`）、`crates/game_core`（`view`）

## 需求理解摘要

**功能：** 单难度实时 AI 通过 participant slot 的合法动作参加对局，并满足基础连锁、抵消、危险识别与 Fever 利用的基线。
**测试性质：** 新功能
**本轮范围：** 规划触发、候选生成与评价、动作执行，以及 AI 对局的可复现性。
**Test Basis：**
- [Confirmed] [AI 参与者](../../development/design/ai-player.md)：读模型边界、四个行为与固定时序。
- [Confirmed] [PRD §4.3](../../PRD.md)：AI 使用与人类相同的规则与可见信息，只通过合法输入行动，并给出单难度验收基线。
- [Confirmed] [DEC-006](../../development/decision/ai-baseline.md)：单一均衡评价器与固定时序。
**设计基线：** AI 不拥有修改规则状态的入口，其计划与时序不进入规则快照。
**关键假设：**
- AI 时序是 `turn_id` 与计划步序的纯函数，不消费随机数。
- 必死候选排序垫底而不剔除，因此候选集不会为空。
**待确认问题：**
- 思考延迟与按键间隔的数值为校准项（[DEC-006](../../development/decision/ai-baseline.md)）；校准后需同步更新以其为测试数据的用例。

## 测试点清单

### Component Integration — AI

- 四种形状与关键障碍盘面上，AI 输出的每个动作都合法且最终到达计划姿态。
- 存在确定抵消、立即溢出风险与可进入 Fever 三类场景时，选择满足生存基线的候选。
- 相同 `PlayerView` 重复规划得到相同计划；镜像盘面得到镜像结果（Concern: Determinism）。
- AI 不读取未进入 `NextQueue` 的颜色与题面随机状态。
- 全部候选都会导致失败时仍产出计划并以硬降收尾，不出现原地不落子。

### Component Integration — AI；Match Flow

- 固定规则与种子下连续至少 20 场 AI 对局全部正常结束，无非法落子、卡死或直接状态修改（Concern: Determinism）。

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
