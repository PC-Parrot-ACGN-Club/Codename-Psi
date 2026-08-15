# 测试用例设计：得分、攻击与垃圾攻防

**关联设计：** [得分、攻击与垃圾攻防](../../development/design/offense-and-nuisance.md)、[连锁强度曲线](../../development/design/chain-power-curve.md)、[DEC-003](../../development/decision/nuisance-queue-representation.md)
**关联实现：** `crates/game_core`（`scoring`、`attack`、`nuisance`）

## 需求理解摘要

**功能：** 把连锁事实换算成分数与攻击，完成余数携带、抵消与垃圾落盘。
**测试性质：** 新功能
**本轮范围：** 连锁步计分、分数换算攻击、单方抵消与垃圾落下；双方安全点仲裁由聚合根测试稿覆盖。
**Test Basis：**
- [Confirmed] [得分、攻击与垃圾攻防](../../development/design/offense-and-nuisance.md)：数据模型与四个行为。
- [Confirmed] [玩法设计 §4.1](../../gameplay.md)：计分公式、`CB`/`GB` 表、目标分与 margin、软降加分不计入换算。
- [Confirmed] [玩法设计 §4.2](../../gameplay.md)：连续抵消、单次落下上限与两种余数列顺分支。
- [Confirmed] [Scoring](https://puyonexus.com/wiki/Scoring)：余数进位公式与 List of Chain Scores 逐链样本。
**设计基线：** 攻击换算不区分对手类型；待接收垃圾为每通道一个精确整数。
**关键假设：**
- 余数携带使逐 link 换算与整链一次换算的总量恒等，因此两者可互为 golden 样本。
- `MarginState` 只持有整数表下标，`TP` 由查表取得。
**待确认问题：**
- 无。

## 测试点清单

### Component — Rules

- 计分公式的单链、多组、多色与倍率 clamp 样例（Concern: Content Validation；TC-001～TC-002）。
- 软降加分只增加显示分数，不进入攻击换算（TC-003）。
- 逐 `ChainLink` 换算的累计结果与整链一次换算逐点相等（TC-004）。
- 两名角色的普通盘与 Fever 盘曲线查询、表尾行为，以及交换角色后攻击结果随 profile 变化（TC-005～TC-006）。
- 跨多次攻击的余数守恒；margin 阶段推进只改变表下标（TC-007～TC-008）。
- 连续抵消、未连锁落下、单次上限 30、整行填充与两种余数列顺分支（TC-009～TC-013）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 等价类划分 | 单组、多组、多色三类连锁步事实；活动通道与另一通道的抵消来源 | TC-001、TC-009 |
| 边界值分析 | `clamp` 的 1 与 999；曲线索引 1、10、24、25；落下量 29/30/31；余数 1 与 2 | TC-002、TC-005、TC-012～TC-013 |
| 判定表 | 攻击量 × 活动通道队列 × 另一通道队列的抵消结果；本次落子是否触发连锁 | TC-009～TC-011 |
| 变形测试 | 逐 link 换算与整链一次换算的总量关系；交换角色的结果关系 | TC-004、TC-006 |
| 性质测试 | 跨多次攻击的余数守恒：`Σ NC × TP + 余数 × TP = Σ SC` | TC-007 |
| 不变量检查 | 软降加分不进入换算；队列不减到负数；margin 推进只改变表下标 | TC-003、TC-008～TC-009 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 单组、多组与多色连锁步按公式计分 | P0 | Component | Content Validation | Rules | 已冻结角色 A 的普通盘曲线，`CB`/`GB` 取 Fever 表 | 参数化提交四个 `ChainLinkFacts` 并读取本步分数 | 步 1 单组 4 球单色；步 1 单组 5 球单色；步 1 红 4+蓝 4；步 3 红 4+蓝 4+绿 4 | 四组分数依次为 160（40×4）、250（50×5）、480（80×6）、3360（120×28）；`CP` 分别取 `A.normal[1]=4`、`[1]=4`、`[1]=4`、`[3]=24`，`CB` 依次为 0、0、2、4，`GB` 依次为 0、1、0、0 | [Confirmed] [玩法设计 §4.1](../../gameplay.md)；[得分与攻防：连锁步计分](../../development/design/offense-and-nuisance.md#连锁步计分) |
| TC-002 | 倍率之和在 1 与 999 两端被 clamp | P1 | Component | Content Validation | Rules | 可分别构造下界与上界的曲线与连锁步事实 | 提交两个触及边界的 `ChainLinkFacts` | 下界：`CP=1`、1 色、单组 4 球；上界：`CP=A.normal[15]=999`、4 色、组大小 `[4,4,4,11]` | 下界组倍率之和为 1，分数为 40（40×1）；上界组倍率之和 `999+8+8=1015` 被 clamp 到 999，分数为 229770（230×999）；clamp 只作用于倍率之和，不作用于 `10 × PC` | [Confirmed] [玩法设计 §4.1](../../gameplay.md)；[得分与攻防：连锁步计分](../../development/design/offense-and-nuisance.md#连锁步计分) |
| TC-003 | 软降加分只增加显示分数不进入攻击换算 | P1 | Component | — | Rules | `ScoreState` 分别保存显示分数与软降加分 | 在一次落子中累计软降加分后触发一个连锁步并换算攻击 | 软降加分 60；连锁步分数 160；`TP=120`；初始余数 0 | 显示分数增加 220；攻击换算只使用 160，得 `NC=1`、余数 `1/3`；软降加分不改变 `NC` 与余数 | [Confirmed] [玩法设计 §4.1](../../gameplay.md)；[得分与攻防：连锁步计分](../../development/design/offense-and-nuisance.md#连锁步计分) |
| TC-004 | 逐 ChainLink 换算与整链一次换算的攻击总量相等 | P0 | Component | — | Rules | 角色 A 普通盘曲线，`TP=120`，初始余数 0 | 对同一条三步连锁分别执行逐步换算与整链一次换算 | 三步各为单组 4 球单色，分数依次 160、480、960（`CP=4/12/24`），总分 1600 | 逐步换算得 `NC` 依次为 1、4、8，合计 13，末余数 `1/3`；整链一次换算得 `NC=13`、余数 `1/3`；两者的攻击总量与末余数逐项相等 | [Confirmed] [得分与攻防：分数换算攻击](../../development/design/offense-and-nuisance.md#分数换算攻击)；[Scoring](https://puyonexus.com/wiki/Scoring) |
| TC-005 | 两名角色的两条曲线按盘面模式与连锁步取值并在表尾饱和 | P1 | Component | Content Validation | Rules | 已冻结两名角色的 `ChainPowerProfile` | 参数化按角色、盘面模式与连锁步查询 `CP` | 连锁步 1、10、24、25；角色 A 与 B 的普通盘与 Fever 盘 | `A.normal` 得 4、440、999、999；`A.fever` 得 4、248、840、840；`B.normal` 得 4、380、999、999；`B.fever` 得 4、275、940、940；步 25 一律返回表尾值 | [Confirmed] [连锁强度曲线：角色参数与生成表](../../development/design/chain-power-curve.md#角色参数与生成表)；[玩法设计 §3.3](../../gameplay.md) |
| TC-006 | 相同连锁事实在交换角色后得到不同攻击 | P1 | Component | — | Rules | 两份只有 `character_id` 不同的冻结规格 | 对两份规格提交同一个 `ChainLinkFacts` 并换算攻击 | 步 4、单组 4 球单色、普通盘；`TP=120`、初始余数 0；`A.normal[4]=33`、`B.normal[4]=29` | 角色 A 得分 1320、`NC=11`、余数 0；角色 B 得分 1160、`NC=9`、余数 `2/3`；差异只来自曲线取值，换算路径不含对手类型系数 | [Confirmed] [得分与攻防：分数换算攻击](../../development/design/offense-and-nuisance.md#分数换算攻击)；[玩法设计 §4.1](../../gameplay.md) |
| TC-007 | 余数跨多次落子携带且攻击总量守恒 | P0 | Component | — | Rules | `TP=120`，初始余数 0 | 连续换算三次落子的攻击并累计 | 落子 1：TC-004 的三步连锁（1600）；落子 2 与落子 3：各一步单组 4 球（160） | 三次落子的 `NC` 依次为 13、1、2，合计 16；末余数为 0；`Σ NC = ⌊Σ SC / TP⌋ = 1920 / 120 = 16`，余数在落子之间不被丢弃也不被重复计入 | [Confirmed] [得分与攻防：分数换算攻击](../../development/design/offense-and-nuisance.md#分数换算攻击)；[Scoring](https://puyonexus.com/wiki/Scoring) |
| TC-008 | margin 推进只改变表下标且 TP 由查表取得 | P1 | Component | — | Rules | 小局已推进到 margin 起始时刻 | 推进到首次衰减后读取 `MarginState` 与 `TP`，并换算一次攻击 | margin 表下标 0→`TP=120`、下标 1→`TP=90`；连锁步分数 360 | 推进后 `MarginState` 只有表下标由 0 变为 1，未保存换算后的 `TP` 副本；换算使用查表所得的 90，得 `NC=4`、余数 0；同一分数在下标 0 时得 `NC=3`、余数 0 | [Confirmed] [得分与攻防：数据模型](../../development/design/offense-and-nuisance.md#数据模型)；[玩法设计 §4.1](../../gameplay.md) |
| TC-009 | 抵消先消耗活动通道再消耗另一通道且队列不减到负数 | P0 | Component | — | Rules | 玩家持有普通与 Fever 两个通道的 `PendingNuisance` | 参数化提交四组攻击量与队列组合并执行抵消 | 活动通道/另一通道/攻击量依次为 3/4/2、3/4/5、3/4/10、0/0/6 | 依次得到：活动 1、另一 4、送出 0；活动 0、另一 2、送出 0；活动 0、另一 0、送出 3；两通道保持 0、送出 6；任一组合下队列均不小于 0，`OffsetFacts` 记录的抵消数量分别为 2、5、7、0 | [Confirmed] [得分与攻防：抵消](../../development/design/offense-and-nuisance.md#抵消)；[玩法设计 §4.2](../../gameplay.md) |
| TC-010 | 本次落子触发连锁时未抵消的垃圾留在队列不落下 | P1 | Component | — | Rules | 活动通道队列非零 | 完成一次触发连锁的落子并推进到垃圾落下判定 | 队列 8；本次落子攻击 3，抵消后队列剩 5 | 队列保持 5 且本次不落下任何垃圾；玩家继续获得下一活动组；`NuisanceDropState` 的列序位置不变 | [Confirmed] [得分与攻防：垃圾落下](../../development/design/offense-and-nuisance.md#垃圾落下)；[玩法设计 §4.2](../../gameplay.md) |
| TC-011 | 本次落子未触发连锁且队列非零时垃圾落入活动盘面 | P1 | Component | — | Rules | 活动通道队列非零，盘面有足够空位 | 完成一次未触发连锁的落子并推进到垃圾落下判定 | 队列 6；列序起始列 `x=0` | 落下 6 颗填满一整行；队列变为 0；入场坐标为 `x=0..5` 的同一行，即各列最高的空格；这批球随后走重力流程 | [Confirmed] [得分与攻防：垃圾落下](../../development/design/offense-and-nuisance.md#垃圾落下) |
| TC-012 | 单次落下上限为 30 且余量留在队列 | P1 | Component | — | Rules | 活动通道队列超过单次上限，盘面有足够空位 | 参数化三种队列量各完成一次未触发连锁的落子 | 队列 29、30、35；单次上限 30 | 依次落下 29、30、30 颗，队列分别剩 0、0、5；落下 30 颗时恰为 5 整行；余量不在同一次落下中补发 | [Confirmed] [得分与攻防：垃圾落下](../../development/design/offense-and-nuisance.md#垃圾落下)；[玩法设计 §4.2](../../gameplay.md) |
| TC-013 | 不足一行的余数按两种分支推进列序 | P1 | Component | — | Rules | 活动通道队列不是 6 的整数倍，列序起始列已知 | 参数化两种队列量各完成一次落下，再各完成一次落下并记录首颗所在列 | 列序起始列 `x=0`；队列 13（余 1）与队列 14（余 2） | 队列 13：先填 2 整行，余 1 颗落在 `x=0`，下一次落下的首颗从 `x=1` 开始；队列 14：先填 2 整行，余 2 颗落在 `x=0`、`x=1`，下一次落下的首颗从上一颗所在列 `x=1` 开始；两个分支的列序位置进入通道状态 | [Confirmed] [玩法设计 §4.2](../../gameplay.md)；[得分与攻防：垃圾落下](../../development/design/offense-and-nuisance.md#垃圾落下) |

## 风险查漏

计分公式的三类事实与两端 clamp、软降加分的排除、逐步与整链换算的等值、两名角色的四条曲线取值与表尾、余数跨落子守恒、margin 表下标语义、抵消的四种组合、连续抵消的两个分支、单次上限与两种列顺分支均有直接用例；双方同安全点的攻防仲裁不在本稿，见[小局、BO3 与安全点](../integration-system/match-and-round.md)。
