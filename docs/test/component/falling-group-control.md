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

- 两个角色各跑完至少两个 16 手周期，NEXT 与实际出生序列一致；4 球单色手数为奇数时第二个 16 手内 L 与 J 互换（TC-001～TC-002）。
- I/L/J/O 的所有朝向都不穿墙、不穿盘面、不产生重复占格（TC-003）。
- 单色 `O` 的旋转输入循环换色且不改变占格（TC-004）。
- 旋转的五个分支各自可复现：目标格为空直接确认、隐藏行内竖直旋转被拒、对侧格为空时上推与侧推、夹在两列间的奇偶计数、确认时计数器重置到最近偶数（TC-005～TC-009）。
- 出生列上格被占时不生成活动组并判负；该判定只在生成时刻发生（TC-010～TC-011）。
- 锁定后失去支撑的球按分裂延迟与下落格数表自由落体（TC-012）。
- 锁定宽限累计 32 tick；按住软降立即锁定；上抬达到 8 次立即锁定（TC-013～TC-014）。
- 自然下落 16 tick 每格、软降 2 tick 每格、横移输入重复 8/2 tick、横移冷却 1 tick（TC-015～TC-016）。

### Component — Rules；Input

- 同一 tick 内横移、旋转、软降同时成立时按横移、旋转、软降的顺序生效；左右方向成立时软降不生效（TC-017）。
- 固定种子与动作日志得到相同的锁定坐标与 NEXT（Concern: Determinism；TC-018）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 状态迁移 | 出生→操控→接地→锁定的阶段与守卫条件 | TC-010～TC-014 |
| 判定表 | 旋转五个分支的目标格、隐藏行、对侧格与计数奇偶；同 tick 动作组合 | TC-005～TC-009、TC-017 |
| 边界值分析 | 宽限 31/32 tick、上抬 7/8 次、下落格数表首尾、贴墙列 `x=0`/`x=5` | TC-003、TC-013～TC-016 |
| 性质测试 | 任意朝向下占格互不重复且落在盘面内 | TC-003 |
| 等价类划分 | 双色形状与单色 `O` 的旋转语义分支 | TC-004 |
| 变形测试 | 相同种子与动作日志重复推进得到相同结果 | TC-018 |
| 场景法 | 跨 16 手边界的完整供给序列 | TC-001～TC-002 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | NEXT 展示的顺序与实际出生的活动组逐手一致 | P0 | Component | — | Rules | 已冻结的角色掉落组与颜色 RNG 流，`NextQueue` 长度取自剖面 | 连续硬降 32 手，每手在生成前记录 `NextQueue` 队首、生成后记录实际活动组 | 参数化角色 A（I×11、L·J×3、O 单色×1、O 双色×1）与角色 B（I×12、L·J×1、O 单色×1、O 双色×2）；空盘 | 32 手中每一手的实际形状与颜色布局都等于生成前 `NextQueue` 队首；队列在每次生成后补足到剖面长度；未入队的未来手不可从读模型取得 | [Confirmed] [盘面与活动组操控：供给与出生](../../development/design/board-and-falling-group.md#供给与出生) |
| TC-002 | 4 球单色手数为奇数时第二个 16 手内 L 与 J 互换 | P1 | Component | — | Rules | 同 TC-001 | 连续硬降 32 手，比较第 1～16 手与第 17～32 手的形状序列 | 角色 A 与角色 B（4 球单色手数均为 1，奇数）；对照序列：4 球单色手数为 2 的构造掉落组 | 两名角色第 17～32 手中原为 `L` 的手变为 `J`、原为 `J` 的手变为 `L`，其余手不变，实际周期为 32 手；对照序列第 17～32 手与第 1～16 手完全相同，周期为 16 手 | [Confirmed] [盘面与活动组操控：供给与出生](../../development/design/board-and-falling-group.md#供给与出生)；[玩法设计 §3.2](../../gameplay.md) |
| TC-003 | 四种形状的每个朝向都产生合法且互不重复的占格 | P1 | Component | — | Rules | 空盘 6 列 × 14 行 | 对每个形状枚举 `transform_id` 0～3 与全部合法轴心列，读取占格集合 | `I`、`L`、`J`、`O` 双色；轴心列 `x=0`～`x=5`；轴心行取隐藏行与可见区各一 | 每个合法姿态的占格互不重复、全部落在 `0 ≤ x ≤ 5`、`0 ≤ y ≤ 13` 内且不与已落盘格相交；越出盘面的姿态不被判为合法姿态 | [Confirmed] [盘面与活动组操控：活动组](../../development/design/board-and-falling-group.md#活动组) |
| TC-004 | 单色 O 的旋转输入循环换色且不改变占格 | P2 | Component | — | Rules | 活动组为单色 `O`，颜色数取剖面值 4 | 连续发出 5 次顺时针旋转，每次记录颜色与占格 | 初始颜色 `c0`；剖面颜色域 4 色 | 五次输入后颜色按固定顺序循环并在第 5 次回到 `c0`；`transform_id` 与四个占格坐标始终不变 | [Confirmed] [盘面与活动组操控：旋转](../../development/design/board-and-falling-group.md#旋转)；[玩法设计 §3.2](../../gameplay.md) |
| TC-005 | 目标格为空时旋转直接确认 | P1 | Component | — | Rules | 活动组 `I` 位于可见区中部，四周为空 | 分别发出顺时针与逆时针旋转 | 轴心 `(x=2, y=6)`；`transform_id` 初值 0 | 顺时针后 `transform_id=1`、逆时针后回到 0；轴心坐标不变；不触发推回，双旋转计数器不变 | [Confirmed] [盘面与活动组操控：旋转](../../development/design/board-and-falling-group.md#旋转) |
| TC-006 | 轴心位于隐藏行时竖直目标位的旋转被拒 | P1 | Component | — | Rules | 活动组轴心位于隐藏行且目标竖直位被占或出界 | 发出使目标 `transform_id` 为竖直位的旋转 | 轴心 `(x=2, y=1)`；目标为上位；`x=2` 列 `y=0` 已被占 | 不旋转、不上推、不侧推，`transform_id` 与坐标均不变；该 no-op 不中止本 tick 的其余处理 | [Confirmed] [盘面与活动组操控：旋转](../../development/design/board-and-falling-group.md#旋转) |
| TC-007 | 目标格被占而对侧格为空时按触底上推或贴墙侧推 | P1 | Component | — | Rules | 分别构造触底与贴墙两种局面 | 发出使目标格落在被占格的旋转 | 触底：轴心 `(x=2, y=13)`，目标为下位；贴墙：轴心 `(x=0, y=6)`，目标为左位 | 触底组整体上推一格后确认旋转，轴心变为 `(x=2, y=12)`；贴墙组整体右推一格后确认，轴心变为 `(x=1, y=6)`；两者 `transform_id` 均更新为目标值 | [Confirmed] [盘面与活动组操控：旋转](../../development/design/board-and-falling-group.md#旋转) |
| TC-008 | 组夹在两列之间时由双旋转计数器的奇偶决定是否放行 | P1 | Component | — | Rules | 活动组竖直位于两侧均被占的列 | 连续发出两次同方向旋转，每次记录姿态与计数器 | 轴心 `(x=2, y=6)`；`x=1` 与 `x=3` 列在 `y=5`～`y=7` 均为实体格；计数器初值 0 | 第一次输入后计数器为 1（奇数），姿态不变；第二次输入后计数器为 2（偶数），放行 180° 翻转，翻转方向由第二次输入的方向决定 | [Confirmed] [盘面与活动组操控：旋转](../../development/design/board-and-falling-group.md#旋转) |
| TC-009 | 确认旋转时把双旋转计数器重置到最近的偶数 | P2 | Component | — | Rules | 计数器已被夹住的局面推到奇数 | 使组脱离夹住局面后发出一次可直接确认的旋转 | 计数器为 3；随后横移到四周为空的列并旋转 | 该次旋转确认后计数器为 2；后续在夹住局面下的第一次输入使其变为 3（奇数）并放弃旋转，奇偶语义未被打乱 | [Confirmed] [盘面与活动组操控：旋转](../../development/design/board-and-falling-group.md#旋转) |
| TC-010 | 生成时出生列上格被占则不生成活动组并判负 | P0 | Component | — | Rules | 上一活动组已锁定，即将生成下一组 | 触发供给与出生 | 剖面 `spawn_column=2`；`x=2` 列自 `y=13` 堆满至出生姿态所需格被占据 | 不产生活动组、不写入盘面；产出该玩家的出生失败事实；`NextQueue` 与 `DropCursor` 不因失败而推进 | [Confirmed] [盘面与活动组操控：供给与出生](../../development/design/board-and-falling-group.md#供给与出生)；[玩法设计 §3.1](../../gameplay.md) |
| TC-011 | 出生列上格已被占但尚未到生成时刻时不判负 | P1 | Component | — | Rules | 同 TC-010 的堆积状态，且当前仍有活动组在操控 | 推进若干操控 tick 后再让当前组锁定并触发下一次生成 | 堆积完成后再推进 60 tick；随后硬降当前组 | 推进期间不产生出生失败；失败事实只在锁定后的那一次生成时刻产生，且恰好一次 | [Confirmed] [盘面与活动组操控：供给与出生](../../development/design/board-and-falling-group.md#供给与出生) |
| TC-012 | 锁定后失去支撑的球按分裂延迟与下落格数表落到位 | P1 | Component | — | Rules | 盘面在相邻两列存在高度差 | 让横置的 `I` 锁定在跨越高度差的位置并推进 tick | 轴心球落在实体支撑上；从球下方空 3 格；分裂延迟轴心 1 tick、从球 2 tick；3 格自由落体 33 tick | 轴心球保持锁定坐标；从球在第 2 tick 开始下落、在第 35 tick 到达目标格；到位前该球不参与连锁扫描；到位后盘面无悬空格 | [Confirmed] [盘面与活动组操控：时序参数](../../development/design/board-and-falling-group.md#时序参数)、[锁定](../../development/design/board-and-falling-group.md#锁定) |
| TC-013 | 接地后锁定宽限累计到 32 tick 才锁定，上抬不重置累计 | P0 | Component | — | Rules | 活动组接地且四周允许横移与旋转 | 分别推进到宽限第 31、32 tick；另构造在第 20 tick 被旋转上推后重新接地的序列 | 宽限 32 tick；上抬序列：接地 20 tick → 上推 → 自然下落 16 tick 重新接地 → 再 12 tick | 第 31 tick 仍可横移与旋转、未锁定；第 32 tick 锁定并写入盘面；上抬序列在重新接地后的第 12 tick（累计 32）锁定，累计值不被上抬清零 | [Confirmed] [盘面与活动组操控：一个操控 tick](../../development/design/board-and-falling-group.md#一个操控-tick)、[锁定](../../development/design/board-and-falling-group.md#锁定) |
| TC-014 | 按住软降与上抬达到上限时立即锁定 | P1 | Component | — | Rules | 活动组接地且允许被旋转上推 | 参数化两条路径：接地后按住软降；连续旋转推回直至达到上抬上限 | 软降路径：接地后第 1 tick 保持 `SoftDrop`；上抬路径：上抬次数 7 与 8 | 软降路径在按住的当 tick 锁定，不再累计宽限；上抬路径在第 7 次上抬后仍可操控，第 8 次上抬达到上限后立即锁定 | [Confirmed] [盘面与活动组操控：锁定](../../development/design/board-and-falling-group.md#锁定) |
| TC-015 | 自然下落与软降按配置速率推进 | P1 | Component | — | Rules | 空盘，活动组自出生姿态开始 | 分别在无输入与持续 `SoftDrop` 下推进 tick 并记录轴心行 | 自然下落 16 tick/格，推进 48 tick；软降 2 tick/格，推进 6 tick | 无输入时第 16、32、48 tick 各下落一格，中间 tick 不移动；持续软降时第 2、4、6 tick 各下落一格；两种速率不叠加 | [Confirmed] [盘面与活动组操控：时序参数](../../development/design/board-and-falling-group.md#时序参数) |
| TC-016 | 横移输入按 8/2 tick 重复且两次横移至少间隔 1 tick | P1 | Component | — | Rules | 空盘，活动组轴心位于最右列 | 持续按住 `Left` 推进 16 tick；另在轴心位于中部时逐 tick 点按 `Left` 推进 4 tick | 轴心自 `x=5` 起；首次重复延迟 8 tick、重复间隔 2 tick、横移冷却 1 tick | 按住路径在第 0、8、10、12、14 tick 各左移一格并在第 14 tick 到达 `x=0`，第 16 tick 的重复因贴墙成为 no-op，其余 tick 不移动；点按路径只在第 1、3 tick 移动，第 2、4 tick 因冷却不生效 | [Confirmed] [盘面与活动组操控：时序参数](../../development/design/board-and-falling-group.md#时序参数)、[一个操控 tick](../../development/design/board-and-falling-group.md#一个操控-tick) |
| TC-017 | 同一 tick 的动作按横移、旋转、软降顺序生效且方向键抑制软降 | P0 | Component | — | Rules；Input | 活动组位于可横移、可旋转、下方有空格的位置 | 参数化提交四种同 tick 归一化动作组合并推进一个 tick | `Left+RotateCW+SoftDrop`；`Left+SoftDrop`；`RotateCW+SoftDrop`；仅 `SoftDrop` | 组合一先左移再以移动后的列判定旋转，且不软降；组合二只左移；组合三先旋转再软降一格；组合四只软降一格；四组均在同一 tick 内完成，无跨 tick 拆分 | [Confirmed] [盘面与活动组操控：一个操控 tick](../../development/design/board-and-falling-group.md#一个操控-tick) |
| TC-018 | 相同种子与动作日志得到相同的锁定坐标与 NEXT | P0 | Component | Determinism | Rules；Input | 两份以相同 `LockedMatchSpec` 与根种子初始化的操控状态 | 对两份状态回放同一份逐 tick 动作日志 | 固定种子 `0x1`；角色 A；含横移、旋转、软降、硬降与自然锁定的 200 tick 日志 | 两份状态的每一手锁定坐标、锁定 tick、`DropCursor`、L/J 周期状态与 `NextQueue` 内容逐项相等；日志分两段连续回放得到与一次回放相同的结果 | [Confirmed] [盘面与活动组操控：供给与出生](../../development/design/board-and-falling-group.md#供给与出生)；[TDD §3](../../TDD.md) |

## 风险查漏

供给序列与 L/J 周期、四形状各朝向的占格合法性、旋转五个分支与计数器语义、出生失败的时机、分裂自由落体、三条锁定路径、四项时序参数与同 tick 动作顺序均有直接用例；确定性由 TC-018 的回放与分段推进双向保护。
