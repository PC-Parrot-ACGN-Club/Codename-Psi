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
- `ClearCommit` 与 `ScanNext` 是零时长边界，停在 tick 边界上可观察；`ClearPreview` 与 `Gravity` 跨 tick 计时。
- 重力与分裂自由落体共用同一张下落格数时长表。
**待确认问题：**
- 消除预览时长与连锁重力参数为校准项（[DEC-004](../../development/decision/settlement-timing-values.md)）；校准后需同步更新以 tick 为测试数据的用例。

## 测试点清单

### Component — Rules

- 单组四连、多个同时成立的组、多色组与阈值以下的组（TC-001～TC-003）。
- 一颗、多颗以及被多个消除组共享的邻接垃圾按坐标去重清除（TC-004）。
- 隐藏行中的球不参与连通组、不被相邻垃圾规则清除；下方清空后落入可见区并参与后续连锁（TC-005～TC-006）。
- 重力产生二连锁以上时，逐步报告的组大小、颜色数与最终盘面正确（TC-007）。
- `ClearPreview` 到期前盘面仍包含待消球；`ClearCommit` 才产生攻击；`Gravity` 到期才提交目标盘面（TC-008～TC-010）。
- `ClearCommit` 与 `ScanNext` 停在 tick 边界上可观察，且都不为连锁步增加 tick（TC-015）。
- `Idle` 只由 `GroupLocked` 离开；推进 tick 不触发结算，结算进行中再次锁定不改变状态（TC-016）。
- 无连锁与全消等边界结果正确（TC-011～TC-012）。
- Fever 在结算过程中归零时完成当前连锁，并在 `Settlement` 退出（TC-013）。
- 推进 tick 的方式与是否存在表现消费者不改变规则阶段的 tick 序列（Concern: Determinism；TC-014）。
- 结算期间读模型给出的是结算自己持有的盘面，已提交的消除当场从中消失（TC-017）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 边界值分析 | 连通球数 3、4、5；零连锁步与全消两端 | TC-001、TC-011～TC-012 |
| 等价类划分 | 单组、多组同时、多色组；一颗、多颗与共享邻接垃圾 | TC-002～TC-004 |
| 状态迁移 | `Idle → ClearPreview → ClearCommit → Gravity → ScanNext → Settlement` 的守卫、提交时刻与两个零时长边界 | TC-008～TC-010、TC-013、TC-015～TC-016 |
| 场景法 | 二连锁的逐步事实与最终盘面 | TC-007 |
| 变形测试 | 推进方式与表现消费者变化时阶段 tick 序列保持一致 | TC-014 |
| 错误猜测 | 隐藏行的球对连通与垃圾清除的干扰，以及它是否被重力搬运 | TC-005～TC-006 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 连通球数达到阈值才形成消除组 | P0 | Component | — | Rules | 稳定盘面，可见区 `y=2..13`，`clear_threshold=4` | 参数化构造同色连通球数后触发扫描 | 3 颗横向连通；4 颗横向连通；5 颗 L 形连通 | 3 颗不形成消除组，直接形成零连锁步报告；4 颗与 5 颗各形成一个消除组，`ChainLinkFacts` 记录的组大小分别为 4 与 5 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除)；[玩法设计 §3.5](../../gameplay.md) |
| TC-002 | 同一连锁步内多个组同时成立时一并记录 | P1 | Component | — | Rules | 稳定盘面 | 构造同步成立的多个组后触发扫描 | 红 4 连于 `x=0..3, y=13`；蓝 4 连于 `x=0..3, y=11`；两组之间隔一行其它颜色 | 两组进入同一个 `ChainLinkFacts`，组大小列表为 `[4, 4]`、颜色数为 2、被清普通球为 8；不拆成两个连锁步 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除) |
| TC-003 | 多色同步消除时颜色数按实际颜色计 | P1 | Component | — | Rules | 稳定盘面 | 构造同一步内三种颜色各一组后触发扫描 | 红、蓝、绿各 4 连；固定坐标遍历顺序 | `ChainLinkFacts` 的颜色数为 3、组大小列表为 `[4, 4, 4]`；组列表次序由固定遍历顺序决定，重复扫描同一盘面得到同一次序 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除) |
| TC-004 | 相邻垃圾按坐标去重清除 | P0 | Component | — | Rules | 稳定盘面，含普通球与垃圾 | 参数化三种邻接关系后推进到 `ClearCommit` | 一颗垃圾邻接单个消除组；三颗垃圾分别邻接同一消除组；一颗垃圾同时邻接两个消除组 | 三组分别清除 1、3、1 颗垃圾；被多个组共享的垃圾只计一次且只清除一次；不与消除组四向相邻的垃圾保留 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除)；[Basic rules](https://puyonexus.com/wiki/Basic_rules) |
| TC-005 | 隐藏行的球不参与连通组也不被相邻垃圾规则清除 | P1 | Component | — | Rules | 两种构造都以支撑堆叠到隐藏行，盘面稳定 | 参数化两种构造后触发扫描 | 构造一：`x=0` 列 `y=2..4` 三颗同色、`y=1` 一颗同色，`y=5..13` 以垃圾支撑；构造二：`y=2` 的 `x=0..3` 四连、`x=0` 的 `y=1` 为垃圾，四列 `y=3..13` 以垃圾支撑 | 构造一不形成消除组、报告零连锁步，隐藏行那颗不补足连通数；构造二清除 4 连与其正下方相邻的 4 颗垃圾，被清坐标全部位于可见区，`y=1` 的垃圾不在其中 | [Confirmed] [连锁结算：数据模型](../../development/design/chain-resolution.md#数据模型)；[玩法设计 §3.1](../../gameplay.md) |
| TC-006 | 隐藏行的球在下方清空后落入可见区并参与后续连锁 | P1 | Component | — | Rules | 某列自底部堆叠到隐藏行 | 推进结算到 `Settlement` 并读取 `ChainReport` | `x=0` 列 `y=5..13` 九颗同色，`y=1..4` 四颗另一同色，其中 `y=1` 位于隐藏行 | 第 1 步清除 9 颗；重力把 `y=1..4` 四颗整体下移到 `y=10..13`，隐藏行那颗因此进入可见区；第 2 步这四颗连通达到阈值并消除；报告共 2 步、总清除 13 颗、`all_clear` 为真。隐藏行那颗若不被重力搬运，可见区只有 3 颗，第 2 步不成立 | [Confirmed] [盘面与活动组操控：盘面](../../development/design/board-and-falling-group.md#盘面)：落入可见区后才参与结算；[连锁结算：重力稳定](../../development/design/chain-resolution.md#重力稳定) |
| TC-007 | 重力形成二连锁时逐步事实与最终盘面正确 | P0 | Component | — | Rules | 稳定盘面，含可触发二连锁的构造 | 推进结算直到 `Settlement` 并读取 `ChainReport` | 蓝 4 连于 `(0,13)`～`(3,13)`；垃圾于 `(3,12)`；红于 `(0,12)`、`(1,12)`、`(2,12)`、`(3,11)` | 报告含 2 个连锁步：第 1 步清 4 蓝与 1 颗垃圾、组大小 `[4]`、颜色数 1；重力后 4 颗红落到 `y=13` 形成第 2 步、组大小 `[4]`、颜色数 1；最终盘面为空且 `all_clear` 为真；总清除普通球为 8 | [Confirmed] [连锁结算：扫描与清除](../../development/design/chain-resolution.md#扫描与清除)、[重力稳定](../../development/design/chain-resolution.md#重力稳定) |
| TC-008 | ClearPreview 到期前盘面仍包含待消球 | P1 | Component | — | Rules | 已进入 `ClearPreview` 的结算 | 逐 tick 推进并在每个 tick 读取盘面与阶段 | `clear_preview_ticks=24`；TC-007 的盘面 | 第 0～23 tick 的 `phase` 为 `ClearPreview` 且待消坐标仍是实体格，`elapsed_ticks` 逐 tick 加一；第 24 tick 才离开该阶段；期间不产生分数与攻击 | [Confirmed] [连锁结算：阶段时长](../../development/design/chain-resolution.md#阶段时长)；[玩法设计 §3.5](../../gameplay.md) |
| TC-009 | ClearCommit 在同一 tick 清空坐标并产生本步事实 | P0 | Component | — | Rules | 同 TC-008，推进到预览到期 | 推进到 `ClearCommit` 的那一 tick 并读取盘面、事实与阶段 | 同 TC-008 | 该 tick 内待消坐标全部变空，并产生本连锁步的分数、攻击与清除事实各一次；该 tick 停在 `ClearCommit` 边界上，其携带的连锁步编号可读；下一 tick 离开该边界并同时为 `Gravity` 计入第 1 tick | [Confirmed] [连锁结算：数据模型](../../development/design/chain-resolution.md#数据模型)、[扫描与清除](../../development/design/chain-resolution.md#扫描与清除) |
| TC-010 | Gravity 到期才原子提交目标盘面 | P1 | Component | — | Rules | 已进入 `Gravity` 的结算 | 逐 tick 推进并读取盘面与阶段进度 | 本轮最大下落格数 2，查表得 `duration_ticks=15` | 第 0～14 tick 读到的仍是重力前盘面，只有起终点与阶段进度可读；第 15 tick 一次性提交目标盘面；提交后不存在空格下方仍有实体格的悬空状态 | [Confirmed] [连锁结算：阶段时长](../../development/design/chain-resolution.md#阶段时长)、[重力稳定](../../development/design/chain-resolution.md#重力稳定) |
| TC-011 | 未触发连锁时直接形成零连锁步报告 | P1 | Component | — | Rules | 稳定盘面，锁定后不存在达到阈值的组 | 触发一次结算并读取报告 | 锁定后最大同色连通数为 3 | 不进入 `ClearPreview`，直接形成 `ChainReport`；连锁步数为 0、总清除数为 0；`all_clear` 为假；报告形成后不可变 | [Confirmed] [连锁结算：完成](../../development/design/chain-resolution.md#完成) |
| TC-012 | 可见区被清空时报告 all_clear | P1 | Component | — | Rules | 可见区只余一组可消球 | 推进结算到 `Settlement` 并读取 `FieldFacts` | 空盘上仅有红 4 连于 `y=13` | 结算完成后可见区无实体格、`all_clear` 为真；`FieldFacts` 不直接给出奖励或胜负结论 | [Confirmed] [连锁结算：完成](../../development/design/chain-resolution.md#完成) |
| TC-013 | 结算过程中 Fever 归零时完成当前连锁并在 Settlement 退出 | P1 | Component | — | Rules | 处于 Fever 且结算已进入第 1 步 `ClearPreview` | 在结算中途把 `fever_time_ticks` 推进到 0，继续推进到 `Settlement` | 二连锁盘面（同 TC-007）；在第 1 步 `Gravity` 期间归零 | 归零不改变后续阶段的 tick 序列，第 2 步照常完成；退出标记在归零 tick 记下，实际退出发生在 `Settlement`；`ChainReport` 含完整 2 步 | [Confirmed] [连锁结算：协作](../../development/design/chain-resolution.md#协作)；[玩法设计 §3.5、§5.2](../../gameplay.md) |
| TC-014 | 推进方式与表现消费者不改变阶段 tick 序列 | P0 | Component | Determinism | Rules | 三份相同初始盘面与结算状态 | 分别以一次推进 90 tick、分三段各 30 tick、以及每 tick 读取表现协议字段的方式推进 | 二连锁盘面（同 TC-007）；`clear_preview_ticks=24`；重力时长查表 | 三者逐 tick 的 `phase`、`elapsed_ticks`、`duration_ticks` 序列完全相同，最终盘面与 `ChainReport` 相同；读取表现字段不推进阶段，也没有由表现返回的完成信号可改变截止 tick | [Confirmed] [连锁结算：表现协议](../../development/design/chain-resolution.md#表现协议)；[玩法设计 §3.5](../../gameplay.md) |
| TC-015 | 两个零时长边界都不为连锁步增加 tick | P1 | Component | — | Rules | 已冻结的结算时长配置与一条二连锁盘面 | 读取剖面时长；逐 tick 推进并记录每个 tick 的阶段标签 | `clear_preview_ticks=24`；重力表 1 格 20、2 格 27、3 格 33；TC-007 的盘面（本轮最大下落 2 格） | `24 + 20 = 44` 与 `24 + 33 = 57` 与 [DEC-004](../../development/decision/settlement-timing-values.md) 记录的连锁步区间一致；本盘面第 1 步为 `24 + 27 = 51` tick，第 24 tick 停在 `ClearCommit`、第 51 tick 停在 `ScanNext`，第 52 tick 已在为第 2 步的 `ClearPreview` 计入第 1 tick | [Confirmed] [连锁结算：阶段时长](../../development/design/chain-resolution.md#阶段时长)、[数据模型](../../development/design/chain-resolution.md#数据模型) |
| TC-016 | Idle 只由锁定离开，且结算中再次锁定不改变状态 | P1 | Component | — | Rules | 一个静息结算持有稳定盘面 | 先推进若干 tick，再施加锁定；结算进行中再施加一次锁定；与「锁定后直接构造」的结果比较 | 盘面为 `y=13` 的 `x=0..3` 四连；空推进 30 tick | 30 tick 内 `phase` 恒为 `Idle`、盘面不变、无 `ChainReport`；锁定后进入 `ClearPreview` 且 `elapsed_ticks=0`；结算进行中的第二次锁定不改变 `phase`；两条路径推进到 `Settlement` 得到相同报告 | [Confirmed] [连锁结算：数据模型](../../development/design/chain-resolution.md#数据模型)、[协作](../../development/design/chain-resolution.md#协作)：`GroupLocked` 触发一次结算 |
| TC-017 | 结算期间读模型给出结算自己的盘面，已提交的消除当场消失 | P1 | Component Integration | — | Rules；Client | 一名玩家的盘面上有一个待触发的消除组，且已进入结算 | 逐 tick 推进整条连锁，每个 tick 读取读模型的盘面 | 底行四连；随包剖面 | `ClearPreview` 期间待消球仍在读模型的盘面上；该链提交的那个 tick，这些坐标当场为空且盘面上的球数正好少了这一链清掉的数量；其后直到结算结束球数不再回升 | [Confirmed] [连锁结算：表现协议](../../development/design/chain-resolution.md#表现协议) |

## 风险查漏

消除阈值、多组与多色、垃圾去重、隐藏行的两项排除与其被重力搬运、二连锁逐步事实、静息态与其锁定触发、两个计时阶段的提交时刻、两个零时长边界的可观察性与不计时、零连锁与全消两端均有直接用例；阶段时序的确定性由 TC-014 覆盖推进方式与表现消费者两个变量。
