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

- 随机流派生与前 N 个输出的 golden vectors（Concern: Determinism；TC-001）。
- 未注册的流名不可派生（TC-002）。
- 两名玩家的同名流互不相同且互不影响（Concern: Determinism；TC-003）。

### Component Integration — Rules；Match Flow

- 同一验证日志重复运行、以及在不同进程中运行，得到相同的 checkpoint 校验和（Concern: Determinism；TC-004～TC-005）。
- 含回滚的千 tick 连续推进后校验和与无回滚推进一致（Concern: Determinism；TC-006）。
- 在活动组、连锁中间、垃圾下落、Fever 中与局间五个阶段分别快照并恢复后继续推进，状态收敛（Concern: Determinism；TC-007）。
- 从同一快照分叉不同输入使校验和分叉；恢复相同输入后再次收敛（Concern: Determinism；TC-008）。
- 单个持久字段变化能改变校验和；事件队列的消费方式不改变校验和（Concern: Determinism；TC-009～TC-010）。
- 摘要、schema 版本或算法版本不匹配时拒绝恢复并返回 typed error（TC-011）。
- 任意结算阶段与任意 Fever tick 的快照恢复后，连锁报告、题面选择与剩余时间一致（Concern: Determinism；TC-012）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 变形测试 | 重复运行、跨进程、分段推进与回滚下的校验和关系 | TC-004～TC-008 |
| 性质测试 | 两名玩家的流互不影响；相同初始状态与输入序列导出相同校验和；快照恢复后状态收敛 | TC-003、TC-006～TC-008、TC-012 |
| 等价类划分 | 快照覆盖表的六个领域各取代表字段；三类版本不匹配 | TC-009、TC-011 |
| 边界值分析 | 快照点取阶段首 tick、末 tick 与阶段边界 | TC-007、TC-012 |
| 错误猜测 | 事件消费方式、未注册流名、遗漏的持久字段 | TC-002、TC-009～TC-010 |
| Characterization Test | 随机流前 N 个输出作为版本内锁定的 golden vector | TC-001 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 命名流的派生结果在算法版本内被 golden vector 锁定 | P0 | Component | Determinism | Rules | 派生函数与三个命名流可用 | 按固定派生键取出每个流的前 N 个输出并与已锁定的 golden vector 比较 | 根种子 `0x1`；`round_index=0`；`draw_attempt=0`；`player_slot=0`；流名 `color`、`nuisance`、`fever-puzzle`；每流前 32 个输出 | 三个流的前 32 个输出与 golden vector 逐项相等；同一派生键重复派生得到同一序列；输出变化时必须同时提高随机算法版本，否则用例失败 | [Confirmed] [确定性与快照：派生随机流](../../development/design/determinism-and-snapshot.md#派生随机流)、[随机流](../../development/design/determinism-and-snapshot.md#随机流) |
| TC-002 | 未注册的流名不可派生 | P2 | Component | — | Rules | 派生函数只接受已注册流名 | 以未注册流名调用派生 | 流名 `unknown-stream` | 返回 typed error，不产生流对象；已注册的三个流名不受影响，仍可正常派生 | [Confirmed] [确定性与快照：派生随机流](../../development/design/determinism-and-snapshot.md#派生随机流) |
| TC-003 | 两名玩家的同名流互不相同且互不影响 | P1 | Component | Determinism | Rules | 同一根种子、局号与重打次数 | 分别为两个 slot 派生同名流，各取前 32 个输出；再推进 slot 0 的流 100 次后读取 slot 1 的下一个输出 | `player_slot=0` 与 `player_slot=1`；流名 `color` | 两名玩家的前 32 个输出序列不相同；推进 slot 0 的流不改变 slot 1 的流位置与后续输出；派生键包含 `player_slot` | [Confirmed] [DEC-005](../../development/decision/color-sequence-derivation.md)；[确定性与快照：随机流](../../development/design/determinism-and-snapshot.md#随机流) |
| TC-004 | 同一验证日志重复运行得到相同的 checkpoint 校验和 | P0 | Component Integration | Determinism | Rules；Match Flow | 一份 `VerificationLog`，含根摘要、算法版本、根种子、角色与逐 tick 输入 | 在同一进程内连续运行同一份日志两次并比较每个 checkpoint | 1000 tick 输入日志；每 100 tick 一个 checkpoint，共 10 个 | 两次运行的 10 个 checkpoint 校验和逐项相等，关键读模型也相等；运行过程无窗口且不读取文件系统与墙钟 | [Confirmed] [确定性与快照：运行验证日志](../../development/design/determinism-and-snapshot.md#运行验证日志)；[TDD §3](../../TDD.md) |
| TC-005 | 同一验证日志在不同进程运行得到相同校验和 | P1 | Component Integration | Determinism | Rules；Match Flow | 同 TC-004 | 在两个独立进程中各运行一次同一份日志并比较结果 | 同 TC-004；两个进程使用不同的容器遍历与线程调度时序 | 两个进程的 10 个 checkpoint 校验和逐项相等；结果不随哈希顺序、线程调度或分配地址变化 | [Confirmed] [确定性与快照：计算状态校验和](../../development/design/determinism-and-snapshot.md#计算状态校验和)、[确定性约束](../../development/design/determinism-and-snapshot.md#确定性约束) |
| TC-006 | 含回滚的千 tick 推进与直推得到相同校验和 | P0 | Component Integration | Determinism | Rules；Match Flow | 两份相同初始 `MatchState` | 一份直推 1000 tick；另一份每 50 tick 快照一次并回滚重推 10 tick 后继续，直到同样推进 1000 tick | 相同的 1000 tick 输入日志；回滚点 20 个 | 两份状态在第 1000 tick 的校验和相等，读模型逐项相等；回滚重推不产生累积偏差 | [Confirmed] [TDD §6](../../TDD.md)；[确定性与快照：快照与恢复](../../development/design/determinism-and-snapshot.md#快照与恢复) |
| TC-007 | 五个阶段的快照恢复后继续推进状态收敛 | P0 | Component Integration | Determinism | Rules；Match Flow | 可在指定阶段停下并快照 | 参数化五个快照点各执行「快照 → 恢复 → 继续推进 200 tick」，与不快照直推的基准比较 | 快照点：持有活动组时、连锁第 2 步 `Gravity` 中、垃圾落下的自由落体中、Fever 中、`RoundOutro` 局间 | 五组恢复后的推进结果与基准的校验和逐项相等；两个通道的队列与列序位置、玩家级 Fever 时间、每等级袋状态、`DropCursor` 与 L/J 周期状态在恢复后与基准相同 | [Confirmed] [确定性与快照：快照](../../development/design/determinism-and-snapshot.md#快照)、[快照与恢复](../../development/design/determinism-and-snapshot.md#快照与恢复) |
| TC-008 | 同一快照在不同输入下分叉并在相同输入下再收敛 | P1 | Component Integration | Determinism | Rules；Match Flow | 一份 `MatchSnapshot` | 从同一快照恢复出两份状态，先喂入不同输入 20 tick，再喂入相同输入 100 tick | 分叉段：一方持续 `Left`、另一方持续 `Right`；收敛段：相同的 100 tick 日志 | 分叉段结束时两者校验和不同；收敛段结束时两者校验和仍不同（状态已分叉且不会自行合流）；把第二份重新从原快照恢复并喂入与第一份完全相同的 120 tick 输入后，两者校验和相等 | [Confirmed] [确定性与快照：快照与恢复](../../development/design/determinism-and-snapshot.md#快照与恢复)、[计算状态校验和](../../development/design/determinism-and-snapshot.md#计算状态校验和) |
| TC-009 | 快照覆盖表的每个持久字段变化都改变校验和 | P1 | Component Integration | Determinism | Rules；Match Flow | 一份已推进的 `MatchState` 及其校验和 | 参数化只改动一个持久字段后重新计算校验和 | 覆盖六个领域各取一项：`round_index`；`DropCursor` 位置；`Gravity` 的未提交目标盘面；某通道的待接收垃圾整数与列序位置；玩家级 Fever 时间与某等级的袋状态；某命名流的流位置 | 每一项改动都使校验和与基准不同；把改动还原后校验和回到基准值；不存在改动后校验和不变的持久字段 | [Confirmed] [确定性与快照：快照](../../development/design/determinism-and-snapshot.md#快照)、[状态编码与校验和](../../development/design/determinism-and-snapshot.md#状态编码与校验和) |
| TC-010 | 规则事件的消费方式不改变校验和 | P1 | Component Integration | Determinism | Rules；Match Flow | 两份相同 `MatchState`，本 tick 均产生若干规则事件 | 一份读取并消费全部事件，另一份完全不读取，随后各自推进相同的 100 tick | 同一 tick 的事件集合含 `GroupLocked`、`NuisanceQueued`、`FeverEntered` | 两份状态在推进后的校验和相等；事件消费游标不进入快照与校验和；产生这些事件所依赖的持久字段在两份状态中相同 | [Confirmed] [确定性与快照：快照](../../development/design/determinism-and-snapshot.md#快照)、[状态编码与校验和](../../development/design/determinism-and-snapshot.md#状态编码与校验和) |
| TC-011 | 摘要或版本不匹配时拒绝恢复并返回 typed error | P1 | Component Integration | — | Rules；Match Flow | 一份有效 `MatchSnapshot` 与其开局规格 | 参数化篡改快照头的一项后调用恢复 | 根摘要不匹配；`snapshot_schema_version` 提高一版；随机算法版本提高一版；状态编码版本提高一版 | 四组均返回 typed error 并指出不匹配项；不产生 `MatchState`，不做近似恢复，也不尝试跨版本迁移；未篡改的快照仍能正常恢复 | [Confirmed] [确定性与快照：快照与恢复](../../development/design/determinism-and-snapshot.md#快照与恢复) |
| TC-012 | 结算阶段与 Fever 中任意 tick 的快照恢复保持关键读模型一致 | P1 | Component Integration | Determinism | Rules；Match Flow | 可在结算与 Fever 的任意 tick 停下并快照 | 遍历一次二连锁结算的每个 tick 与一段 Fever 的每个 tick，各执行「快照 → 恢复 → 推进到 `Settlement` 或题面切换」 | 二连锁结算共约 60 tick，逐 tick 取快照；Fever 段取 120 个连续 tick | 每个快照点恢复后得到的 `ChainReport`、下一题面 id 与目标等级、剩余 Fever 时间都与不快照直推的基准相同；无任一 tick 因阶段中间状态未进入快照而分叉 | [Confirmed] [确定性与快照：快照](../../development/design/determinism-and-snapshot.md#快照)；[连锁结算：数据模型](../../development/design/chain-resolution.md#数据模型) |

## 风险查漏

流派生与跨玩家独立性、验证日志的重复与跨进程运行、含回滚的千 tick 推进、五个阶段与逐 tick 的快照恢复、分叉与收敛、覆盖表六个领域的字段敏感性、事件消费的无影响性、三类版本拒绝均有直接用例；快照不覆盖的表现与 AI 状态由其所属测试稿约束。
