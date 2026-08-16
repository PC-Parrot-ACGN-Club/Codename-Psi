# 盘面与活动组操控

**相关模块：** `game_core::board`、`game_core::piece`、`game_core::drop_stream`、`game_core::control`
**关联文档：** [玩法设计 §3](../../gameplay.md)、[规则配置与开局规格冻结](rule-configuration.md)、[统一游戏动作与 Tick 输入](game-action-input.md)、[DEC-002](../decision/timing-parameter-source.md)

## 目标

把一名玩家从 NEXT 取得活动组、经移动/旋转/下落到锁定入盘的过程定义为确定性 tick 状态机，并给出出生失败判定。

## 数据模型

### 盘面

盘面为 6 列 × 14 行，坐标原点在左上角，`y` 向下增长。`y = 0` 与 `y = 1` 是隐藏行：其中的球不计入连锁，落入可见区（`y = 2..13`）后才参与结算。可见区即玩法契约中的 6 × 12 棋盘。

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `Cell` | 一个规则格 | `Empty`、`Color(ColorId)`、`Nuisance` |
| `Board` | 全部规则格 | 列数、行数、隐藏行数与出生列来自冻结的剖面 |
| `spawn_column` | 出生列与失败判定列 | 活动组的轴心生成于该列的隐藏行 |

### 活动组

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `FallingGroup` | 尚未写入盘面的球组 | 形状、`transform_id`、轴心坐标与各球颜色；所有占格互不重复，且与已落盘格不相交 |
| `transform_id` | 几何朝向 | `0..3`，顺时针旋转为 `(id + 1) mod 4`，逆时针为 `(id + 3) mod 4` |
| `DropCursor` | 掉落序列位置 | 16 手序列下标与 L/J 周期状态 |
| `NextQueue` | 玩家可见的后续活动组 | 长度来自剖面 |
| `ControlState` | 操控计时器 | 自然下落、软降、横移重复、锁定宽限与双旋转计数 |
| `TurnId` | 落子编号 | 一名玩家本小局内单调递增 |

形状、轴心与颜色布局：

| 形状 | 球数 | 轴心 | 颜色布局 | 旋转 |
| --- | ---: | --- | --- | --- |
| `I` | 2 | 下球 | 下球第一色，上球第二色 | 绕轴心 |
| `L` / `J` | 3 | 拐角球 | 由掉落组指定：竖直两球为第一色、余球为第二色；或水平两球为第一色、余球为第二色 | 绕轴心 |
| `O` 双色 | 4 | 2×2 中心 | 下横排第一色，上横排第二色，两色必不相同 | 绕中心 |
| `O` 单色 | 4 | — | 全部同色 | 不旋转；旋转输入改为循环换色 |

### 时序参数

值以 tick 表达，来自冻结的剖面；取值来源见 [DEC-002](../decision/timing-parameter-source.md)。

| 参数 | 值 |
| --- | ---: |
| 自然下落 | 16 tick / 格 |
| 软降 | 2 tick / 格 |
| 横移输入首次重复延迟 | 8 tick |
| 横移输入重复间隔 | 2 tick |
| 横移冷却 | 1 tick |
| 同一旋转键冷却 | 1 tick |
| 锁定宽限 | 触底累计 32 tick |
| 上抬次数上限 | 8 |
| 分裂前延迟 | 轴心球 1 tick，从球 2 tick |

分裂后自由落体的时长按下落格数查表：

| 格数 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| tick | 20 | 27 | 33 | 38 | 43 | 47 | 51 | 55 | 58 | 61 | 64 | 67 | 70 |

## 行为

### 供给与出生

- 输入：`DropCursor`、玩家颜色 RNG 流、盘面。
- 处理：按游标取得本手形状与颜色布局，从 RNG 取得布局所需的新颜色，补足 `NextQueue`，再把队首组放到出生姿态——轴心位于 `spawn_column` 的隐藏行。游标推进到下一手；跨过 16 手边界且该序列的 4 球单色手数为奇数时，`L` 与 `J` 在下一个 16 手内互换。
- 输出：新的活动组与更新后的 `NextQueue`。
- 错误语义：生成时 `spawn_column` 的上格已被占据时不生成活动组，该玩家立即输掉小局。失败判定只在这一时刻发生。

`DropCursor` 与颜色 RNG 都属于可回滚状态。未进入 `NextQueue` 的未来组不对外可见。

### 一个操控 tick

