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
- [Confirmed] [Fever (rule)](https://puyonexus.com/wiki/Fever_%28rule%29)：Fever 1/2 主机/PC 列的计时、题面下限与翻盘行为。
**设计基线：** Fever 时间是玩家级持久值，冻结的队列不落垃圾但可被抵消。
**关键假设：**
- 题面按等级独立无重复袋选择，袋状态属于规则状态。
- 归零翻盘的立即落下遵守单次上限。
**待确认问题：**
- 无。

## 测试点清单

### Component — Rules

- 量表从 0 填满进入 Fever；一个安全点最多增加一格；时间不超过上限（TC-001～TC-003）。
- 不在 Fever 时全消也能累加 Fever 时间并 clamp 到上限（TC-004）。
- 时间奖励归被抵消的攻击方，抵消方不获得时间（TC-005）。
- 时间奖励在最后一个连锁步开始消除动画的 tick 发放（TC-006）。
- 普通盘全消立即在普通盘上装载预设 4 连题面；三种全消组合各自的效果正确（TC-007～TC-008）。
- 等级域内各目标等级的题面都能加载且合法（Concern: Content Validation；TC-009）。
- 每个等级的无重复袋取尽后重新装填，同一袋内题面不重复（TC-010）。
- 达标、全消、差 1、差 2、差 3 及以上五个分支的下一等级正确（TC-011～TC-012）。
- 双盘与双队列的冻结、合并与归零后立即落下（TC-013～TC-015）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 边界值分析 | 量表 6/7 格、时间上限 1800 tick、等级域 3 与 15 两端 | TC-001、TC-003～TC-004、TC-012 |
| 判定表 | 五个题面结果分支；三种全消组合；奖励归属的攻守两侧 | TC-005、TC-007～TC-008、TC-011 |
| 状态迁移 | `Normal → Fever → Normal` 的进入、冻结、合并与退出守卫 | TC-001、TC-013～TC-015 |
| 等价类划分 | 等级域内每个目标等级的题面加载 | TC-009 |
| 不变量检查 | 一个安全点最多加一格；同一袋内不重复；奖励只在指定 tick 发放一次 | TC-002、TC-006、TC-010 |
| 错误猜测 | 归零瞬间仍有未抵消垃圾、非活动通道被抵消 | TC-013、TC-015 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 量表填满后在安全点进入 Fever 并冻结普通通道 | P0 | Component | — | Rules | 玩家处于普通盘，量表容量 7，`fever_time_ticks` 非零 | 连续在安全点提交有效连锁抵消事实，并在第 6、7 格时读取状态 | 量表容量 7；起始 0 格；`fever_time_ticks=1200`（20 秒） | 第 6 格时仍在普通盘、未产生 `FeverEntered`；第 7 格所在安全点切换 `active_channel` 为 Fever、量表重置为 0、普通通道被冻结并保留其盘面与队列、按 `fever_time_ticks` 起算会话并装载首个题面 | [Confirmed] [Fever 循环：进入 Fever](../../development/design/fever-mode.md#进入-fever)；[玩法设计 §5.1](../../gameplay.md) |
| TC-002 | 一个安全点最多为量表增加一格 | P1 | Component | — | Rules | 量表为 0 | 在同一个安全点提交含多次抵消的事实集合 | 同一安全点内 3 次有效连锁抵消 | 量表只增加到 1 格；量表满后继续提交抵消事实不再增加，保持容量值直到进入 Fever 时被重置 | [Confirmed] [Fever 循环：量表与时间奖励](../../development/design/fever-mode.md#量表与时间奖励)；[玩法设计 §5.1](../../gameplay.md) |
| TC-003 | Fever 时间累加不超过剖面声明的上限 | P1 | Component | — | Rules | 处于 Fever，时间接近上限 | 提交一次 Fever 内全消并读取时间 | `fever_time_ticks=1750`；上限 1800 tick（30 秒）；全消奖励 +300 tick | 时间被 clamp 到 1800 而非 2050；显示秒数为向下取整的 30；clamp 后不再因同一奖励重复累加 | [Confirmed] [Fever 循环：数据模型](../../development/design/fever-mode.md#数据模型)、[量表与时间奖励](../../development/design/fever-mode.md#量表与时间奖励) |
| TC-004 | 不在 Fever 时全消同样累加玩家级 Fever 时间 | P1 | Component | — | Rules | 玩家处于普通盘且未进入过 Fever | 参数化两组起始时间各提交一次普通盘全消 | 起始 1000 tick 与 1750 tick；全消奖励 +300 tick；上限 1800 tick | 第一组得 1300、第二组被 clamp 到 1800；两组的时间都保存在玩家级字段上，不随会话创建或销毁而重置 | [Confirmed] [Fever 循环：数据模型](../../development/design/fever-mode.md#数据模型)；[玩法设计 §5.1、§5.3](../../gameplay.md) |
| TC-005 | 时间奖励归被抵消的攻击方而非抵消方 | P0 | Component | — | Rules | 两名玩家的 `fever_time_ticks` 均已知 | 提交一次抵消事实，其中一方的连锁被另一方抵消 | 攻击方与抵消方起始时间均为 600 tick；奖励 +60 tick（1 秒） | 攻击方时间变为 660、抵消方保持 600；奖励只依据 `OffsetFacts` 中的攻守双方字段判定，不依据谁先进入安全点 | [Confirmed] [Fever 循环：量表与时间奖励](../../development/design/fever-mode.md#量表与时间奖励)；[玩法设计 §5.1](../../gameplay.md) |
| TC-006 | Fever 中的时间奖励在最后一个连锁步开始消除动画的 tick 发放 | P1 | Component | — | Rules | 处于 Fever，正在结算一条三步连锁 | 逐 tick 推进结算，在每个 tick 读取 `fever_time_ticks` | 三步连锁；奖励 +60 tick；第 3 步进入 `ClearPreview` 的 tick 记为 `t` | 时间在 `t` 增加 60，`t` 之前的任一 tick 都未增加；`Settlement` 到达时不再重复发放；一条连锁只发放一次 | [Confirmed] [Fever 循环：量表与时间奖励](../../development/design/fever-mode.md#量表与时间奖励)；[玩法设计 §5.2](../../gameplay.md) |
| TC-007 | 普通盘全消立即装载预设 4 连题面并加 5 秒 | P1 | Component | — | Rules | 玩家处于普通盘，`fever_time_ticks` 未达上限 | 触发一次使可见区清空的落子并推进到安全点 | 起始 `fever_time_ticks=600`；预设题面目标连锁 4 | 普通盘上立即装载预设 4 连题面，活动通道仍为普通盘、量表不因此增加；时间变为 900；本次不进入 Fever | [Confirmed] [Fever 循环：全消](../../development/design/fever-mode.md#全消)；[玩法设计 §5.3](../../gameplay.md) |
| TC-008 | Fever 内全消与全消同时进场的目标等级与时间效果 | P1 | Component | — | Rules | 可分别构造 Fever 内全消与全消同时量表填满两种局面 | 参数化两组局面各推进到安全点 | 组一：Fever 内目标 10、实际打出 10 且全消；组二：普通盘全消且该安全点量表填满，首题基准等级 6 | 组一下一题目标为 12（`actual + 2`）、时间 +300；组二首个 Fever 题面目标为 8（基准 +2）、时间 +300；两组的多个效果由同一张优先级表一次结算，改变系统执行顺序不改变结果 | [Confirmed] [Fever 循环：全消](../../development/design/fever-mode.md#全消)；[玩法设计 §5.3](../../gameplay.md) |
| TC-009 | 等级域内每个目标等级的题面都能加载且合法 | P1 | Component | Content Validation | Rules | 题面书覆盖剖面声明的全部目标等级 | 遍历等级域，对每个等级各装载一次题面并校验盘面 | 等级域 `3..=15`（13 个等级） | 每个等级都能取出题面并装载到 Fever 盘；装载后的盘面坐标落在可见区内、无悬空格、目标连锁与该等级一致；无任一等级取不到题面 | [Confirmed] [Fever 循环：题面循环](../../development/design/fever-mode.md#题面循环)；[规则配置：数据模型](../../development/design/rule-configuration.md#数据模型) |
| TC-010 | 每等级的无重复袋取尽后重新装填 | P2 | Component | — | Rules | 某等级的题面集合大小已知 | 在同一等级上连续取出 `n + 1` 个题面 | 等级 10 的题面集合大小 `n=5`；`fever-puzzle` 流固定种子 | 前 `n` 次取出的题面 id 两两不同、恰好覆盖该等级的全部题面；第 `n + 1` 次触发重新装填并再次从完整集合中取出；袋状态随取用推进且属于规则状态 | [Confirmed] [Fever 循环：数据模型](../../development/design/fever-mode.md#数据模型) |
| TC-011 | 五个题面结果分支给出正确的下一等级 | P0 | Component | — | Rules | 处于 Fever，当前目标等级已知 | 参数化提交五种 `ChainReport` 结果并读取下一目标等级 | 目标 10；实际依次为 10（达标）、10 且全消、9（差 1）、8（差 2）、7（差 3） | 下一目标依次为 11、12、10、7、5；每个分支只由实际连锁与目标的关系决定，与本步分数与攻击无关 | [Confirmed] [Fever 循环：题面循环](../../development/design/fever-mode.md#题面循环)；[玩法设计 §5.2](../../gameplay.md) |
| TC-012 | 升降级结果 clamp 到等级域两端 | P1 | Component | — | Rules | 处于 Fever，等级域 `3..=15` | 参数化提交两种触及边界的结果 | 目标 15、实际 15（达标，原式给出 16）；目标 3、实际 0（差 3 以上，原式给出 −2） | 两组的下一目标分别 clamp 为 15 与 3；clamp 后仍能正常取出该等级的题面 | [Confirmed] [Fever 循环：题面循环](../../development/design/fever-mode.md#题面循环)；[玩法设计 §5.2](../../gameplay.md) |
| TC-013 | 非活动通道冻结时不落垃圾但仍可被抵消 | P1 | Component | — | Rules | 处于 Fever，两个通道的队列均非零 | 在 Fever 中完成一次未触发连锁的落子，再提交一次超过活动通道队列量的抵消 | 普通通道队列 8、Fever 通道队列 4；抵消攻击量 7 | 未触发连锁的落子只把垃圾落到 Fever 盘，普通盘不落下任何垃圾且盘面不变；抵消先清空 Fever 通道的 4，再从普通通道扣 3，普通通道剩 5 | [Confirmed] [Fever 循环：数据模型](../../development/design/fever-mode.md#数据模型)；[玩法设计 §5.2](../../gameplay.md) |
| TC-014 | 退出 Fever 时合并队列并恢复普通通道 | P0 | Component | — | Rules | 处于 Fever，时间即将归零，两个通道队列非零 | 推进到时间归零，再推进已开始的结算到 `Settlement` | 普通通道队列 5、Fever 通道队列 3；归零时正处于连锁第 2 步 | 归零 tick 记下退出待处理但不中断结算；`Settlement` 安全点产生 `FeverExited`；普通通道队列变为 8、Fever 通道队列清零、`active_channel` 恢复为普通盘；Fever 会话与其列序状态被丢弃 | [Confirmed] [Fever 循环：退出 Fever](../../development/design/fever-mode.md#退出-fever)；[玩法设计 §5.2](../../gameplay.md) |
| TC-015 | 归零翻回后的立即落下遵守单次上限 | P1 | Component | — | Rules | 归零瞬间未能抵消且合并后队列超过单次上限 | 完成退出并观察翻回后的第一次落下 | 合并后队列 35；单次上限 30 | 翻回普通盘后立即触发一次落下，落下 30 颗，队列剩 5；余量留在队列等待后续落下判定；该次落下不因“立即”而绕过上限 | [Confirmed] [Fever 循环：退出 Fever](../../development/design/fever-mode.md#退出-fever)；[玩法设计 §5.2](../../gameplay.md) |

## 风险查漏

量表的填充上限与进入时刻、时间的三个来源与 clamp、奖励归属与发放 tick、三种全消组合、等级域覆盖与袋语义、五个升降级分支与两端 clamp、冻结通道的两条语义、退出合并与立即落下均有直接用例；跨玩家的奖励来源构造见[小局、BO3 与安全点](../integration-system/match-and-round.md)。
