# 测试用例设计：AI 参与者

**关联设计：** [AI 参与者](../../development/design/ai-player.md)、[DEC-006](../../development/decision/ai-baseline.md)
**关联实现：** `crates/client`（`ai`、`simulation`）、`crates/game_core`（`view`）

## 需求理解摘要

**功能：** 单难度实时 AI 通过 participant slot 的合法动作参加对局，并满足基础连锁、抵消、危险识别与 Fever 利用的基线。
**测试性质：** 新功能
**本轮范围：** 规划触发、候选生成与评价、动作执行，以及 AI 对局的可复现性。
**Test Basis：**
- [Confirmed] [AI 参与者](../../development/design/ai-player.md)：读模型边界、四个行为与固定时序。
- [Confirmed] [PRD §4.3](../../PRD.md)：AI 使用与人类相同的规则与可见信息，只通过合法输入行动，并给出单难度验收基线。
- [Confirmed] [DEC-006](../../development/decision/ai-baseline.md)：单一均衡评价器与固定时序。
**设计基线：** AI 不拥有修改规则状态的入口，其计划与时序不进入规则快照。
**关键假设：**
- AI 时序是 `turn_id` 与计划步序的纯函数，不消费随机数。
- 必死候选排序垫底而不剔除，因此候选集不会为空。
**待确认问题：**
- 思考延迟与按键间隔的数值为校准项（[DEC-006](../../development/decision/ai-baseline.md)）；用例以配置值 `d`、`k` 表达并断言相对时序，取值确定后补充绝对 tick 断言。

## 测试点清单

### Component Integration — AI

- 四种形状与关键障碍盘面上，AI 输出的每个动作都合法且最终到达计划姿态（TC-001）。
- `turn_id` 改变时规划一次；计划失效、盘面变化或目标 turn 结束时丢弃并重新规划（TC-002）。
- 思考延迟与按键间隔为固定 tick，AI 时序不消费随机数（TC-003）。
- 存在确定抵消、立即溢出风险与可进入 Fever 三类场景时，选择满足生存基线的候选（TC-004～TC-006）。
- 相同 `PlayerView` 重复规划得到相同计划；镜像盘面得到镜像结果（Concern: Determinism；TC-007～TC-008）。
- AI 不读取未进入 `NextQueue` 的颜色与题面随机状态（TC-009）。
- 全部候选都会导致失败时仍产出计划并以硬降收尾，不出现原地不落子（TC-010）。

### Component Integration — AI；Match Flow

