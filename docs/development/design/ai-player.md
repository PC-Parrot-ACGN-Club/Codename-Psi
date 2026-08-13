# AI 参与者

**相关模块：** `client::ai`、`game_core::view`
**关联文档：** [PRD §4.3](../../PRD.md)、[盘面与活动组操控](board-and-falling-group.md)、[小局、BO3 与安全点](match-and-round.md)、[DEC-006](../decision/ai-baseline.md)

## 目标

提供单难度实时 AI，能完成基础连锁、进行垃圾抵消、识别即将溢出的危险并利用 Fever 机会，且只通过 participant slot 的合法动作参加对局。

## 数据模型

AI 编排位于 `client::ai`：它是输入生产方，与本地设备输入同级；`game_core` 只提供公开读模型与可复用的纯规则查询。

```text
MatchState
  └─ PlayerView
       └─ AiPlanner ─ PlacementPlan ─ AiActionExecutor ─ PlayerActions
                                                            └─ TickInputs
```

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `PlayerView` | 玩家可见信息 | 双方公开盘面、活动组、已入队 `NextQueue`、精确垃圾、Fever、比分；不含 RNG 状态与未入队的未来组 |
| `PlacementCandidate` | 一个可达终局姿态 | 到达它的合法动作序列，以及规则沙盒的模拟摘要 |
| `CandidateScore` | 候选评分 | 固定顺序的整数特征，不使用浮点与不稳定比较 |
| `PlacementPlan` | 已选定的落点 | 绑定 `turn_id` 与观察版本 |
| `AiControllerState` | 执行状态 | 当前计划、思考延迟计时、按键间隔计时、是否需要重规划 |

AI 可在沙盒中复制单盘状态评估候选，但沙盒必须复用规则模块的纯函数，不维护第二套消除或计分公式。AI 状态不进入规则快照。

### 时序

| 参数 | 说明 |
| --- | --- |
| 思考延迟 | 每个新活动组开始规划到发出首个动作之间的固定 tick 数 |
| 按键间隔 | 计划执行期间两次动作之间的固定 tick 数 |

两者都是固定 tick，因此 AI 时序是 `turn_id` 与计划步序的纯函数，不消费随机数。具体数值没有来源，属校准项，取值理由见 [DEC-006](../decision/ai-baseline.md)。

## 行为

### 触发规划

- 输入：`PlayerView`、当前 `turn_id`。
- 处理：`turn_id` 改变时规划一次；计划中的动作变为非法、盘面因垃圾落下或 Fever 切换而变化、或目标 turn 结束时丢弃计划并重新规划。
- 输出：`PlacementPlan`。
- 错误语义：规划完成的时机不依赖墙钟或异步任务的返回顺序。

### 候选生成

- 输入：当前活动组与盘面。
- 处理：枚举所有可达的旋转与横向落点，为每个落点保留至少一条合法动作路径，并在规则沙盒中执行锁定、连锁、攻防与失败检查。
- 输出：`PlacementCandidate` 集合。
- 错误语义：只剔除非法或无法由动作到达的候选。**导致失败的候选保留并在评价中排到最后**，因此候选集不会为空。

### 候选评价

- 输入：候选集合与其模拟摘要。
- 处理：按词典序分层比较，而不是把所有指标合成一个总分：

  1. 避免立即溢出；
  2. 能消除迫近垃圾时优先存活；
  3. 可进入或利用 Fever 时优先完成该机会；
  4. 比较即时连锁、攻击与全消；
  5. 比较盘面高度、凹洞、颜色连接潜力与下一组适配；
  6. 完全同分时按规范候选顺序取第一个。

- 输出：唯一的 `PlacementPlan`。
- 错误语义：相同 `PlayerView` 重复评价得到相同计划。

### 动作执行

- 输入：`PlacementPlan`、`AiControllerState`。
- 处理：等待固定思考延迟后开始发出动作，每两次动作之间等待固定按键间隔；每个固定 tick 最多发出一次一次性动作，持续动作按既定 tick 数保持。目标姿态达到且路径校验通过后发出硬降。
- 输出：逐 tick 的 `PlayerActions`，经既有归一化后进入 `TickInputs`。
- 错误语义：**每个计划都以硬降或锁定收尾**——AI 始终交出一个落点，即使全部候选都会导致失败。AI 没有直接设置坐标、直接锁定或清盘的入口。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `PlayerView` | [小局、BO3 与安全点](match-and-round.md) | `client::ai` | 只读投影，与人类玩家可见的信息范围相同 |
| 纯规则查询与沙盒推进 | `game_core` | `client::ai` | 与生产对局共用同一套规则函数 |
| `PlayerActions` | `client::ai` | `client::simulation` | 与本地设备输入合成 `TickInputs` |

## 边界

- 本文不定义规则判定本身——AI 的一切结果都来自规则模块（见[盘面与活动组操控](board-and-falling-group.md)、[连锁结算](chain-resolution.md)、[得分、攻击与垃圾攻防](offense-and-nuisance.md)）。
- 本文不定义难度分级与角色打法差异（见 [DEC-006](../decision/ai-baseline.md)）。
- 本文不定义动作的物理绑定与设备采集（见[本地输入采样](local-input-sampling.md)）。
- 本文不定义 AI 的角色语音与表现反馈（见[表现与 UI 设计](../../presentation.md)）。

## Test Basis

- [PRD §4.3](../../PRD.md)：AI 使用与人类相同的规则、只通过合法输入行动、可读信息限于人类同款可见状态，并以基础连锁、抵消、危险识别与 Fever 利用为验收基线。
- [统一游戏动作与 Tick 输入](game-action-input.md)：AI 输出与人类输入共用同一动作位集与归一化入口。
- [Issue #12](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/12)：要求合法 AI 动作与基础连锁、抵消、危险、Fever 决策。
