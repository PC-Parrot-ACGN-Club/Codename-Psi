# 连锁结算

**相关模块：** `game_core::resolution`
**关联文档：** [玩法设计 §3.5](../../gameplay.md)、[盘面与活动组操控](board-and-falling-group.md)、[小局、BO3 与安全点](match-and-round.md)、[DEC-004](../decision/settlement-timing-values.md)

## 目标

把一次锁定后的盘面推进到稳定状态，并产出每个连锁步的完整事实，供攻防、Fever 与小局裁决消费。

## 数据模型

```text
ResolutionState
├─ Idle
├─ ClearPreview { link_facts, elapsed_ticks, duration_ticks }
├─ ClearCommit  { link_facts }
├─ Gravity      { moves, target_board, elapsed_ticks, duration_ticks }
├─ ScanNext     { next_chain_index }
└─ Settlement   { report }
```

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `ColorGroup` | 四向连通、颜色相同的一组普通球 | 只在可见区内成立 |
| `ChainLinkFacts` | 一个连锁步的事实 | 本步编号、被清普通球、相邻清除垃圾、颜色数、各组大小 |
| `ChainReport` | 一次落子的完整结算结果 | 全部连锁步、总清除数、最终盘面事实 |
| `FieldFacts` | 稳定盘面的只读结论 | `all_clear`；不直接决定跨领域奖励或胜负 |

`Settlement` 是终态。`ClearCommit` 是零时长边界，其余四态都跨 tick 保存，因此结算过程可被快照与恢复。

**隐藏行不参与结算。** 按[盘面几何](board-and-falling-group.md#盘面)，`y = 0` 与 `y = 1` 的球不计入连锁：扫描连通组时排除这两行，相邻垃圾清除不跨入这两行，`all_clear` 也以可见区为空成立。

### 阶段时长

| 阶段 | 时长 |
| --- | --- |
| `ClearPreview` | 12 tick |
| `ClearCommit` | 0 tick（边界） |
| `Gravity` | 按本轮最大下落格数查表 |

重力使用与[分裂自由落体](board-and-falling-group.md#时序参数)相同的参数组，因此共用同一张格数时长表。取值来源与其校准地位见 [DEC-004](../decision/settlement-timing-values.md)。

## 行为

### 扫描与清除

- 输入：稳定盘面、当前连锁步编号。
- 处理：
  1. 以固定坐标遍历顺序查找可见区内全部四向同色连通组，选出大小达到 `clear_threshold` 的组。
  2. 合并其普通球坐标，收集四向相邻的垃圾坐标；一颗垃圾即使邻接多个消除组也只清除一次。
  3. 保存坐标与 `ChainLinkFacts`，进入 `ClearPreview`；盘面此时仍保留待消球。
  4. 预览到期进入 `ClearCommit`，一次性清空上述坐标，并在该 tick 产生本连锁步的分数、攻击与清除事实。
  5. 计算重力移动与目标盘面，进入 `Gravity`。
- 输出：逐步累积的 `ChainLinkFacts`。
- 错误语义：没有达到阈值的组时不进入 `ClearPreview`，直接形成报告。

固定遍历顺序不改变最终盘面，但决定组列表次序、事件次序与 checksum 编码。

### 重力稳定

- 输入：已清除但未稳定的盘面。
- 处理：每列独立压实，保持同列球的垂直相对顺序；只移动已落盘格，不读取玩家输入。重力到期后原子提交目标盘面。
- 输出：稳定盘面与每颗球的起终点。
- 错误语义：提交后的盘面不得存在空格下方仍有实体格的悬空状态。

### 完成

- 输入：稳定盘面、已累积的连锁步。
- 处理：无可消组时形成 `ChainReport`；零个连锁步表示本次落子未触发连锁。
- 输出：`ChainReport` 与 `FieldFacts`，在 `Settlement` 交给安全点。
- 错误语义：报告一经形成即不可变。

### 表现协议

- 表现层读取 `phase`、`elapsed_ticks`、`duration_ticks`、待消坐标与重力移动，不返回完成信号。
- 动画设置、渲染帧率、无窗口运行与表现跳帧均不改变阶段截止 tick。
- 规则层可以提前算出 `target_board` 或完整连锁计划，但未提交的结果只属于内部可回滚状态，不经读模型暴露。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `GroupLocked` 与盘面 | [活动组操控](board-and-falling-group.md) | 本主题 | 触发一次结算 |
| 逐步的 `ChainLinkFacts` | 本主题 | [得分与攻防](offense-and-nuisance.md) | 在对应 `ClearCommit` tick 生效，不提前发布 |
| `ChainReport`、`FieldFacts` | 本主题 | 安全点 | 在 `Settlement` 交付 |

双方各自推进自己的结算阶段：一名玩家处于任一阶段都不暂停另一名玩家的操控。Fever 与 margin 时钟在结算期间继续推进。

## 边界

- 本文不定义分数与攻击的换算（见[得分、攻击与垃圾攻防](offense-and-nuisance.md)）。
- 本文不定义全消奖励与 Fever 转换（见[Fever 循环](fever-mode.md)）。
- 本文不定义失败判定——判负只在生成活动组时发生（见[盘面与活动组操控](board-and-falling-group.md#供给与出生)）。
- 本文不定义安全点的执行顺序（见[小局、BO3 与安全点](match-and-round.md#安全点)）。
- 本文不定义动画插值、音效与粒子（见[表现与 UI 设计](../../presentation.md)）。

## Test Basis

- [玩法设计 §3.5](../../gameplay.md)：五段阶段状态机、阶段提交语义、表现不得推进规则、攻击只在 `ClearCommit` 逐步生效。
- [Basic rules](https://puyonexus.com/wiki/Basic_rules)：四向连通达到四个即消除，垃圾通过邻接消除组而清除。
- [Issue #12](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/12)：要求重力、四连、相邻垃圾、多轮连锁与全消事实。
