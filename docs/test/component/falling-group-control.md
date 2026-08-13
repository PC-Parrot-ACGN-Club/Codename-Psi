# 测试用例设计：盘面与活动组操控

**关联设计：** [盘面与活动组操控](../../development/design/board-and-falling-group.md)、[DEC-002](../../development/decision/timing-parameter-source.md)
**关联实现：** `crates/game_core`（`board`、`piece`、`drop_stream`、`control`）

## 需求理解摘要

**功能：** 从 NEXT 取得活动组，经移动、旋转、下落到锁定入盘的确定性 tick 状态机，以及出生失败判定。
**测试性质：** 新功能
**本轮范围：** 供给与出生、单个操控 tick 的动作顺序、旋转五步判定、锁定与分裂。
**Test Basis：**
- [Confirmed] [盘面与活动组操控](../../development/design/board-and-falling-group.md)：盘面几何、形状与颜色布局、时序参数与四个行为。
- [Confirmed] [玩法设计 §3.1、§3.2、§3.4](../../gameplay.md)：可见区与隐藏行、失败判定时机、掉落组与六个规则动作。
- [Confirmed] [Dropset](https://puyonexus.com/wiki/Dropset)：四种形状的颜色布局与每 16 手按 4 球单色手数奇偶互换 L/J。
**设计基线：** 时序参数录入 Puyo Puyo Tsu 的逆向工程帧数，`timing_source` 与 `reference_profile` 分开记录。
**关键假设：**
- 盘面为 6 列 × 14 行，`y = 0`、`y = 1` 为隐藏行。
- 旋转与推回不重置自然下落计时，因此不存在无限拖延落子的路径。
**待确认问题：**
- 时序参数为校准项（[DEC-002](../../development/decision/timing-parameter-source.md)）；校准后需同步更新以帧数为测试数据的用例。

## 测试点清单

### Component — Rules

- 两个角色各跑完至少两个 16 手周期，NEXT 与实际出生序列一致；4 球单色手数为奇数时第二个 16 手内 L 与 J 互换。
- I/L/J/O 的所有朝向都不穿墙、不穿盘面、不产生重复占格。
- 单色 `O` 的旋转输入循环换色且不改变占格。
- 旋转的五个分支各自可复现：目标格为空直接确认、隐藏行内竖直旋转被拒、对侧格为空时上推与侧推、夹在两列间的奇偶计数、确认时计数器重置到最近偶数。
- 出生列上格被占时不生成活动组并判负；该判定只在生成时刻发生。
- 锁定后失去支撑的球按分裂延迟与下落格数表自由落体。
- 锁定宽限累计 32 tick；按住软降立即锁定；上抬达到 8 次立即锁定。
- 自然下落 16 tick 每格、软降 2 tick 每格、横移输入重复 8/2 tick、横移冷却 1 tick。

### Component — Rules；Input

- 同一 tick 内横移、旋转、软降同时成立时按横移、旋转、软降的顺序生效；左右方向成立时软降不生效。
- 固定种子与动作日志得到相同的锁定坐标与 NEXT（Concern: Determinism）。

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
