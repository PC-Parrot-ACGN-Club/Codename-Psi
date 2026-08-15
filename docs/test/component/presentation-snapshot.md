# 测试用例设计：表现快照与表现事件

**关联设计：** [表现运行时](../../development/design/presentation-runtime.md)、[表现与 UI 设计 §8](../../presentation.md)、[小局、BO3 与安全点](../../development/design/match-and-round.md)

**关联实现：** `../../../crates/client`、`../../../crates/game_core`

## 需求理解摘要

**功能：** 验证表现快照的构造与完整性、表现事件的编号与去重，以及动画强度对规则事实的不可见性。
**测试性质：** 新功能
**本轮范围：** 从内存中的 `MatchView` 与 `MatchStepReport` 构造快照与事件序列的纯投影行为。
**Test Basis：**

- [Confirmed] [表现运行时](../../development/design/presentation-runtime.md)：快照构造、事件发布、动画强度与画面重建。
- [Confirmed] [表现与 UI 设计 §8](../../presentation.md)：`MatchPresentationSnapshot` 与 `PresentationEvent` 的组成。
- [Confirmed] [表现与 UI 设计 §3.1](../../presentation.md)：优势状态的推导输入。

**设计基线：** 快照与事件都是规则事实的纯函数，不需要渲染后端或窗口即可判定。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义画面重建与降级的运行时行为（见 [表现运行时](../integration-system/presentation-runtime.md)）、规则阶段本身的推进（见 [连锁结算](chain-resolution.md)、[Fever 循环](fever-mode.md)），也不定义角色表现数据的解析（见 [角色表现数据](character-presentation.md)）。

## 测试点清单

### Component — Client

