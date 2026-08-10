# 玩法设计：Fever 连锁对战

**文档定位：** 立项时制定的玩法契约。描述对局流程、规则轮廓与原作数值默认来源，作为规则引擎开发至少应覆盖的范围；开发与校准时可修订条文或配置，修订时同步更新本文档、`RuleProfile` 与验证样本。  
**产品范围：** R1 本地对战与 AI，R2 局域网对战共用同一规则引擎  
**关联：** [PRD](PRD.md)、[TDD](TDD.md)

## 1. 目的与参考范围

本文件描述本项目的对局流程和规则边界。玩法以 Puyo Nexus Wiki 所记录的 **Fever 规则**为参考，使用独立的原创角色、名称、视觉、音频和界面表达。

参考剖面默认为 **Puyo Puyo Fever / Fever 2 的主机与 PC 规则轮廓**。Wiki 记录了多个作品和平台在 Fever 时间、题面、垃圾顺序与失败处理上的差异；一场对局运行时以当时锁定的版本化 `RuleProfile` 为准，同一场内保持单一规则剖面。[Fever 规则差异表](https://puyonexus.com/wiki/Fever_%28rule%29)

**数值默认策略：** `RuleProfile` 与 `fever.ron` 中的可调常数默认录入该参考剖面的原作数值；来源页见各节 Wiki 链接与 §8。优先在配置层校准数值，并保留引用与验证样本 ID；契约层变更时更新本节与关联验收项。

Fever 系列的角色选择包含可影响对局的掉落组与连锁强度。R1 的两位原创角色各自绑定一个数据化 `DropSet`；双方共享同一 `RuleProfile`，每个角色拥有自己固定的掉落序列。角色 UI、掉落组和连锁强度配置都在开局前锁定，并写入联机握手与开发用确定性验证元数据。特殊球种、道具和额外竞技模式属于当前范围外内容。面向玩家的对局回放不在产品范围（见 [PRD](PRD.md) §2.3）；下文「输入日志 / 确定性验证」仅指开发与 CI。

## 2. 一局的完整流程

```text
比赛开始（BO3，比分 0:0）
  → 小局准备：锁定 RuleProfile、随机种子、玩家角色、输入映射
  → 双方展示信息并倒计时
  → 循环执行 60Hz 对局 tick
       生成球对 → 输入操控 → 锁定 → 消除结算 → 攻击抵消/入队
       → Fever 判定或垃圾落下 → 生成下一球对
  → 一方触及溢出判定格：另一方赢得小局
  → 更新比分；先获两局者赢得比赛
  → 赛果页：再来一局或返回主菜单
```

小局的对局状态为 `Normal`、`Resolving`、`Fever`、`RoundOver`。表现层可以在状态切换时播放动画和音效；规则层以固定 tick 推进，所有会影响胜负的计时、随机数、输入与攻击结算都进入网络同步状态，并可由开发用输入日志复现。

## 3. 棋盘、球对与操作

### 3.1 棋盘与失败

- 每名玩家使用独立的 6 列 × 12 行棋盘；顶部出生区含一个溢出判定格。
- 球对首次生成在出生区。某次结算后溢出判定格被占据时，该玩家立即输掉小局。
- 棋盘只保存规则格子。渲染球体、缩放、粒子和 UI 指示由客户端从规则快照生成。

Wiki 的通用规则采用 6 × 12 格棋盘，且以标记的出生/失败格作为失败判定。[Basic rules](https://puyonexus.com/wiki/Basic_rules)

### 3.2 角色掉落组（DropSet）

掉落组是某个角色在支持该机制的模式中使用的固定落子形状序列。它自《Puyo Puyo Fever》起存在于主要作品，并贯穿 Fever、Transformation、Searchlight、Slot、Pair Puyo、Party 与 Tiny Puyo 等模式。[Dropset](https://puyonexus.com/wiki/Dropset)

| 作品 / 模式 | 角色掉落组 | 说明 |
| --- | --- | --- |
| Fever、Fever 2 | 有 | 每个角色使用独立序列；Fever 1/2 还有 16 手后 L/J 翻转的特有循环。 |
| 15th Anniversary、Puyo Puyo 7、20th Anniversary | 有 | Fever 规则沿用角色掉落组；后续作品为新角色增加或调整掉落组。 |
| Chronicle、Champions | 有 | Fever 模式明确使用角色掉落组和角色连锁强度。 |
| Puyo Puyo Tetris、Puyo Puyo Tetris 2 | 有限使用 | Party、Fusion、Tiny Puyo 等模式使用；常规 Versus 的 Puyo 侧采用 Tsu 规则。 |
| Non-Stop Fever、Mini Puyo Fever | 无角色差异 | 所有角色使用同一个预设掉落组。 |

《Puyo Puyo Champions》的 Fever 对战明确使用独特掉落组和角色连锁强度。[Champions 玩法说明](https://puyonexus.com/wiki/Puyo_Puyo_Champions) 《Puyo Puyo Chronicle》也在 Fever 模式中为全部角色提供掉落组。[Chronicle 玩法说明](https://puyonexus.com/wiki/Puyo_Puyo_Chronicle) 

`DropSet` 是 16 手的循环序列。每一手保存形状、同色/双色布局及颜色抽取状态：

| 形状 | 球数 | 规则表达 |
| --- | ---: | --- |
| `I` | 2 | 纵向双球，轴心在下。 |
| `L` / `J` | 3 | 拐角三球；配置决定哪一条边使用第一颜色。 |
| `O` | 4 | 2×2 方块；可为单色或上下两色。单色 `O` 的旋转输入用于切换颜色。 |

Fever 1/2 的 L/J 数量与落点位置属于角色序列的一部分；部分角色每 16 手切换 L 与 J。15th 及后续 Fever 作品保留角色序列，同时移除了该 L/J 周期差异。[Dropset 细节](https://puyonexus.com/wiki/Dropset)

R1 为两个原创角色配置两个平衡的 16 手序列，并以 `drop_set_id` 关联。首版不复刻既有角色名称、序列或颜色种子；每个原创序列经镜像/对称校验、AI 对局和胜率测试后才发布。`ChainPowerProfile` 存放连锁攻击倍率表：原作按角色分档（见 [List of attack powers](https://puyonexus.com/wiki/List_of_attack_powers) 的 Fever / Fever 2 · Fever 表）；R1 双方共享同一张中位参考表（录入时注明所取角色档与作品），后续平衡版可按角色拆分。

### 3.3 下落操作

- 每个回合按当前角色 `DropSet` 生成一组 2、3 或 4 球的下落组；颜色来自本局锁定的确定性随机序列和该手的颜色布局。
- 玩家可执行左移、右移、软降、硬降、顺时针旋转和逆时针旋转。
- 左右移动、旋转和下落只在目标格有效时生效；硬降将球对落到当前列可达的最低合法位置并进入锁定流程。
- 旋转以配置的轴心为中心。地面空间不足时应用上抬旋转；靠墙旋转时应用横向平移；双球组夹在两列之间时，连续旋转可完成 180° 翻转。具体尝试顺序写入 `rotation` 规则配置并以表驱动测试覆盖。

地面旋转、墙边旋转与双旋转属于系列通用旋转机制。[Rotation](https://puyonexus.com/wiki/Rotation)

### 3.4 锁定与连锁结算

一次锁定后的结算必须完整执行，再生成下一球对：

1. 对活动球对应用重力并写入棋盘。
2. 查找横向或纵向相连、数量至少为 4 的同色普通球。
3. 同时移除本轮命中的全部普通球，以及与它们四向相邻的垃圾球。
4. 对剩余球应用重力。
5. 若重力后又出现可消组，进入下一连锁轮；直到棋盘稳定。

连锁数等于本次锁定中发生的消除轮数。普通球达到四向连接的四个或以上即可消除；垃圾球通过与普通球消除组相邻而清除。[Basic rules](https://puyonexus.com/wiki/Basic_rules)

## 4. 分数、攻击与垃圾

### 4.1 分数与攻击点

每轮分数与垃圾换算采用 Wiki [Scoring](https://puyonexus.com/wiki/Scoring) 公式；连锁倍率（Chain Power）表来自 [List of attack powers](https://puyonexus.com/wiki/List_of_attack_powers)。

```text
score = (10 × PC) × clamp(CP + CB + GB, 1, 999)
```

| 符号 | 含义 | 来源 |
| --- | --- | --- |
| `PC` | 本轮消除的普通球数 | [Scoring](https://puyonexus.com/wiki/Scoring) |
| `CP` | 当前连锁步的连锁倍率；表值为 0 时按 1 | [List of attack powers](https://puyonexus.com/wiki/List_of_attack_powers) |
| `CB` | 颜色倍率（Fever 表） | 下表 / [Scoring](https://puyonexus.com/wiki/Scoring) |
| `GB` | 各组组倍率之和（Fever 表） | 下表 / [Scoring](https://puyonexus.com/wiki/Scoring) |

**Fever 颜色倍率（CB）** — [Scoring § Color Bonus](https://puyonexus.com/wiki/Scoring)

| 本轮消除颜色数 | 1 | 2 | 3 | 4 | 5 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Fever | 0 | 2 | 4 | 8 | 16 |

**Fever 组倍率（单组 GB，多组相加）** — [Scoring § Group Bonus](https://puyonexus.com/wiki/Scoring)

| 组内球数 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11+ |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Fever | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 8 |

攻击垃圾数量按累计余数计算 — [Scoring § Nuisance Formula](https://puyonexus.com/wiki/Scoring)：

```text
NP = SC / TP + NL
NC = floor(NP)
NL = NP - NC
```

| 符号 | 含义 | 默认值 / 来源 |
| --- | --- | --- |
| `SC` | 本轮（或本链累计，按结算实现）分数 | [Scoring](https://puyonexus.com/wiki/Scoring) |
| `TP` | 目标分（每颗垃圾所需分数） | Fever 对战标准 **120** — [Margin time](https://puyonexus.com/wiki/Margin_time)（Scoring 页「Default is 70」指通用默认，Fever 对战以 120 为准） |
| `NL` | 上次余数，∈ [0, 1) | 跨攻击携带 |

**Margin time（目标分衰减）：** 小局进行到 margin 时间后，`TP` 按 Wiki 算法递减（首次 ×0.75，之后每 16 秒一次，最多 14 次或降至 1）。Fever 对战初始 `TP = 120` 的迭代表见 [Margin time § Target Points = 120](https://puyonexus.com/wiki/Margin_time)。首版启用该机制；margin 秒数按 Fever 1/2 主机/PC 参考值录入配置。

**软降加分（Drop Bonus）：** 可计入画面分数；是否计入垃圾换算因作品而异。[Scoring § Drop Bonus](https://puyonexus.com/wiki/Scoring) 明确写出「Fever 规则下也计入」的作品为 15th / Puyo Puyo 7。首版按 Fever 1/2 主机/PC：软降加分**只显示、不计入**垃圾换算，除非校准证明参考剖面需要计入。

**人对人伤害衰减：** Fever 1/2 主机/PC 差异表记 Human VS 使用分数衰减 **666/999**。[Fever 规则差异表](https://puyonexus.com/wiki/Fever_%28rule%29) 本地双人与 LAN 启用；人机对战按「Human VS Only」默认不启用，配置可单独打开。

### 4.2 抵消、入队与落下

- 同一结算帧先计算双方的总攻击，再各自抵消自己的待接收垃圾；余量进入对手的待接收垃圾队列。
- 玩家完成任意连锁且队列中有待接收垃圾时，连锁攻击优先用于抵消该队列。
- Fever 规则采用连续抵消：本回合触发连锁时，尚未抵消的垃圾继续留在队列，玩家获得下一球对；本回合未触发连锁时，待接收垃圾落入棋盘。[Fever 规则](https://puyonexus.com/wiki/Fever_%28rule%29) [Offset rule](https://puyonexus.com/wiki/Offset_rule)
- 单次落下上限默认 **30**（一整块 Rock = 五整行）；列序采用 Fever 系固定顺序（非整随机），「上一落点后续 / 下一列」分支按 Fever 1/2 主机/PC 写入配置并单测。[Nuisance queue](https://puyonexus.com/wiki/Nuisance_queue) [Fever 规则 § Nuisance Order](https://puyonexus.com/wiki/Fever_%28rule%29)
- 垃圾 UI 同时展示精确数量和分级图标。图标单位取系列标准：1 / 6 / 30 / 180 / 360 / 720；Fever / Fever 2 符号上限为 Crown（720）。数字是准确来源。[Nuisance queue](https://puyonexus.com/wiki/Nuisance_queue)

## 5. Fever 系统

### 5.1 量表与进入条件

- Fever 量表有 **7** 格；抵消待接收垃圾并完成任意连锁时填充 1 格。[Fever 规则](https://puyonexus.com/wiki/Fever_%28rule%29)
- 量表填满后，在当前结算安全点进入 Fever；进入前保存普通棋盘和普通垃圾队列。
- Fever 时间范围 **15–30 秒**；具体初值与上下限进 `RuleProfile`。[Fever 规则](https://puyonexus.com/wiki/Fever_%28rule%29)
- 竞技剖面：4 色、开局量表 **0/7**（Wiki Difficulties · Normal）。[Fever 规则 § Difficulties](https://puyonexus.com/wiki/Fever_%28rule%29)
- Fever 1/2 主机/PC：对手**抵消**己方攻击时，给对手 Fever 时间 +1 秒；显示秒数向下取整。[Fever 规则差异表](https://puyonexus.com/wiki/Fever_%28rule%29)

### 5.2 Fever 对局循环

1. 切换到 Fever 棋盘和 Fever 垃圾队列；普通棋盘与普通垃圾队列保持冻结。
2. 按当前 Fever 等级生成一个预设连锁题面，题面目标连锁长度 **3–15**（主机/PC 最小题面为 3）。[Fever 规则](https://puyonexus.com/wiki/Fever_%28rule%29)
3. 玩家使用正常球对操作触发或延长题面连锁，并将攻击照常送往对手。
4. 题面结果分支（原文）— [Fever 规则 § Fever Mode](https://puyonexus.com/wiki/Fever_%28rule%29)：
   - 达标（含延长后达标）：下一题面目标 = 实际打出连锁 + 1
   - 全消：下一题面目标 = 实际打出连锁 + 2
   - 差 1：维持同等目标；差 2：目标 = 实际打出 − 1；差 ≥3：目标 = 实际打出 − 2
5. Fever 时间到零：主机/PC 为**存活并翻回普通盘**（部分掌机为直接判负）。Fever 队列合并回普通队列；若归零瞬间未抵消且任一队列有垃圾，翻回后垃圾立即落下。[Fever 规则差异表](https://puyonexus.com/wiki/Fever_%28rule%29)

### 5.3 全消与失败

- 普通场全消：投放预设 4 连题面，Fever 时间 +5 秒。[All clear](https://puyonexus.com/wiki/All_clear) [Fever 规则](https://puyonexus.com/wiki/Fever_%28rule%29)
- Fever 中全消：下一题面 +2 连，Fever 时间 +5 秒（时间尚未耗尽时）。
- 全消同时进入 Fever：首个 Fever 题面 +2 连，并 +5 秒。
- Fever 内溢出判定格被占据：立即输掉小局。

## 6. 比赛流程与模式

### 6.1 BO3 对战

- 每场比赛固定为 BO3，先胜两局者获胜。
- 小局开始前展示双方角色、当前比分和倒计时。
- 本地模式可暂停、重开或退出；网络模式持续运行，断线事件直接判定断线方输掉比赛。
- 小局结算完成后更新比分并自动开始下一局；比赛结算后进入赛果页。

### 6.2 对手类型

| 模式 | 对手输入来源 | 规则差异 |
| --- | --- | --- |
| 单人 | AI 通过合法动作选择球对位置和旋转 | 无 |
| 本地双人 | 同一设备的 P1 / P2 输入映射 | 无 |
| 局域网对战（R2） | GGRS 同步的远端 tick 输入 | 无 |

AI、开发用确定性验证与联机复用 `MatchState.step([P1Input, P2Input])`，以相同随机种子和输入序列获得相同结果。

## 7. 规则配置与验收

`assets/data/rules/fever.ron` 应带 schema 版本和规则版本，并至少覆盖：

- 棋盘尺寸、出生格、溢出格、落下速度、锁定延迟、旋转尝试表、颜色数、颜色种子与角色 `DropSet` 表。
- `CP` / `CB` / `GB` 表、目标分 120、margin time 参数、攻击余数、单次落下上限 30、列顺序、666/999 衰减开关。
- Fever 量表容量、初始/上限时间、时间奖励、题面表、等级升降表、普通/Fever 队列合并时机、Cover-X 与归零落垃圾分支。
- `reference_profile = "fever1_2_console_pc"`、各常数的 Wiki URL、录入日期和**确定性验证样本 ID**（固定种子 + 输入日志 → checksum；非玩家回放功能）。

规则验收以这些测试为最小集合：

1. 四连消除、相邻垃圾清除、重力与多轮连锁。
2. 每个角色的 16 手掉落序列、I/L/J/O 的颜色布局、墙边旋转、地面旋转、双旋转与出生区失败。
3. 攻击余数、同 tick 双方抵消、连续抵消、未连锁垃圾落下和固定列顺序；目标分 120 与 margin 迭代抽样。
4. 七格量表、进入/退出 Fever、题面成功/失败分支、全消奖励、Fever 内溢出、时间归零翻盘。
5. 固定种子和输入日志的最终状态 checksum；AI、本地双人与 LAN 会话之间的结果一致。

## 8. 参考资料与数值出处

| 主题 | Wiki | 本项目采用的要点 |
| --- | --- | --- |
| 规则剖面与差异表 | [Fever (rule)](https://puyonexus.com/wiki/Fever_%28rule%29) | Fever 1/2 主机/PC：连续抵消、7 格、15–30s、题面 3–15、L/J 周期、Cover-X 存活翻盘、Human VS 666/999 |
| 基础棋盘与连锁 | [Basic rules](https://puyonexus.com/wiki/Basic_rules) | 6×12、≥4 消除、相邻清垃圾 |
| 分数 / 垃圾公式 | [Scoring](https://puyonexus.com/wiki/Scoring) | `(10×PC)×clamp(CP+CB+GB,1,999)`；余数累计；Fever 的 CB/GB 表 |
| 连锁倍率表 | [List of attack powers](https://puyonexus.com/wiki/List_of_attack_powers) | Fever / Fever 2 的 Fever 表；R1 共享一张中位参考档 |
| 目标分与 margin | [Margin time](https://puyonexus.com/wiki/Margin_time) | Fever 对战初始 TP=120 及衰减表 |
| 垃圾队列与单次上限 | [Nuisance queue](https://puyonexus.com/wiki/Nuisance_queue) | 1/6/30/180/360/720；单次最多 30；Fever 2 符号上限 Crown |
| 连续抵消语义 | [Offset rule](https://puyonexus.com/wiki/Offset_rule) | Fever 连续抵消 vs 经典一次抵消 |
| 掉落组 | [Dropset](https://puyonexus.com/wiki/Dropset) | 16 手；I/L/J/O；Fever 1/2 L/J 周期 |
| 旋转 | [Rotation](https://puyonexus.com/wiki/Rotation) | 墙踢、上抬、双旋转 |
| 全消 | [All clear](https://puyonexus.com/wiki/All_clear) | 普通场 4 连题面 +5s；Fever 内 +2 题面 +5s |
