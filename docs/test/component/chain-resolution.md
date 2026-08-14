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

- 单组四连、多个同时成立的组、多色组与阈值以下的组（TC-001～TC-003）。
- 一颗、多颗以及被多个消除组共享的邻接垃圾按坐标去重清除（TC-004）。
- 隐藏行中的球不参与连通组、不被相邻垃圾规则清除、不阻止 `all_clear`（TC-005～TC-006）。
- 重力产生二连锁以上时，逐步报告的组大小、颜色数与最终盘面正确（TC-007）。
- `ClearPreview` 到期前盘面仍包含待消球；`ClearCommit` 才产生攻击；`Gravity` 到期才提交目标盘面（TC-008～TC-010）。
- 无连锁与全消等边界结果正确（TC-011～TC-012）。
- Fever 在结算过程中归零时完成当前连锁，并在 `Settlement` 退出（TC-013）。
- 推进 tick 的方式与是否存在表现消费者不改变规则阶段的 tick 序列（Concern: Determinism；TC-014）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 边界值分析 | 连通球数 3、4、5；零连锁步与全消两端 | TC-001、TC-011～TC-012 |
| 等价类划分 | 单组、多组同时、多色组；一颗、多颗与共享邻接垃圾 | TC-002～TC-004 |
| 状态迁移 | `Idle → ClearPreview → ClearCommit → Gravity → ScanNext → Settlement` 的守卫与提交时刻 | TC-008～TC-010、TC-013 |
| 场景法 | 二连锁的逐步事实与最终盘面 | TC-007 |
| 变形测试 | 推进方式与表现消费者变化时阶段 tick 序列保持一致 | TC-014 |
| 错误猜测 | 隐藏行残留对连通、垃圾清除与 `all_clear` 的干扰 | TC-005～TC-006 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 连通球数达到阈值才形成消除组 | P0 | Component | — | Rules | 稳定盘面，可见区 `y=2..13`，`clear_threshold=4` | 参数化构造同色连通球数后触发扫描 | 3 颗横向连通；4 颗横向连通；5 颗 L 形连通 | 3 颗不形成消除组，直接形成零连锁步报告；4 颗与 5 颗各形成一个消除组，`ChainLinkFacts` 记录的组大小分别为 4 与 5 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除)；[玩法设计 §3.5](../../gameplay.md) |
| TC-002 | 同一连锁步内多个组同时成立时一并记录 | P1 | Component | — | Rules | 稳定盘面 | 构造同步成立的多个组后触发扫描 | 红 4 连于 `x=0..3, y=13`；蓝 4 连于 `x=0..3, y=11`；两组之间隔一行其它颜色 | 两组进入同一个 `ChainLinkFacts`，组大小列表为 `[4, 4]`、颜色数为 2、被清普通球为 8；不拆成两个连锁步 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除) |
| TC-003 | 多色同步消除时颜色数按实际颜色计 | P1 | Component | — | Rules | 稳定盘面 | 构造同一步内三种颜色各一组后触发扫描 | 红、蓝、绿各 4 连；固定坐标遍历顺序 | `ChainLinkFacts` 的颜色数为 3、组大小列表为 `[4, 4, 4]`；组列表次序由固定遍历顺序决定，重复扫描同一盘面得到同一次序 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除) |
| TC-004 | 相邻垃圾按坐标去重清除 | P0 | Component | — | Rules | 稳定盘面，含普通球与垃圾 | 参数化三种邻接关系后推进到 `ClearCommit` | 一颗垃圾邻接单个消除组；三颗垃圾分别邻接同一消除组；一颗垃圾同时邻接两个消除组 | 三组分别清除 1、3、1 颗垃圾；被多个组共享的垃圾只计一次且只清除一次；不与消除组四向相邻的垃圾保留 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除)；[Basic rules](https://puyonexus.com/wiki/Basic_rules) |
| TC-005 | 隐藏行的球不参与连通组也不被相邻垃圾规则清除 | P1 | Component | — | Rules | 盘面在 `y=0`、`y=1` 与可见区顶部均有球 | 参数化两种构造后触发扫描 | 构造一：同色球 3 颗在 `y=2` 且 1 颗在 `y=1`；构造二：可见区 4 连正上方 `y=1` 为垃圾 | 构造一不形成消除组，隐藏行那颗不补足连通数；构造二清除可见区 4 连但保留 `y=1` 的垃圾；两种构造下隐藏行的球始终留在原坐标 | [Confirmed] [连锁结算：数据模型](../../development/design/chain-resolution.md#数据模型)；[玩法设计 §3.1](../../gameplay.md) |
| TC-006 | 隐藏行有残留时可见区清空仍成立 all_clear | P1 | Component | — | Rules | 结算前盘面在隐藏行留有球 | 触发使可见区全部清空的结算并读取 `FieldFacts` | 可见区仅一组 4 连；`y=1` 保留 1 颗普通球与 1 颗垃圾 | 结算完成后 `all_clear` 为真；隐藏行的两颗球保持在原坐标，不因 `all_clear` 被清除 | [Confirmed] [连锁结算：数据模型](../../development/design/chain-resolution.md#数据模型) |
| TC-007 | 重力形成二连锁时逐步事实与最终盘面正确 | P0 | Component | — | Rules | 稳定盘面，含可触发二连锁的构造 | 推进结算直到 `Settlement` 并读取 `ChainReport` | 蓝 4 连于 `(0,13)`～`(3,13)`；垃圾于 `(3,12)`；红于 `(0,12)`、`(1,12)`、`(2,12)`、`(3,11)` | 报告含 2 个连锁步：第 1 步清 4 蓝与 1 颗垃圾、组大小 `[4]`、颜色数 1；重力后 4 颗红落到 `y=13` 形成第 2 步、组大小 `[4]`、颜色数 1；最终盘面为空且 `all_clear` 为真；总清除普通球为 8 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除)、[重力稳定](../../development/design/chain-resolution.md#重力稳定) |
| TC-008 | ClearPreview 到期前盘面仍包含待消球 | P1 | Component | — | Rules | 已进入 `ClearPreview` 的结算 | 逐 tick 推进并在每个 tick 读取盘面与阶段 | `clear_preview_ticks=12`；TC-007 的盘面 | 第 0～11 tick 的 `phase` 为 `ClearPreview` 且待消坐标仍是实体格，`elapsed_ticks` 逐 tick 加一；第 12 tick 才离开该阶段；期间不产生分数与攻击 | [Confirmed] [连锁结算：阶段时长](../../development/design/chain-resolution.md#阶段时长)；[玩法设计 §3.5](../../gameplay.md) |
| TC-009 | ClearCommit 在同一 tick 清空坐标并产生本步事实 | P0 | Component | — | Rules | 同 TC-008，推进到预览到期 | 推进到 `ClearCommit` 的那一 tick 并读取盘面、事实与后继阶段 | 同 TC-008 | 该 tick 内待消坐标全部变空，并产生本连锁步的分数、攻击与清除事实各一次；`ClearCommit` 不占用额外 tick，同一 tick 内已进入 `Gravity` | [Confirmed] [连锁结算：数据模型](../../development/design/chain-resolution.md#数据模型)、[扫描与清除](../../development/design/chain-resolution.md#扫描与清除) |
| TC-010 | Gravity 到期才原子提交目标盘面 | P1 | Component | — | Rules | 已进入 `Gravity` 的结算 | 逐 tick 推进并读取盘面与阶段进度 | 本轮最大下落格数 2，查表得 `duration_ticks=15` | 第 0～14 tick 读到的仍是重力前盘面，只有起终点与阶段进度可读；第 15 tick 一次性提交目标盘面；提交后不存在空格下方仍有实体格的悬空状态 | [Confirmed] [连锁结算：阶段时长](../../development/design/chain-resolution.md#阶段时长)、[重力稳定](../../development/design/chain-resolution.md#重力稳定) |
| TC-011 | 未触发连锁时直接形成零连锁步报告 | P1 | Component | — | Rules | 稳定盘面，锁定后不存在达到阈值的组 | 触发一次结算并读取报告 | 锁定后最大同色连通数为 3 | 不进入 `ClearPreview`，直接形成 `ChainReport`；连锁步数为 0、总清除数为 0；`all_clear` 为假；报告形成后不可变 | [Confirmed] [连锁结算：完成](../../development/design/chain-resolution.md#完成) |
| TC-012 | 可见区被清空时报告 all_clear | P1 | Component | — | Rules | 可见区只余一组可消球 | 推进结算到 `Settlement` 并读取 `FieldFacts` | 空盘上仅有红 4 连于 `y=13` | 结算完成后可见区无实体格、`all_clear` 为真；`FieldFacts` 不直接给出奖励或胜负结论 | [Confirmed] [连锁结算：完成](../../development/design/chain-resolution.md#完成) |
| TC-013 | 结算过程中 Fever 归零时完成当前连锁并在 Settlement 退出 | P1 | Component | — | Rules | 处于 Fever 且结算已进入第 1 步 `ClearPreview` | 在结算中途把 `fever_time_ticks` 推进到 0，继续推进到 `Settlement` | 二连锁盘面（同 TC-007）；在第 1 步 `Gravity` 期间归零 | 归零不改变后续阶段的 tick 序列，第 2 步照常完成；退出标记在归零 tick 记下，实际退出发生在 `Settlement`；`ChainReport` 含完整 2 步 | [Confirmed] [连锁结算：协作](../../development/design/chain-resolution.md#协作)；[玩法设计 §3.5、§5.2](../../gameplay.md) |
| TC-014 | 推进方式与表现消费者不改变阶段 tick 序列 | P0 | Component | Determinism | Rules | 三份相同初始盘面与结算状态 | 分别以一次推进 60 tick、分三段各 20 tick、以及每 tick 读取表现协议字段的方式推进 | 二连锁盘面（同 TC-007）；`clear_preview_ticks=12`；重力时长查表 | 三者逐 tick 的 `phase`、`elapsed_ticks`、`duration_ticks` 序列完全相同，最终盘面与 `ChainReport` 相同；读取表现字段不推进阶段，也没有由表现返回的完成信号可改变截止 tick | [Confirmed] [连锁结算：表现协议](../../development/design/chain-resolution.md#表现协议)；[玩法设计 §3.5](../../gameplay.md) |

## 风险查漏

消除阈值、多组与多色、垃圾去重、隐藏行三项排除、二连锁逐步事实、三个阶段的提交时刻、零连锁与全消两端均有直接用例；阶段时序的确定性由 TC-014 覆盖推进方式与表现消费者两个变量。