- 快照覆盖常驻信息且无对局实例时不产生（TC-001～TC-002）。
- 表现事件按报告顺序编号，同 tick 内唯一（TC-003）。
- 同一标识只演出一次（TC-004）。
- 丢弃全部事件后快照仍完整表达对局状态（TC-005）。
- 优势状态由规则事实推导（TC-006）。
- 两档动画强度不改变快照中的规则事实（TC-007；Concern: Determinism）。
- 一次性事实留下的文字行的取舍、本地化与到期（TC-008）。
- 垃圾队列按固定单位分解为分级图标（TC-009）。
- Fever 题面的预设格子被标出，缺题面与查不到题面都不标记（TC-010）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 场景法 | 普通场、Fever 场与结算中三种规则阶段的快照 | TC-001、TC-005 |
| 等价类划分 | 有对局实例与无对局实例 | TC-002 |
| 等价类划分 | 未进入 Fever、题面可查、题面查不到 | TC-010 |
| 边界值 | 同 tick 零个、一个与多个事件 | TC-003 |
| 错误猜测 | 同一事件被重复消费 | TC-004 |
| 判定表 | 待接收垃圾、溢出风险、净攻击与 Fever 状态的组合 | TC-006 |
| 对比测试 | `Full` 与 `Reduced` 两档下的同一规则输入 | TC-007 |
| 判定表 | 全消 × 送出攻击 × 抵消三种事实的同 tick 组合 | TC-008 |
| 边界值 | 文字行存活期的最后一 tick 与其后一 tick | TC-008 |
| 边界值 | 队列为 0、恰好一个单位、单位减一与远超面板槽位 | TC-009 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 快照在三种规则阶段均覆盖全部常驻信息 | P0 | Component | — | Client | 可构造任意 `MatchView` 与 `LockedMatchSpec` | 分别在三种阶段构造快照 | 普通场落子中；Fever 场进行中；连锁结算 `ClearPreview` 中 | 每份快照均含双方棋盘、活动组、三手 NEXT、`drop_set_id`、分数、普通与 Fever 两条垃圾队列的精确数量、Fever 量表/时间/题面目标、`chain_count`、比分与 `round`；结算中的快照含当前结算阶段与进度 | [Confirmed] [表现运行时：快照构造](../../development/design/presentation-runtime.md#快照构造)；[表现与 UI 设计 §8](../../presentation.md) |
| TC-002 | 无对局实例时不产生快照 | P2 | Component | — | Client | 不存在对局实例 | 请求构造快照 | 无 `MatchView` | 不产生快照；HUD 数据源为空且不产生诊断噪音 | [Confirmed] [表现运行时：快照构造](../../development/design/presentation-runtime.md#快照构造) |
| TC-003 | 表现事件按报告顺序从 0 编号且同 tick 内唯一 | P1 | Component | — | Client | 可构造含任意事件序列的 `MatchStepReport` | 参数化发布三种事件数量的报告 | 同一 `match_tick` 下 0 个、1 个、5 个事件（含同类别多 slot） | 零事件时不产生 `PresentationEvent`；其余情况 `ordinal` 从 0 连续递增，顺序与 `MatchStepReport.events` 一致；同一 `match_tick` 内 `ordinal` 互不相同 | [Confirmed] [表现运行时：表现事件发布](../../development/design/presentation-runtime.md#表现事件发布) |
| TC-004 | 同一 `(match_tick, ordinal)` 只演出一次 | P1 | Component | — | Client | 已发布一批表现事件 | 将同一批事件重复提交给演出消费入口 | 同一 tick 的三个事件各提交两次 | 每个标识只触发一次演出；重复提交不增加演出次数，也不改变快照 | [Confirmed] [表现运行时：表现事件发布](../../development/design/presentation-runtime.md#表现事件发布) |
| TC-005 | 丢弃全部表现事件后快照仍完整表达对局状态 | P0 | Component | — | Client | 一段含消除、垃圾入队与 Fever 进入的 tick 序列 | 推进该序列，丢弃全部 `PresentationEvent`，只保留最后一份快照 | 连续 30 tick，其间发生连锁、垃圾入队与 Fever 进入 | 最终快照的棋盘、两条队列数量、Fever 状态、分数与比分与逐 tick 消费事件的结果一致；不存在只能由事件得知的常驻信息 | [Confirmed] [表现运行时：画面重建](../../development/design/presentation-runtime.md#画面重建) |
| TC-006 | 优势状态由待接收垃圾、溢出风险、净攻击与 Fever 状态推导 | P2 | Component | — | Client | 可构造双方的规则事实 | 参数化构造四组局面并读取 `momentum` | 本方净攻击为正且对方待接收垃圾多；本方待接收垃圾多且接近溢出；双方对称；一方处于 Fever | 前两组的 `advantage_side` 分别指向本方与对方；对称局面不指向任一方；`momentum` 只由列出的规则事实决定，不受角色表现数据或动画强度影响 | [Confirmed] [表现与 UI 设计 §3.1](../../presentation.md)；[表现运行时：快照构造](../../development/design/presentation-runtime.md#快照构造) |
| TC-007 | 两档动画强度不改变快照中的规则事实 | P0 | Component | Determinism | Client；Rules | 同一 `MatchView` 与 `MatchStepReport` | 分别以 `Full` 与 `Reduced` 构造快照与事件，并各求一次每颗球的痕迹数 | `AnimationIntensity::Full`；`AnimationIntensity::Reduced` | 两档产生的快照除演出参数外逐字段相同，把演出参数归一后整份快照相等；两档的演出参数本身不同；事件的种类、顺序与 `(match_tick, ordinal)` 相同；结算阶段的 `duration_ticks` 相同；`Full` 每颗球留下若干痕迹而 `Reduced` 每颗为零 | [Confirmed] [表现运行时：动画强度](../../development/design/presentation-runtime.md#动画强度)；[表现与 UI 设计 §6.1](../../presentation.md) |
| TC-008 | 一次性事实留下的文字行按优先级取舍并按 tick 到期 | P1 | Component | — | Client | 可构造含任意事实的 `MatchStepReport` 与两份语言词条 | 参数化三组同 tick 事实各观察一次，再推进到存活期两端，并把同一份报告重复观察一次 | 组一：slot 0 送出 12 且抵消 2，slot 1 抵消 6 且送出 0；组二：slot 0 同 tick 全消并送出 30；组三：一个无可说事实的 tick | 组一的两侧分别显示送出量与抵消量；组二显示全消；组三保留上一行不清屏；文字为本地化的事实名称加精确数量，`zh-CN` 下抵消行为「抵消 6」；存活期最后一 tick 仍显示、其后一 tick 消失；重复观察同一份报告不改变任何一行 | [Confirmed] [表现运行时：文字反馈](../../development/design/presentation-runtime.md#文字反馈)；[表现与 UI 设计 §3](../../presentation.md) |
| TC-009 | 垃圾队列按固定单位从大到小分解为分级图标 | P2 | Component | — | Client | 可给出任意精确队列数量 | 参数化若干队列数量各求一次图标序列 | 0、1、5、6、35、2531、100000 | 0 不出图标；6 出一个「一行」图标而不是六个单位图标；35 分解为 30 加五个 1；2531 依次用尽 1440、720、360 后才用更轻的单位；任意数量的图标数不超过面板槽位、单调不增、所代表的总量不超过队列本身；单位集合为 1440/720/360/180/30/6/1 | [Confirmed] [表现运行时：垃圾队列分级](../../development/design/presentation-runtime.md#垃圾队列分级)；[表现与 UI 设计 §4.3](../../presentation.md) |
| TC-010 | Fever 题面的预设格子在快照中被标出 | P2 | Component | — | Client | 冻结规格含题面书，可构造任意 `MatchView` | 参数化三种情形各构造一次快照 | 未开启 Fever；题面标识取自题面书；题面标识为不存在的 id | 未开启 Fever 时不标记任何格子；题面可查时标记恰为该题面自己的格子，且对手侧不受影响；题面查不到时不标记且仍产出完整快照 | [Confirmed] [表现运行时：题面预设标记](../../development/design/presentation-runtime.md#题面预设标记)；[表现与 UI 设计 §4.4](../../presentation.md) |

## 风险查漏

快照完整性、事件编号与去重、可重建性、优势推导、文字反馈取舍、队列分级、题面预设标记与动画强度不变性均有直接用例；跳帧采样、降级与整局 checksum 一致性由集成测试稿覆盖。
