# 玩法设计：Fever 连锁对战

**文档定位：** 立项时制定的玩法契约。描述对局流程、规则轮廓与原作数值默认来源，作为规则引擎开发至少应覆盖的范围；开发与校准时可修订条文或配置，修订时同步更新本文档、`RuleProfile` 与验证样本。  
**产品范围：** R1 本地对战与 AI，R2 局域网对战共用同一规则引擎  
**关联：** [PRD](PRD.md)、[TDD](TDD.md)

## 1. 目的与参考范围

本文件描述本项目的对局流程和规则边界。玩法以 Puyo Nexus Wiki 所记录的 **Fever 规则**为参考，使用独立的原创角色、名称、视觉、音频和界面表达。

参考剖面默认为 **Puyo Puyo Fever / Fever 2 的主机与 PC 规则轮廓**。Wiki 记录了多个作品和平台在 Fever 时间、题面、垃圾顺序与失败处理上的差异；一场对局运行时以当时锁定的版本化 `RuleProfile` 为准，同一场内保持单一规则剖面。[Fever 规则差异表](https://puyonexus.com/wiki/Fever_%28rule%29)

**数值默认策略：** `RuleProfile` 与内容库中的可调常数默认录入该参考剖面的原作数值；来源页见各节 Wiki 链接与 §8。优先在配置层校准数值，并保留引用与验证样本 ID；契约层变更时更新本节。

Fever 系列的角色选择包含可影响对局的掉落组与连锁强度。两位原创角色各自绑定一个数据化 `DropSet` 和一个 `ChainPowerProfile`；`ChainPowerProfile` 分别定义普通盘与 Fever 盘的逐连锁倍率曲线。双方共享同一 `RuleProfile`，角色差异只来自开局选定的角色规则定义。角色 UI、掉落组和连锁强度配置都在开局前锁定，并写入联机握手与开发用确定性验证元数据。特殊球种、道具和额外竞技模式属于当前范围外内容。面向玩家的对局回放不在产品范围（见 [PRD](PRD.md) §2.3）；下文「输入日志 / 确定性验证」仅指开发与 CI。

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

- 每名玩家使用独立的 6 列 × 12 行可见棋盘，其上方另有隐藏行用于出生与缓冲；隐藏行中的球不计入连锁，落入可见区后才参与结算。
- 活动组生成于出生列的隐藏行。生成下一组时若出生列上格已被占据，该玩家立即输掉小局；失败判定只在生成时刻发生。
- 棋盘只保存规则格子。渲染球体、缩放、粒子和 UI 指示由客户端从规则快照生成。

Wiki 的通用规则采用 6 × 12 格棋盘，且以标记的出生/失败格作为失败判定；该格即出生点。[Basic rules](https://puyonexus.com/wiki/Basic_rules) 生成失败即判负的判定时机来自逆向工程记录。[Falling Pair Spawning Process](https://puyonexus.com/wiki/Puyo_Puyo_Tsu/Falling_Pair_Spawning_Process)

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

`DropSet` 是 16 手的循环序列。每一手保存形状和同色/双色布局；颜色随机流及其当前位置属于对局状态，不属于静态掉落组：

| 形状 | 球数 | 规则表达 |
| --- | ---: | --- |
| `I` | 2 | 纵向双球，轴心在下。 |
| `L` / `J` | 3 | 拐角三球；配置决定哪一条边使用第一颜色。 |
| `O` | 4 | 2×2 方块；可为单色或上下两色。单色 `O` 的旋转输入用于切换颜色。 |

Fever 1/2 的 L/J 数量与落点位置属于角色序列的一部分；部分角色每 16 手切换 L 与 J。15th 及后续 Fever 作品保留角色序列，同时移除了该 L/J 周期差异。[Dropset 细节](https://puyonexus.com/wiki/Dropset)

两个原创角色各自配置一个原创 16 手序列并以 `drop_set_id` 关联；不复刻既有角色名称、完整序列或颜色种子。每个序列记录 2/3/4 球数量、三球颜色布局和单色/双色 O。

两个序列的 16 手总球量相同。L/J 周期不是独立配置项：每跨过 16 手边界，4 球单色手数为奇数的角色在下一个 16 手内互换 L 与 J，因此实际序列周期为 32 手；该手数为偶数时周期为 16 手。周期由校验器从序列本身推导。

### 3.3 角色连锁强度（ChainPowerProfile）

角色玩法数据使用以下关系：

```text
CharacterRuleDefinition
├─ character_id
├─ drop_set_id ─────────────→ DropSet
└─ chain_power_profile_id ──→ ChainPowerProfile
                                ├─ normal[]
                                └─ fever[]
```

`CharacterRuleDefinition` 是角色选择进入规则核心后的稳定身份；`DropSet` 和 `ChainPowerProfile` 是它引用的两个独立玩法组成。每个 `ChainPowerProfile` 包含两条相互独立的逐连锁倍率曲线：

| 曲线 | 使用时机 | 索引与约束 |
| --- | --- | --- |
| `normal` | 普通盘发生的连锁步 | 以从 1 开始的当前连锁步索引；超过表尾时使用表尾值 |
| `fever` | Fever 盘发生的连锁步 | 以从 1 开始的当前连锁步索引；超过表尾时使用表尾值 |

两位原创角色分别拥有独立的 `ChainPowerProfile`，普通盘和 Fever 盘都可以形成角色差异。profile 是角色玩法身份的一部分；即使两条曲线的某些或全部数值相同，也必须由数据显式表达，不能回退到隐藏的全局角色倍率。掉落组控制每 16 手获得的形状、球量和布局，连锁强度控制各连锁步的 `CP`；两者共同定义角色规则能力，不能用其中一项隐式推导另一项。

倍率曲线以 Fever / Fever 2 的角色分档为数值参考，但两个原创角色使用原创曲线。每条曲线是一条定长 24 项的整数表，由一条全角色共享的形状曲线与该角色的强度、倾斜参数采样生成。配置同时保存整数表与生成参数：整数表是权威数据，参数是来源信息，另记录来源档位、校准样本和版本。平衡调整修改角色参数并重新生成整张表，不手改单格，也不修改计分公式。角色选择、对局快照、联机握手和确定性验证元数据锁定 `character_id`、`drop_set_id`、`chain_power_profile_id` 及其内容摘要。

### 3.4 下落操作

- 每个回合按当前角色 `DropSet` 生成一组 2、3 或 4 球的下落组；颜色来自本局锁定的确定性随机序列和该手的颜色布局。
- 玩家可执行左移、右移、软降、硬降、顺时针旋转和逆时针旋转。
- 左右移动、旋转和下落只在目标格有效时生效；硬降将球对落到当前列可达的最低合法位置并进入锁定流程。
- 旋转以形状的轴心为中心。目标格被占时检查其对侧格：对侧为空则把整组推向对侧——触底上推、贴墙或贴列侧推；两侧都被占时由双旋转计数器决定是否放行 180° 翻转。判定顺序见[盘面与活动组操控](development/design/board-and-falling-group.md)。

地面旋转、墙边旋转与双旋转属于系列通用旋转机制。[Rotation](https://puyonexus.com/wiki/Rotation) 完整判定顺序与推回方向来自逆向工程记录。[Rotation, collision and push back](https://puyonexus.com/wiki/Puyo_Puyo_Tsu/Rotation,_collision_and_push_back)

### 3.5 锁定与连锁结算

一次锁定后的结算必须完整执行，再生成下一球对：

1. 对活动球对应用重力并写入棋盘。
2. 查找横向或纵向相连、数量至少为 4 的同色普通球。
3. 同时移除本轮命中的全部普通球，以及与它们四向相邻的垃圾球。
4. 对剩余球应用重力。
5. 若重力后又出现可消组，进入下一连锁轮；直到棋盘稳定。

连锁数等于本次锁定中发生的消除轮数。普通球达到四向连接的四个或以上即可消除；垃圾球通过与普通球消除组相邻而清除。[Basic rules](https://puyonexus.com/wiki/Basic_rules)

#### 结算计算与阶段提交

规则层可以在进入一轮结算时立即计算该轮的消除集合、重力目标和后续盘面，但这些结果必须按固定 tick 阶段提交。结算阶段提供确定的可读节拍，使表现层能够播放消除和重力动画，同时不取得规则推进权：

```text
Lock
  → ClearPreview
  → ClearCommit
  → Gravity
  → ScanNext
       ├─ 有下一连锁 → ClearPreview
       └─ 无下一连锁 → Settlement
  → SpawnNext
```

| 阶段 | 规则状态 | 表现可观察内容 | 阶段结束动作 |
| --- | --- | --- | --- |
| `ClearPreview` | 盘面仍保留本轮待消球；保存待消普通球、相邻垃圾、连锁步和剩余 tick | 待消坐标、连锁步、阶段进度 | 原子删除待消球并产生本连锁步的分数、攻击和清除事实 |
| `ClearCommit` | 盘面处于已清除、尚未重力稳定的状态 | 清除已经发生的边界 | 计算每颗剩余球的重力移动及目标盘面 |
| `Gravity` | 保存重力前盘面、各球起终点、目标盘面和剩余 tick | 每颗球的起点、终点和阶段进度 | 原子提交目标盘面 |
| `ScanNext` | 盘面为已提交的稳定盘面 | 本轮重力完成 | 扫描下一可消组，或形成完整连锁报告 |
| `Settlement` | 连锁报告已经完整，等待攻防、Fever、垃圾与失败安全点结算 | 最终连锁数与结算结果 | 进入下一规则阶段或生成下一掉落组 |

`ClearPreview` 和 `Gravity` 的持续时间来自开局锁定的 `RuleProfile`，使用整数 tick 表达并进入规则配置摘要、快照和状态校验值。重力持续时间按本轮最大下落格数查配置中的下落时长表；同一规则配置与盘面必须得到相同持续时间。玩家自身处于任一 Resolve 阶段时，其落子动作输入不修改盘面；输入按正常 tick 消费，不延迟到下一活动组。

表现层只读取当前阶段、起终状态、`elapsed_ticks` 和 `duration_ticks`；不得以“动画播放完成”回调推进规则。表现帧率不足时跳到规则指定的最新阶段进度，关闭或降低动画强度时也不能缩短规则阶段。无窗口模拟、AI、本地对局与网络对局均按同一 tick 状态机推进。

双方分别保存自己的活动组与结算阶段。一方处于 `Resolving` 时，另一方仍可处于 `Normal` 或 `Fever` 并继续操控。连锁分数、攻击和抵消只在对应 `ClearCommit` tick 逐步生效；即使规则提前算出完整连锁，也不能在第一轮预览时一次性发布后续连锁的攻击或通过公开状态泄露尚未提交的结果。

Fever 时间和 margin time 不因结算演出窗口暂停。Fever 时间在结算阶段归零时标记退出待处理，已经开始的当前连锁继续结算到 `Settlement` 安全点；随后退出 Fever，不生成下一题面。该规则保证连锁节拍不会由表现控制，也不会让长连锁演出无限延长 Fever。

## 4. 分数、攻击与垃圾

### 4.1 分数与攻击点

每轮分数与垃圾换算采用 Wiki [Scoring](https://puyonexus.com/wiki/Scoring) 公式；连锁倍率（Chain Power）表来自 [List of attack powers](https://puyonexus.com/wiki/List_of_attack_powers)。

```text
score = (10 × PC) × clamp(CP + CB + GB, 1, 999)
```

| 符号 | 含义 | 来源 |
| --- | --- | --- |
| `PC` | 本轮消除的普通球数 | [Scoring](https://puyonexus.com/wiki/Scoring) |
| `CP` | 按当前角色、当前盘面模式和连锁步，从已锁定 `ChainPowerProfile.normal` 或 `.fever` 取得的倍率；表值为 0 时按 1 | [List of attack powers](https://puyonexus.com/wiki/List_of_attack_powers) |
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

**攻击换算不区分对手类型：** 单人、本地双人与局域网对局使用同一套换算，`SC` 直接进入上述公式。

### 4.2 抵消、入队与落下

- 同一结算帧先计算双方的总攻击，再各自抵消自己的待接收垃圾；余量进入对手的待接收垃圾队列。
- 玩家完成任意连锁且队列中有待接收垃圾时，连锁攻击优先用于抵消该队列。
- Fever 规则采用连续抵消：本回合触发连锁时，尚未抵消的垃圾继续留在队列，玩家获得下一球对；本回合未触发连锁时，待接收垃圾落入棋盘。[Fever 规则](https://puyonexus.com/wiki/Fever_%28rule%29) [Offset rule](https://puyonexus.com/wiki/Offset_rule)
- 单次落下上限默认 **30**（一整块 Rock = 五整行）；列序采用 Fever 系固定顺序（非整随机）。凑满整行后余 **1** 颗时，下一次落下的首颗从下一列开始；余 **2 颗及以上**时，下一次落下的首颗从上一颗所在列开始。[Nuisance queue](https://puyonexus.com/wiki/Nuisance_queue) [Fever 规则 § Nuisance Order](https://puyonexus.com/wiki/Fever_%28rule%29)
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
5. Fever 时间到零：主机/PC 为**存活并翻回普通盘**（部分掌机为直接判负）。若玩家正在 Resolve，按 §3.5 完成已经开始的当前连锁，并在 `Settlement` 安全点翻回普通盘；否则在当前安全点翻回。Fever 队列合并回普通队列；若归零瞬间未抵消且任一队列有垃圾，翻回后垃圾立即落下。[Fever 规则差异表](https://puyonexus.com/wiki/Fever_%28rule%29)

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
- 双方在同一判定时点同时失败时该小局判和：比分不变，以同一局号重打，重打使用与上一次不同的随机序列。一方先失败、另一方在之后的时点失败按正常胜负结算。
- 比赛只在某一方先胜两局时结束；和局不计入比分，小局数不设上限。

### 6.2 对手类型

| 模式 | 对手输入来源 | 规则差异 |
| --- | --- | --- |
| 单人 | AI 通过合法动作选择球对位置和旋转 | 无 |
| 本地双人 | 同一设备的 P1 / P2 输入映射 | 无 |
| 局域网对战（R2） | GGRS 同步的远端 tick 输入 | 无 |

AI、开发用确定性验证与联机复用 `MatchState.step([P1Input, P2Input])`，以相同随机种子和输入序列获得相同结果。

## 7. 规则配置

竞技规则数值集中写入版本化配置，分为两部分：**规则剖面**描述一套规则怎么算，**内容库**描述有哪些角色和素材可选。两者变更频率不同，各自独立版本化。

规则剖面带 schema 版本、规则版本和 `reference_profile = "fever1_2_console_pc"`，至少覆盖：

- 棋盘尺寸与隐藏行数、出生列与出生姿态、自然下落与软降速度、横移输入重复、锁定宽限、上抬次数上限、颜色数、颜色种子。
- 结算阶段的消除预览时长、清除提交边界，以及按下落格数查表的重力时长与分裂延迟。
- 共享的 `CB` / `GB` 表、目标分 120、margin time 的目标分衰减表、攻击余数、单次落下上限 30、列顺序。
- Fever 量表容量、初始/上限时间、时间奖励、题面目标等级域、等级升降表、普通/Fever 队列合并时机、Cover-X 与归零落垃圾分支。
- 各常数的 Wiki URL、录入日期和**确定性验证样本 ID**（固定种子 + 输入日志 → checksum；非玩家回放功能）。

内容库按剖面分区保存玩法数据与素材：每个角色的 `DropSet`、普通盘与 Fever 盘 `CP` 曲线及其生成参数，以及 Fever 题面表。角色身份与其玩法数据分开保存，`character_id`、`drop_set_id`、`chain_power_profile_id` 及各自内容摘要在开局前锁定。

所有影响结果的时长以 tick 写入配置，秒只出现在注释里；由参数推导的数值表（margin 目标分衰减、`CP` 曲线）以整数表写入，表是权威数据、生成参数是来源信息，规则运行期不做实时换算。所有时长与数值表都进入确定性规则摘要。

规则数据不可用时不存在权威规则依据，因此不进入对局，也不使用内置默认规则数据。

规则的可判定验收条目见[测试用例设计](test/design/README.md)。

## 8. 参考资料与数值出处

| 主题 | Wiki | 本项目采用的要点 |
| --- | --- | --- |
| 规则剖面与差异表 | [Fever (rule)](https://puyonexus.com/wiki/Fever_%28rule%29) | Fever 1/2 主机/PC：连续抵消、7 格、15–30s、题面 3–15、L/J 周期、Cover-X 存活翻盘 |
| 基础棋盘与连锁 | [Basic rules](https://puyonexus.com/wiki/Basic_rules) | 6×12、≥4 消除、相邻清垃圾 |
| 分数 / 垃圾公式 | [Scoring](https://puyonexus.com/wiki/Scoring) | `(10×PC)×clamp(CP+CB+GB,1,999)`；余数累计；Fever 的 CB/GB 表 |
| 连锁倍率表 | [List of attack powers](https://puyonexus.com/wiki/List_of_attack_powers) | Fever / Fever 2 按角色分别提供普通盘与 Fever 盘曲线；原创角色各自配置两条曲线 |
| 目标分与 margin | [Margin time](https://puyonexus.com/wiki/Margin_time) | Fever 对战初始 TP=120 及衰减表 |
| 垃圾队列与单次上限 | [Nuisance queue](https://puyonexus.com/wiki/Nuisance_queue) | 1/6/30/180/360/720；单次最多 30；Fever 2 符号上限 Crown |
| 连续抵消语义 | [Offset rule](https://puyonexus.com/wiki/Offset_rule) | Fever 连续抵消 vs 经典一次抵消 |
| 掉落组 | [Dropset](https://puyonexus.com/wiki/Dropset) | 16 手；I/L/J/O；Fever 1/2 L/J 周期 |
| 旋转 | [Rotation](https://puyonexus.com/wiki/Rotation) | 墙踢、上抬、双旋转 |
| 活动组操控与帧数据 | [Puyo Puyo Tsu 逆向工程](https://puyonexus.com/wiki/Puyo_Puyo_Tsu/Frame_Data_Tables) | 输入优先级、旋转与推回判定、下落/软降/锁定宽限/自由落体帧数；来源为 Tsu 而非 Fever，标注见 DEC-002 |
| 全消 | [All clear](https://puyonexus.com/wiki/All_clear) | 普通场 4 连题面 +5s；Fever 内 +2 题面 +5s |