- 固定规则与种子下连续至少 20 场 AI 对局全部正常结束，无非法落子、卡死或直接状态修改（Concern: Determinism；TC-011）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 性质测试 | AI 输出始终属于当前状态允许的动作集合；每个计划以硬降或锁定收尾 | TC-001、TC-010～TC-011 |
| 场景法 | 确定抵消、立即溢出、可进入 Fever 三类局面的完整决策链路 | TC-004～TC-006 |
| 变形测试 | 相同观察重复规划、镜像盘面与固定种子重跑的结果关系 | TC-007～TC-008、TC-011 |
| 状态迁移 | 规划触发与丢弃计划的守卫条件 | TC-002 |
| 边界值分析 | 思考延迟与按键间隔的首个与相邻动作 tick；候选集为空的边界 | TC-003、TC-010 |
| 错误猜测 | AI 越过读模型取用随机状态；盘面在计划执行中途被垃圾或 Fever 切换改变 | TC-002、TC-009 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 四种形状在障碍盘面上都能由合法动作到达计划姿态 | P0 | Component Integration | — | AI | AI 通过 participant slot 提交输入，规则状态只由 `MatchState::step` 改变 | 参数化形状与盘面组合，让 AI 完成一手落子并逐 tick 校验其输出 | 形状 `I`、`L`、`J`、`O`（单色与双色）；盘面：空盘、单列深井、贴左墙高台、贴右墙高台、中央凹洞 | 每 tick 输出的动作都属于该 tick 的合法动作集合；锁定时的姿态与坐标等于 `PlacementPlan` 指定的落点；全程没有绕过归一化入口或直接写盘面的调用 | [Confirmed] [AI 参与者：动作执行](../../development/design/ai-player.md#动作执行)；[PRD §4.3](../../PRD.md) |
| TC-002 | 计划在 turn 改变或观察失效时被丢弃并重新规划 | P1 | Component Integration | — | AI | AI 已对当前 `turn_id` 产出计划并开始执行 | 参数化四种刺激后读取 `AiControllerState` 与后续输出 | `turn_id` 改变；计划中剩余动作变为非法；盘面因垃圾落下而变化；盘面因 Fever 切换而变化 | 四种刺激都使当前计划被丢弃并对新的 `PlayerView` 重新规划一次；同一 `turn_id` 在无上述刺激时只规划一次；重新规划后仍以合法动作收尾，不残留旧计划的动作 | [Confirmed] [AI 参与者：触发规划](../../development/design/ai-player.md#触发规划) |
| TC-003 | 思考延迟与按键间隔为固定 tick 且不消费随机数 | P1 | Component Integration | Determinism | AI | AI 时序参数取自配置 | 记录一手落子中每个动作发出的 tick，并在前后比较规则随机流位置 | 思考延迟 `d` tick、按键间隔 `k` tick（取配置值）；同一计划连续执行两次 | 首个动作在规划完成后的第 `d` tick 发出，相邻两个动作恰隔 `k` tick，每个 tick 至多发出一次一次性动作；两次执行的动作 tick 序列相同；`color`、`nuisance`、`fever-puzzle` 三个流的位置在 AI 规划与执行前后不变 | [Confirmed] [AI 参与者：时序](../../development/design/ai-player.md#时序)；[DEC-006](../../development/decision/ai-baseline.md) |
| TC-004 | 存在确定抵消机会时优先完成抵消 | P1 | Component Integration | — | AI | AI 侧队列非零，盘面存在一步即可触发连锁的落点 | 让 AI 规划并执行一手，读取本手结算后的队列 | 待接收垃圾 6；盘面存在可立即形成 4 连的落点；同时存在不触发连锁但盘面更平整的落点 | AI 选择触发连锁的落点；本手结算后队列被抵消且未落下垃圾；评价的抵消层优先于盘面形态层 | [Confirmed] [AI 参与者：候选评价](../../development/design/ai-player.md#候选评价)；[PRD §4.3](../../PRD.md) |
| TC-005 | 存在立即溢出风险时优先避免溢出 | P0 | Component Integration | — | AI | 盘面已堆高，部分落点会使出生列上格被占 | 让 AI 规划并执行一手，读取候选排序与本手结果 | 出生列邻近堆到距隐藏行 1 格；候选中既有会导致下一手出生失败的落点，也有可存活的落点 | AI 选择不导致下一手出生失败的落点；会导致失败的候选在评价中排序垫底但仍保留在候选集中；本手结束后该玩家未判负 | [Confirmed] [AI 参与者：候选生成](../../development/design/ai-player.md#候选生成)、[候选评价](../../development/design/ai-player.md#候选评价) |
| TC-006 | 可进入或利用 Fever 时优先完成该机会 | P1 | Component Integration | — | AI | 量表差一格填满，且存在可抵消并触发连锁的落点 | 让 AI 规划并执行一手，读取量表与通道状态 | 量表 6/7；待接收垃圾 4；存在一步抵消并触发连锁的落点 | AI 选择该落点；本手安全点量表填满并进入 Fever；在无溢出风险时该层优先于即时攻击层 | [Confirmed] [AI 参与者：候选评价](../../development/design/ai-player.md#候选评价)；[PRD §4.3](../../PRD.md) |
| TC-007 | 相同 PlayerView 重复规划得到相同计划 | P1 | Component Integration | Determinism | AI | 一份固定的 `PlayerView` | 对同一份 `PlayerView` 连续规划 10 次并比较结果 | 中局盘面，候选集含至少两个同分候选 | 10 次得到同一个 `PlacementPlan`，包括落点、姿态与动作路径；同分候选按规范候选顺序取第一个，排序不受容器遍历顺序影响 | [Confirmed] [AI 参与者：候选评价](../../development/design/ai-player.md#候选评价) |
| TC-008 | 镜像盘面得到镜像计划 | P1 | Component Integration | Determinism | AI | 一份 `PlayerView` 及其左右镜像 | 分别规划并比较两个计划 | 中局盘面及其按列镜像；活动组颜色布局同步镜像 | 镜像盘面的计划落点列等于原计划落点列的镜像，姿态为对应的镜像朝向；两者的评价分层结论一致 | [Confirmed] [AI 参与者：候选评价](../../development/design/ai-player.md#候选评价) |
| TC-009 | AI 只读 PlayerView，不取用未入队颜色与题面随机状态 | P1 | Component Integration | — | AI | AI 只能经 `PlayerView` 取得规则信息 | 在两份只有不可见信息不同的状态上各规划一次并比较计划 | 两份状态的盘面、活动组、`NextQueue`、队列、Fever 与比分完全相同；仅未入队的后续颜色与 `fever-puzzle` 流位置不同 | 两份状态得到相同的 `PlacementPlan`；AI 侧不存在读取 RNG 状态或未入队组的路径；AI 状态不出现在规则快照中 | [Confirmed] [AI 参与者：数据模型](../../development/design/ai-player.md#数据模型)；[确定性与快照：快照](../../development/design/determinism-and-snapshot.md#快照) |
| TC-010 | 全部候选都会导致失败时仍产出计划并以硬降收尾 | P1 | Component Integration | — | AI | 盘面已堆满到任何落点都会使下一手出生失败 | 让 AI 规划并执行一手 | 出生列及相邻列均堆到隐藏行 | 候选集非空，必死候选保留并排序垫底；AI 仍产出唯一 `PlacementPlan` 并以硬降收尾；不出现不发出任何动作或活动组停在盘面上不落子的情形 | [Confirmed] [AI 参与者：候选生成](../../development/design/ai-player.md#候选生成)、[动作执行](../../development/design/ai-player.md#动作执行) |
| TC-011 | 固定规则与种子下 20 场 AI 对局全部正常结束且可复现 | P0 | Component Integration | Determinism | AI；Match Flow | 已冻结的两人对局规格，双方均由 AI 驱动 | 以 20 个固定根种子各跑完一整场 BO3，随后用同一批种子重跑一次 | 根种子 `0x1`～`0x14`；角色组合覆盖 A×A、A×B、B×B；每场设 tick 上限 | 20 场全部以某方两胜结束，无一场触及 tick 上限；全程无非法动作、无 AI 直接修改规则状态的调用；重跑得到逐场相同的 `MatchOutcome`、小局结果序列与终局校验和 | [Confirmed] [PRD §4.3](../../PRD.md)；[小局、BO3 与安全点：完成态](../../development/design/match-and-round.md#完成态) |

## 风险查漏

四种形状与五类障碍盘面的合法性、规划触发与四种重规划刺激、固定时序与不消费随机数、三类生存基线场景、重复规划与镜像的一致性、读模型边界、必死局面的收尾、以及 20 场对局的结束性与可复现性均有直接用例；规则判定本身不在本稿，由各 Component 测试稿承担。