- 输入：当前活动组、`ControlState`、已归一化的 `PlayerActions`。
- 处理：按 **横移 → 旋转 → 软降** 的固定顺序解释本 tick 的动作，随后推进自然下落与锁定宽限计时。左右方向成立时软降不生效。横移在输入当 tick 生效，且与上一次横移之间至少间隔一个 tick。**旋转与推回不重置自然下落计时**：被上抬的组照常按自然下落重新接地，其锁定宽限继续累计。
- 输出：新的姿态与计时器，或进入锁定。
- 错误语义：目标格非法的移动是确定性 no-op，不中止本 tick。

### 旋转

- 输入：旋转方向、当前 `transform_id`、盘面。
- 处理：
  1. 同一 tick 内顺时针与逆时针同时成立时不旋转。
  2. 求目标 `transform_id`；目标格为空则直接确认。
  3. 目标格被占（含出界）时，若轴心球位于隐藏行且目标为竖直位（上或下），则不旋转——这防止在隐藏行内把整组顶上去。
  4. 检查对侧格（目标 `transform_id` 与 `2` 异或）：对侧格为空时把整组推向对侧后确认——触底时上推，贴墙或贴列时侧推。
  5. 对侧格也被占（组夹在两列之间）时，双旋转计数器自增：结果为奇数则放弃本次旋转并保留计数；结果为偶数则放行 180° 翻转，翻转方向由放行这一次的输入方向决定。
- 输出：新的 `transform_id` 与推回后的坐标。
- 错误语义：任一不旋转分支都是 no-op。

确认旋转时把双旋转计数器重置到最近的偶数。单色 `O` 不参与上述流程，其旋转输入循环切换本组颜色。

### 锁定

- 输入：接地的活动组、`ControlState`。
- 处理：硬降把组落到当前列可达的最低合法位置并立即锁定。自然下落或软降接地后开始累计锁定宽限；按住软降跳过宽限立即锁定；组被旋转推回上抬达到上限次数时也立即锁定。锁定时把全部球原子写入盘面；失去支撑的球在分裂延迟后按下落格数表自由落体。
- 输出：写入盘面的球格与 `GroupLocked` 事实。
- 错误语义：任一目标格非法时不写入部分球。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `LockedMatchSpec` | `game_core::match_spec` | 本主题 | 盘面几何、出生姿态、掉落组与时序参数 |
| `PlayerActions` | `game_core::input` | 本主题 | 已归一化的本 tick 动作位集 |
| 活动组与 `NextQueue` 快照 | 本主题 | 表现层、AI | 只读；不含 RNG 状态与未入队的未来组 |
| `GroupLocked`、出生失败 | 本主题 | 连锁结算、小局裁决 | 已发生的领域事实 |

## 边界

- 本文不定义消除、重力与连锁结算（见[玩法设计 §3.5](../../gameplay.md)）。
- 本文不定义动作的设备采集与归一化（见[统一游戏动作与 Tick 输入](game-action-input.md)）。
- 本文不定义配置的解析、校验与冻结（见[规则配置与开局规格冻结](rule-configuration.md)）。
- 本文不定义时序参数的取值来源与其风险（见 [DEC-002](../decision/timing-parameter-source.md)）。
- 本文不定义渲染、动画与音效（见[表现与 UI 设计](../../presentation.md)）。

## Test Basis

- [玩法设计 §3.1、§3.2、§3.4](../../gameplay.md)：盘面与失败判定、掉落组形状与颜色布局、可执行的六个规则动作。
- [Dropset](https://puyonexus.com/wiki/Dropset)：四种形状的颜色布局，以及每 16 手按 4 球单色手数奇偶互换 L/J。
- [Puyo Puyo Tsu/Falling Pair Control](https://puyonexus.com/wiki/Puyo_Puyo_Tsu/Falling_Pair_Control)：输入按横移、旋转、软降的顺序处理，左右方向成立时软降停止。
- [Puyo Puyo Tsu/Rotation, collision and push back](https://puyonexus.com/wiki/Puyo_Puyo_Tsu/Rotation,_collision_and_push_back)：目标格、隐藏行、对侧格与双旋转计数器的完整判定顺序。
- [Puyo Puyo Tsu/Falling Pair Spawning Process](https://puyonexus.com/wiki/Puyo_Puyo_Tsu/Falling_Pair_Spawning_Process)：生成时检查出生列上格，被占则该玩家判负。
- [Puyo Puyo Tsu/Frame Data Tables](https://puyonexus.com/wiki/Puyo_Puyo_Tsu/Frame_Data_Tables)：下落、软降、输入重复、锁定宽限、分裂与自由落体的帧数。
- [Issue #12](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/12)：要求 I/L/J/O、NEXT、移动、旋转、下落、锁定与出生。
