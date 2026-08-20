# 规则配置与开局规格冻结

**相关模块：** `game_core::config`、`game_core::rules`、`game_core::match_spec`、`client::data`、`assets/data/rules`
**关联文档：** [玩法设计](../../gameplay.md)、[版本化运行数据加载](runtime-data-loading.md)、[连锁强度曲线](chain-power-curve.md)、[DEC-001](../decision/rule-family-variation.md)

## 目标

把 `assets/data/rules/` 的版本化数据解析成 typed model，完成语义校验，并在一场 BO3 开始前冻结成整场不可变的对局规格。

## 数据模型

配置分成两个独立版本化的部分：**规则剖面**描述一套规则怎么算，**内容库**描述有哪些角色和素材可选。

```text
profiles/<profile_id>.ron   规则剖面
roster.ron                  角色身份
play/<profile_id>/*.ron     该剖面下的角色玩法数据与素材
```

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `RuleProfileId` | 剖面的稳定标识 | 与 `rule_version`、内容摘要并列，三者互不合并 |
| `RuleProfile` | 一套完整竞技规则的数值与语义 | 分为 `field`、`round`、`drop`、`rotation`、`resolve`、`scoring`、`offense`、`nuisance`、`fever` 九节；一个文件一个剖面，并携带 `reference_profile` |
| `CharacterIdentity` | 角色在规则核心中的身份 | ID 唯一 |
| 角色玩法数据 | 某剖面下某角色引用的 `DropSet` 与 `ChainPowerProfile` | 按剖面分区；某剖面不使用的玩法概念在该分区内不存在 |
| `DropSet` | 16 手循环掉落序列 | 每手形状与颜色布局有效；L/J 周期由 4 球单色手数的奇偶推导，不是独立字段 |
| `ChainPowerProfile` | 普通盘与 Fever 盘各一条定长 24 项整数 CP 曲线 | 取值落在 `[1, 999]`；连锁步超过表尾时取表尾值；同时携带生成参数作为来源信息 |
| `FeverPuzzleBook` | Fever 题面集合 | 覆盖所属剖面 `fever` 节声明的全部目标等级；每个题面通过盘面合法性检查 |
| `MatchRequest` | 一场 BO3 的开局请求 | `rule_profile_id`、根随机种子、两个 participant slot 及各自 `character_id` |
| `LockedMatchSpec` | 冻结后的对局规格 | BO3 生命周期内不可变、可复制；携带摘要树与算法版本 |

数值使用整数、tick 或明确的有理数表达，规则状态中不出现浮点。所有影响结果的时长以 tick 写入配置。由参数推导的数值表（margin 目标分衰减、CP 曲线）以整数表写入：表是权威数据，生成参数是来源信息。

### 摘要树

每个配置文件是一个摘要主体，根摘要覆盖有序的主体摘要：

```text
root_digest = H( profile_digest ‖ roster_digest ‖ play_digest[0..n] )
```

`LockedMatchSpec` 携带根摘要与本局实际使用的主体摘要。摘要不一致时可定位到具体主体；改动一个角色玩法数据文件时，其余主体摘要不变。

## 行为

### 解析与结构校验

- 输入：内存中的 RON 文本及其资源类别。
- 处理：反序列化、`schema_version` 检查、ID 引用解析。
- 输出：typed model。
- 错误语义：返回带字段路径的 `ConfigError`，不产生部分可用的规则对象。

### 语义校验

- 输入：解析后的剖面与内容库。
- 处理：只使用整数运算，依次检查三层。
  1. **通用完整性**：ID 唯一且引用可解析；棋盘尺寸、出生/溢出坐标与题面坐标在允许范围内；tick 时序为正且上下限有序；形状布局、旋转尝试表与 16 手掉落组完整；CP 曲线为 24 项且落在取值域内。
  2. **剖面自洽**：CP、CB、GB、目标分、margin 表与垃圾上限的组合不造成整数溢出；`fever` 节的容量、时间范围、题面等级域与升降级结果闭合。
  3. **内容对剖面的覆盖**：剖面声明需要的每类玩法数据，对每个可选角色都存在且完整；题面书覆盖剖面声明的全部目标等级。
- 输出：可用于冻结的已校验数据。
- 错误语义：返回违反的约束与字段路径。

### 冻结对局规格

- 输入：`MatchRequest` 与已校验的剖面、内容库。
- 处理：按 `rule_profile_id` 取得剖面，按 `character_id` 取得该剖面分区下的玩法数据；把影响结果的配置投影为不可变值；计算摘要树并与根种子一并保存。
- 输出：`LockedMatchSpec`。
- 错误语义：剖面或所选角色的玩法数据不可用时返回冻结失败，不产生 `LockedMatchSpec`。

冻结之后的对局不再读取资产，也不观察资产变更；冻结点之前重新解析资产不影响已经冻结的规格。`LockedMatchSpec` 不携带对手类型：单人、本地双人与局域网对局使用同一份规则投影，规则核心无从表达某一方的输入来自人还是 AI。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| 规则源文本 | `client::data` | `game_core::config` | 已读取的内存内容与资源类别 |
| 已校验剖面与内容库 | `game_core::config` | client 流程组件 | resolved 规则数据 |
| `MatchRequest` | client 角色选择流程与赛果页再战 | `game_core::match_spec` | 剖面 id、根种子与双方角色 |
| `LockedMatchSpec` | `game_core::match_spec` | 规则聚合根、确定性验证、联机握手 | 整场 BO3 不可变 |

1. `client::data` 读取 `assets/data/rules/` 并把内容交给 `game_core::config` 解析与校验。
2. 解析或校验失败时该类别按[阻断级](runtime-data-loading.md#失败分级)处置。
3. 角色选择流程在双方确认后提交 `MatchRequest`。
4. `game_core::match_spec` 冻结出 `LockedMatchSpec`，其后的规则推进只读取它。

## 边界

- 本文不定义规则数值本身及其原作出处（见[玩法设计](../../gameplay.md)）。
- 本文不定义资产读取、schema 版本头与失败分级（见[版本化运行数据加载](runtime-data-loading.md)）。
- 本文不定义 CP 曲线的形状、参数与生成，也不定义整数表与生成参数的一致性校验（见[连锁强度曲线](chain-power-curve.md)）。
- 本文不定义配置为何不表达规则族差异（见 [DEC-001](../decision/rule-family-variation.md)）。

## Test Basis

- [玩法设计 §7](../../gameplay.md)：剖面与内容库两部分结构、必须覆盖的量、时长以 tick 写入、派生数值表以整数表写入。
- [玩法设计 §3.2、§3.3](../../gameplay.md)：`DropSet` 的 16 手与 L/J 周期推导、`ChainPowerProfile` 的两条 24 项曲线与摘要锁定。
- [版本化运行数据加载 §失败分级](runtime-data-loading.md#失败分级)：规则数据为阻断级，失败时不进入对局。
- [Issue #12](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/12)：要求版本化数值、题面、颜色、角色定义与掉落组。
