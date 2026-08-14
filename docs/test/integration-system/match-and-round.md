# 测试用例设计：小局、BO3 与安全点

**关联设计：** [小局、BO3 与安全点](../../development/design/match-and-round.md)
**关联实现：** `crates/game_core`（`match_state`、`round`、`player`、`view`）

## 需求理解摘要

**功能：** 以唯一入口同步消费双方动作，在安全点仲裁跨玩家结果，并推进小局与 BO3。
**测试性质：** 新功能
**本轮范围：** 安全点的六步顺序、失败判定与同时失败、小局初始化、完成态，以及需要两名玩家才能证明的攻防与结算行为。
**Test Basis：**
- [Confirmed] [小局、BO3 与安全点](../../development/design/match-and-round.md)：聚合模型、安全点顺序与五个行为。
- [Confirmed] [玩法设计 §6.1](../../gameplay.md)：BO3、同时失败判和、重打得到不同随机序列、比赛由某方两胜结束。
- [Confirmed] [小局、BO3 与安全点 §安全点](../../development/design/match-and-round.md#安全点)：抵消与落下都以进入安全点时的队列数量为输入。
**设计基线：** 跨玩家写操作只发生在聚合根，participant slot 的迭代顺序不影响结果。
**关键假设：**
- 失败判定只有一种检查，普通盘与 Fever 盘共用。
- 每局 RNG 由根种子、局号与重打次数独立派生。
**待确认问题：**
- 局间倒计时与结果停留的时长由规则剖面配置，属校准项；校准后需同步更新以其为测试数据的用例。

## 测试点清单

### Component Integration — Match Flow

- 双方在同一 tick 到达不同或相同安全点时，迭代顺序不影响最终状态（TC-001）。
- `TickInputs` 槽位数量错误时拒绝该 tick 且不修改任何状态（TC-002）。
- 普通盘与 Fever 盘的出生失败都按同一条规则结束小局（TC-003）。
- 同一安全点双方失败得到 `Draw`、胜场不变、局号不前进；错开一个 tick 则按正常胜负结算（TC-004～TC-005）。
- 和局重打的球序与上一次不同；`MatchOutcome` 始终由某方两胜产生（Concern: Determinism；TC-006～TC-007）。
- 2:0 与 2:1 两种 BO3 走向；角色选择跨局保留，局内状态正确重置（TC-008～TC-009）。
- `RoundIntro` 与 `RoundOutro` 忽略玩法动作，首个开放 tick 对双方对称（TC-010～TC-011）。
- `MatchEnded` 恰好产生一次；完成态继续调用 `step` 只推进总 tick 且不产生事件（TC-012）。

### Component Integration — Rules；Match Flow

- 一名玩家处于结算阶段而另一名操控活动组时，两方状态各自正确推进（TC-013）。
- 双方同安全点攻击、双方各有旧队列、单方与双方无连锁的组合矩阵（TC-014）。
- participant slot 对调后，镜像初始状态得到镜像结果（TC-015）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 判定表 | 双方连锁有无 × 双方旧队列有无的攻防组合；同时失败与错开失败 | TC-004～TC-005、TC-014 |
| 状态迁移 | `RoundIntro → Playing → RoundOutro → Completed` 的守卫、重复事件与完成态 | TC-003、TC-008～TC-012 |
| 变形测试 | 迭代顺序、slot 对调与镜像输入下的结果关系 | TC-001、TC-015 |
| 场景法 | 2:0 与 2:1 两条完整 BO3 路径；和局重打 | TC-006、TC-008～TC-009 |
| 边界值分析 | 胜场 0/1/2；失败错开 0 与 1 tick | TC-005、TC-007 |
| 错误猜测 | 槽位数量错误、完成态继续推进、双方阶段错位 | TC-002、TC-012～TC-013 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | participant slot 的迭代顺序不改变安全点结果 | P0 | Component Integration | Determinism | Match Flow | 两份完全相同的 `MatchState`，双方均将在本 tick 形成 `PlayerSettlement` | 以两种 slot 迭代顺序各推进同一 tick，并另构造只有一方到达安全点的局面重复一次 | 局面一：双方同 tick 到达安全点，攻击量 5 与 3；局面二：仅 slot 0 到达安全点 | 两种迭代顺序下双方的队列、分数、量表、盘面与 `MatchStepReport` 的事件序列逐项相同；事件按固定类别与 slot 排序，与迭代顺序无关 | [Confirmed] [小局、BO3 与安全点：安全点](../../development/design/match-and-round.md#安全点) |
| TC-002 | 槽位数量错误时拒绝该 tick 且不修改任何状态 | P1 | Component Integration | — | Match Flow | 已推进若干 tick 的 `MatchState`，其校验和已记录 | 参数化提交槽位数量非 2 的 `TickInputs` | `len=0`、`len=1`、`len=3` | 三组均返回槽位数量错误；`match_tick` 不增加，状态校验和与调用前相同；随后提交合法的双槽输入仍能正常推进 | [Confirmed] [小局、BO3 与安全点：一个规则 tick](../../development/design/match-and-round.md#一个规则-tick) |
| TC-003 | 普通盘与 Fever 盘的出生失败按同一条规则结束小局 | P0 | Component Integration | — | Match Flow | 一方即将在生成活动组时触发出生失败 | 参数化两种局面各推进到失败所在安全点 | 局面一：slot 0 在普通盘出生列堆满；局面二：slot 0 处于 Fever 且 Fever 盘出生列堆满 | 两种局面都在安全点第 5 步产生 `PlayerDefeated(slot 0)`，`RoundOutcome` 为 `Decided(slot 1)`；胜场记到 slot 1；Fever 局面不因即将退出 Fever 而规避失败判定 | [Confirmed] [小局、BO3 与安全点：失败判定](../../development/design/match-and-round.md#失败判定)；[Fever 循环：退出 Fever](../../development/design/fever-mode.md#退出-fever) |
| TC-004 | 同一安全点双方失败判和且比分与局号不变 | P1 | Component Integration | — | Match Flow | 双方均将在同一批失败检查中满足出生失败条件 | 推进到该安全点并读取小局与比赛状态 | `round_index=0`、`wins=[0,0]`、`draw_attempt=0` | `RoundOutcome` 为 `Draw`；`wins` 保持 `[0,0]`；`round_index` 仍为 0；`draw_attempt` 变为 1 并以同一局号重打；不产生 `MatchOutcome` | [Confirmed] [小局、BO3 与安全点：失败判定](../../development/design/match-and-round.md#失败判定)；[玩法设计 §6.1](../../gameplay.md) |
| TC-005 | 失败错开一个 tick 时按正常胜负结算 | P1 | Component Integration | — | Match Flow | 两名玩家的失败条件分别在相邻两个 tick 成立 | 推进两个 tick 并读取小局结果 | slot 0 在 tick `t` 失败、slot 1 在 tick `t+1` 失败 | `RoundOutcome` 为 `Decided(slot 1)`，`wins` 变为 `[0,1]`，`round_index` 前进；slot 1 在 `t+1` 的条件不再被检查，不改判为 `Draw` | [Confirmed] [小局、BO3 与安全点：失败判定](../../development/design/match-and-round.md#失败判定)；[玩法设计 §6.1](../../gameplay.md) |
| TC-006 | 和局重打得到与上一次不同的球序 | P0 | Component Integration | Determinism | Match Flow | 已在 `round_index=0` 判和一次 | 记录判和前与重打后双方各 16 手的形状与颜色序列 | 根种子 `0x1`；`round_index=0`；`draw_attempt` 由 0 变为 1 | 重打后双方的颜色序列与上一次不同；相同根种子、局号与 `draw_attempt` 重新初始化则得到完全相同的序列；两名玩家的序列互不相同 | [Confirmed] [小局、BO3 与安全点：小局初始化](../../development/design/match-and-round.md#小局初始化)；[DEC-005](../../development/decision/color-sequence-derivation.md) |
| TC-007 | MatchOutcome 只能由某一方达到两胜产生 | P1 | Component Integration | — | Match Flow | 可构造含多次和局的小局序列 | 推进一场包含两次和局的 BO3 直到结束 | 小局结果序列：`Draw`、`Decided(0)`、`Draw`、`Decided(1)`、`Decided(0)` | 前四局结束时 `wins` 依次为 `[0,0]`、`[1,0]`、`[1,0]`、`[1,1]`，均未产生 `MatchOutcome`；第五局后 `wins=[2,1]` 并产生 `MatchOutcome(slot 0)`；和局不计入比分且不设次数上限 | [Confirmed] [小局、BO3 与安全点：一个规则 tick](../../development/design/match-and-round.md#一个规则-tick)；[玩法设计 §6.1](../../gameplay.md) |
| TC-008 | 2:0 的 BO3 在第二局结束时完成 | P0 | Component Integration | — | Match Flow | 已冻结的两人对局规格 | 驱动两小局均由 slot 0 获胜 | 小局结果：`Decided(0)`、`Decided(0)` | 第一局后 `wins=[1,0]` 且自动建立下一小局；第二局后 `wins=[2,0]`、`phase` 为 `Completed`、`MatchOutcome` 为 slot 0；不再建立第三小局 | [Confirmed] [小局、BO3 与安全点：一个规则 tick](../../development/design/match-and-round.md#一个规则-tick) |
| TC-009 | 2:1 的 BO3 跨局保留角色并重置局内状态 | P1 | Component Integration | — | Match Flow | 已冻结的两人对局规格，双方角色不同 | 驱动三小局并在每局开始时读取双方状态 | 小局结果：`Decided(0)`、`Decided(1)`、`Decided(0)`；第一局结束时 slot 0 的分数、队列、量表与 Fever 时间均非零 | 第三局后 `wins=[2,1]` 并产生 `MatchOutcome(slot 0)`；每局开始时双方盘面为空、分数、攻击余数、量表、Fever 时间与两个通道队列均重置为初值；`character_id` 与 `wins` 跨局保留 | [Confirmed] [小局、BO3 与安全点：小局初始化](../../development/design/match-and-round.md#小局初始化) |
| TC-010 | RoundIntro 与 RoundOutro 忽略玩法动作 | P1 | Component Integration | — | Match Flow | 小局处于 `RoundIntro`；另构造处于 `RoundOutro` 的局面 | 在两个阶段分别提交含全部六种玩法动作的 `TickInputs` 并推进 | 每个阶段推进 10 tick，双方每 tick 均提交 `Left+RotateCW+SoftDrop+HardDrop` | 两个阶段的盘面、活动组与分数均不变；只有阶段计时按 tick 递减，`match_tick` 正常递增；输入被消费而不滞留到下一阶段生效 | [Confirmed] [小局、BO3 与安全点：一个规则 tick](../../development/design/match-and-round.md#一个规则-tick) |
| TC-011 | RoundIntro 结束时首个开放 tick 对双方对称 | P1 | Component Integration | — | Match Flow | 小局处于 `RoundIntro` 的最后一个 tick | 推进到 `Playing` 的首个 tick，双方提交相同动作 | 倒计时剩余 1 tick；双方均提交 `SoftDrop` | 双方在同一个 tick 由 `RoundIntro` 进入 `Playing`；该 tick 双方都已持有活动组且都消费了本 tick 输入；两侧的 `round_tick` 与操控计时器起点相同 | [Confirmed] [小局、BO3 与安全点：小局初始化](../../development/design/match-and-round.md#小局初始化) |
| TC-012 | MatchEnded 恰好产生一次且完成态只推进总 tick | P1 | Component Integration | — | Match Flow | 已达到 2:0 的 `MatchState` | 在比赛结束的那一 tick 之后继续推进若干 tick | 结束后再推进 10 tick，每 tick 提交合法双槽输入 | `MatchEnded` 在结束 tick 产生一次，后续 10 tick 不再产生；每个后续 tick 的 `MatchStepReport` 不含任何事件；`match_tick` 每 tick 加一；`wins` 与 `MatchOutcome` 不再变化，也不返回错误 | [Confirmed] [小局、BO3 与安全点：完成态](../../development/design/match-and-round.md#完成态) |
| TC-013 | 一方结算而另一方操控时两方状态各自推进 | P1 | Component Integration | — | Rules；Match Flow | slot 0 已进入结算，slot 1 持有活动组 | 推进跨越 slot 0 完整结算过程的 tick 序列，slot 1 每 tick 提交操控动作 | slot 0：二连锁，`clear_preview_ticks=12`、重力时长查表；slot 1：持续 `Left` 与一次 `RotateCW` | slot 0 的结算阶段按自身 tick 序列推进，不被 slot 1 的输入改变；slot 1 的横移与旋转按操控规则生效，不因对方结算而暂停或延迟；两方的 Fever 与 margin 时钟在此期间都继续推进 | [Confirmed] [连锁结算：协作](../../development/design/chain-resolution.md#协作)；[小局、BO3 与安全点：一个规则 tick](../../development/design/match-and-round.md#一个规则-tick) |
| TC-014 | 双方攻防的四种组合在同一安全点得到确定结果 | P0 | Component Integration | — | Rules；Match Flow | 可构造双方连锁与旧队列的任意组合 | 参数化四组局面各推进到同一安全点并读取双方队列与落盘 | 组一：双方均连锁，攻击 5 与 3，旧队列均 0；组二：双方均连锁，攻击 5 与 3，旧队列 4 与 6；组三：slot 0 连锁攻击 5、slot 1 无连锁且旧队列 6；组四：双方均无连锁，旧队列 6 与 8 | 组一：双方各自无可抵消队列，slot 0 队列变 3、slot 1 变 5，本安全点不落下；组二：各自只抵消进入安全点时已有的量，slot 0 队列变 0、slot 1 变 4，本安全点不落下；组三：slot 0 队列变 0，slot 1 未连锁并在第 4 步只落下进入安全点时已有的 6 颗，本安全点收到的 5 颗留在队列等待下一个安全点；组四：双方分别落下 6 与 8 颗，不产生新攻击 | [Confirmed] [小局、BO3 与安全点：安全点](../../development/design/match-and-round.md#安全点) |
| TC-015 | slot 对调后镜像初始状态得到镜像结果 | P1 | Component Integration | Determinism | Rules；Match Flow | 两份由构造给定的 `MatchState`（盘面、队列与计时器直接设定，不经随机流生成），第二份是第一份按 slot 对调的镜像 | 对两份状态提交互为镜像的逐 tick 输入并推进到同一安全点 | 60 tick 输入日志；初始队列 4 与 6、量表 3 与 5、待消盘面各自给定 | 第二份的 slot 0 结果等于第一份的 slot 1 结果，slot 1 亦然；分数、队列、量表、Fever 时间与 `RoundOutcome` 逐项镜像；事件序列在按 slot 重映射后相同 | [Confirmed] [小局、BO3 与安全点：安全点](../../development/design/match-and-round.md#安全点) |

## 风险查漏

迭代顺序无关、槽位校验、两块盘共用的失败判定、同时与错开失败、和局重打的序列差异、BO3 的两条走向与跨局重置、两个非玩法阶段、完成态与事件唯一性、双方阶段错位与攻防四种组合、slot 镜像均有直接用例；单盘内部的操控、结算、攻防与 Fever 规则不在本稿，见对应 Component 测试稿。
