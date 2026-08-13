# 确定性、快照与状态校验

**相关模块：** `game_core::determinism`、`game_core::verification_log`
**关联文档：** [TDD §3、§6](../../TDD.md)、[小局、BO3 与安全点](match-and-round.md)、[DEC-005](../decision/color-sequence-derivation.md)

## 目标

保证相同的开局规格、根种子与逐 tick 输入得到相同状态，并提供深复制、恢复、稳定校验和与无窗口运行的验证日志。该能力服务于开发校验与后续的网络回滚。

## 数据模型

### 确定性约束

- 规则核心不读取墙钟、线程调度、系统熵、文件系统或 ECS 查询顺序。
- 规则时间只使用整数 tick；坐标、分数、倍率与余数不使用浮点。
- 任何影响结果的容器具有规范遍历顺序；不让哈希顺序进入状态转换。
- 所有随机抽取来自命名流；新增一种随机消费不扰动无关领域的已有序列。
- 摘要与算法版本进入验证元数据。

### 随机流

```text
root_seed
└─ derive(round_index, draw_attempt, player_slot, stream_name)
     ├─ "color"
     ├─ "nuisance"
     └─ "fever-puzzle"
```

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `MatchRng` | 全部命名流的集合 | 每名玩家各持一组独立流，派生键含 `player_slot`（[DEC-005](../decision/color-sequence-derivation.md)） |
| 命名流 | 一个领域的随机来源 | 名称是稳定领域标识；算法升级必须提版本，不得在同一版本下静默改变输出 |
| `draw_attempt` | 同一局号的重打次数 | 进入派生键，使[同时失败](match-and-round.md#失败判定)后重打得到不同序列 |

### 快照

```text
MatchSnapshot
├─ snapshot_schema_version
├─ 根摘要与本局使用的主体摘要
├─ algorithm_versions
└─ state
```

快照覆盖全部会影响后续结果的可变状态：

| 领域 | 覆盖内容 |
| --- | --- |
| 比赛 | match/round tick、BO3 阶段、比分、`round_index`、`draw_attempt`、小局历史 |
| 盘面与操控 | 两名玩家各两个通道的盘面、活动组、`NextQueue`、`DropCursor` 与 L/J 周期状态、全部操控计时器 |
| 结算 | 当前结算阶段与其计时、待消集合、重力移动、尚未提交的目标盘面 |
| 攻防 | 分数、攻击余数、margin 表下标、**每通道的待接收垃圾整数与列序位置** |
| Fever | 量表、**玩家级 Fever 时间**、活动通道、会话的目标等级与当前题面、**每等级的无重复袋状态** |
| 随机 | 全部命名流的内部状态，含流位置 |

不进入快照：规则事件消费游标、渲染实体、音频、用户设置、设备状态、AI 计划与其时序、网络 socket。

### 状态编码与校验和

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `StateCodecV1` | 状态的规范字节编码 | 明确的字段顺序、宽度与字节序；不对内存布局、`Debug` 输出或未承诺稳定的通用序列化结果直接哈希 |
| `StateChecksum` | 状态校验和 | 由编码结果产生的定长摘要；根摘要作为前缀参与计算 |

静态规则正文不重复编码。当步规则事件不进入校验和，但产生这些事件所依赖的持久字段必须进入。

### 验证日志

```text
VerificationLog
├─ format_version
├─ 根摘要与本局主体摘要
├─ rng / state codec 版本
├─ root_seed / character_ids
├─ inputs[tick][2]
└─ checkpoints[]
```

## 行为

### 派生随机流

- 输入：根种子、`round_index`、`draw_attempt`、participant slot、流名。
- 处理：按固定派生函数生成该流的初始状态。
- 输出：可独立推进的流。
- 错误语义：未注册的流名不可派生。

### 快照与恢复

- 输入：`MatchState`（快照）或 `MatchSnapshot` 与开局规格（恢复）。
- 处理：快照深复制上表全部字段；恢复先校验 schema 版本、摘要与算法版本，通过后重建状态。
- 输出：`MatchSnapshot` 或重建的 `MatchState`。
- 错误语义：任一版本或摘要不匹配时返回 typed error，不做近似恢复，也不提供跨版本迁移。

### 计算状态校验和

- 输入：`MatchState`。
- 处理：按 `StateCodecV1` 编码后求摘要。
- 输出：`StateChecksum`。
- 错误语义：同一状态在不同进程与不同运行中得到同一值。

### 运行验证日志

- 输入：`VerificationLog`。
- 处理：无窗口创建 `MatchState`，逐 tick 解码输入并推进，在每个 checkpoint 比较校验和与关键读模型。
- 输出：结构化差异，或全部一致的结论。
- 错误语义：日志的格式版本或摘要与当前规则不匹配时拒绝运行。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| 摘要树与算法版本 | [开局规格冻结](rule-configuration.md) | 本主题 | 进入快照头与校验和前缀 |
| 可变规则状态 | [小局、BO3 与安全点](match-and-round.md) | 本主题 | 快照的唯一来源 |
| `MatchSnapshot`、`StateChecksum` | 本主题 | 开发校验、网络回滚 | 只在相同摘要与算法版本间可比 |

## 边界

- 本文不定义规则本身的推进（见[小局、BO3 与安全点](match-and-round.md)）。
- 本文不定义配置摘要的构成（见[规则配置与开局规格冻结](rule-configuration.md#摘要树)）。
- 本文不定义网络协议、预测输入与断线处理，也不提供面向玩家的回放功能（见 [PRD §2.3](../../PRD.md)）。
- 本文不定义具体的随机算法与摘要算法选型——只要满足上述版本化与规范编码约束即可。

## Test Basis

- [TDD §3](../../TDD.md)：规则以固定 tick 推进，相同初始状态与输入序列得到相同状态校验和。
- [TDD §6](../../TDD.md)：要求深拷贝快照、恢复与稳定校验和，并在包含回滚的千 tick 连续推进后保持一致。
- [玩法设计 §6.1](../../gameplay.md)：和局重打必须得到与上一次不同的随机序列。
- [Issue #12](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/12)：要求同种子同输入同结果、复制恢复与 checksum。
