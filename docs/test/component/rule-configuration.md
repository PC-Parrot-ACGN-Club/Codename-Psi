# 测试用例设计：规则配置与开局规格冻结

**关联设计：** [规则配置与开局规格冻结](../../development/design/rule-configuration.md)、[连锁强度曲线](../../development/design/chain-power-curve.md)、[DEC-004](../../development/decision/settlement-timing-values.md)
**关联实现：** `crates/game_core`（`config`、`rules`、`match_spec`）、`assets/data/rules`

## 需求理解摘要

**功能：** 把版本化规则剖面与内容库解析、校验，并在 BO3 开始前冻结成不可变的对局规格。
**测试性质：** 新功能
**本轮范围：** 解析与结构校验、三层语义校验、摘要树、冻结入口，以及连锁强度曲线的生成与一致性校验。
**Test Basis：**
- [Confirmed] [规则配置与开局规格冻结](../../development/design/rule-configuration.md)：数据模型、摘要树与三个行为的输入输出与错误语义。
- [Confirmed] [连锁强度曲线](../../development/design/chain-power-curve.md)：曲线族公式、角色参数、生成与 CI 校验的分工。
- [Confirmed] [玩法设计 §7](../../gameplay.md)：剖面与内容库的两部分结构、必须覆盖的量、时长以 tick 写入。
- [Confirmed] [版本化运行数据加载 §失败分级](../../development/design/runtime-data-loading.md#失败分级)：规则数据为阻断级。
**设计基线：** 参考剖面 `fever1_2_console_pc`，规则数据不可用时不进入对局且无内置默认值。
**关键假设：**
- 解析器只接受调用方传入的内存内容，用例不访问文件系统。
- 「整数表与生成参数一致」在 CI 固定工具链上执行，不属于运行期校验，因此该测试点的失败面是提交而非开局。
**待确认问题：**
- 时长类配置为校准项（[DEC-004](../../development/decision/settlement-timing-values.md)）；校准后需同步更新以其为测试数据的用例。

## 测试点清单

### Component — Configuration

- 有效剖面与内容库解析并冻结成两人对局规格（Concern: Content Validation；TC-001）。
- 断引用、越界坐标、非法时间、缺失题面与非法掉落组各返回稳定错误类别（Concern: Content Validation；TC-002～TC-005）。
- 相同规范内容得到相同摘要；字段顺序与 RON 空白变化不改变摘要（TC-006）。
- 只改动一个角色玩法数据文件时，该主体摘要与根摘要变化，其余主体摘要不变（TC-007）。
- 资产源数据在加载完成后被修改，不改变已创建的 `LockedMatchSpec`（TC-008）。
- 规则数据缺失或校验失败时冻结入口返回失败，且不产生 `LockedMatchSpec`（Concern: Content Validation；TC-009）。
- 所有时长配置以 tick 表达；margin 按整数表取值，运行期不出现实时换算（TC-010）。

### Component — Configuration；Rules

- 以 `A=400 τ=1.00 g=0.25` 与 `F=40 σ=1.00` 生成的两张表与原作对应档位含饱和段逐点相等（Concern: Content Validation；TC-011）。
- 以 `A=360 τ=1.00 g=0.25` 与 `F=36 σ=1.00` 生成的两张表与原作对应档位含饱和段逐点相等（Concern: Content Validation；TC-012）。
- 每个角色配置内的整数表与其生成参数重新生成的结果逐点相等（TC-013）。
- 曲线表长为 24、取值落在 `[1, 999]`，连锁步超过 24 时取表尾值（TC-014）。
- 手改单格的配置能被运行期接受、被 CI 拒绝（TC-015）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 等价类划分 | 有效数据、结构缺失、语义冲突；三层校验各自的违反类别 | TC-001～TC-005 |
| 变形测试 | 字段顺序、空白与单文件改动对摘要的影响关系 | TC-006～TC-007 |
| 场景法 | 加载→校验→冻结→资产变更的完整链路 | TC-008～TC-009 |
| 固定样本比对 | 原作 Fever 2 定义档位与交叉验证档位的整数表作为逐点基准 | TC-011～TC-012 |
| 边界值分析 | 曲线索引 1、24、25 与取值域上下限 1、999 | TC-014 |
| 不变量检查 | 整数表与生成参数一致；运行期不出现实时换算 | TC-010、TC-013、TC-015 |
| 错误猜测 | 手改单格的配置在运行期与 CI 的不同失败面 | TC-015 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 有效剖面与内容库解析后冻结出两人对局规格 | P0 | Component | Content Validation | Configuration | 内存中持有参考剖面文本、角色身份表与两名角色的玩法数据文本 | 依次调用解析、语义校验与冻结入口 | `rule_profile_id=fever1_2_console_pc`；`root_seed=0x1`；slot 0=角色 A，slot 1=角色 B | 三步全部成功；`LockedMatchSpec` 的剖面 id、根种子与双方 `character_id` 与 `MatchRequest` 一致；携带根摘要与本局使用的主体摘要；不携带对手类型字段 | [Confirmed] [规则配置：冻结对局规格](../../development/design/rule-configuration.md#冻结对局规格) |
| TC-002 | 引用与坐标违反通用完整性时返回带字段路径的错误 | P1 | Component | Content Validation | Configuration | 一份有效数据集，可逐项注入单点缺陷 | 参数化注入单个缺陷后调用语义校验 | 角色引用不存在的 `drop_set_id`；题面引用不存在的等级；出生列 `spawn_column=6`（列数 6）；题面坐标 `y=14`（行数 14）；两个角色使用相同 `character_id` | 每组均返回违反的约束与字段路径；不产生可用的规则对象；相同缺陷重复校验返回同一错误类别 | [Confirmed] [规则配置：语义校验](../../development/design/rule-configuration.md#语义校验) |
| TC-003 | 时序与掉落组结构违反通用完整性时被拒 | P1 | Component | Content Validation | Configuration | 同 TC-002 | 参数化注入单个缺陷后调用语义校验 | 自然下落 `0 tick/格`；Fever 时间上限小于下限；掉落组只有 15 手；三球手缺少颜色布局；CP 曲线 23 项 | 五组均在通用完整性层返回错误并指出字段路径；不落到剖面自洽层与覆盖层 | [Confirmed] [规则配置：语义校验](../../development/design/rule-configuration.md#语义校验) |
| TC-004 | 剖面自洽层拒绝不闭合的 Fever 等级域与溢出组合 | P1 | Component | Content Validation | Configuration | 通用完整性层可通过的剖面 | 参数化注入单个缺陷后调用语义校验 | 等级域 `3..=15` 但升级表可给出 16；量表容量 0；`CP`/`CB`/`GB`/目标分组合使 `link_score` 超出所用整数宽度 | 三组均在剖面自洽层返回错误；错误指向 `fever` 或 `scoring` 节的具体字段 | [Confirmed] [规则配置：语义校验](../../development/design/rule-configuration.md#语义校验) |
| TC-005 | 内容未覆盖剖面声明时在覆盖层被拒 | P1 | Component | Content Validation | Configuration | 剖面声明等级域 `3..=15` 且声明需要 `DropSet` 与 `ChainPowerProfile` | 参数化删除内容库中的一类数据后调用语义校验 | 题面书缺少等级 15；角色 B 缺少 Fever 曲线；角色 B 无玩法数据分区 | 三组均在覆盖层返回错误，指出缺失的角色或等级；有效角色的数据不因此被判为无效 | [Confirmed] [规则配置：语义校验](../../development/design/rule-configuration.md#语义校验) |
| TC-006 | 规范内容相同时摘要不随字段顺序与空白变化 | P1 | Component | — | Configuration | 一份有效剖面文本 | 生成三份语义等价的变体并分别解析取摘要 | 原文；顶层字段顺序打乱；缩进、换行与行尾空白改写 | 三者的主体摘要与根摘要两两相等；改动任一数值后摘要变化 | [Confirmed] [规则配置：摘要树](../../development/design/rule-configuration.md#摘要树) |
| TC-007 | 改动单个角色玩法数据只影响该主体摘要与根摘要 | P1 | Component | — | Configuration | 一份含剖面、角色身份表与两名角色玩法数据的完整数据集 | 只修改角色 B 的玩法数据文本后重新解析 | 角色 B 的 `A` 参数 380→382 并重新生成整数表 | 角色 B 的主体摘要与根摘要变化；剖面、角色身份表与角色 A 的主体摘要不变；差异可定位到角色 B 主体 | [Confirmed] [规则配置：摘要树](../../development/design/rule-configuration.md#摘要树) |
| TC-008 | 冻结后修改资产源数据不改变已有的 LockedMatchSpec | P1 | Component | — | Configuration | 已由 TC-001 的数据集冻结出 `LockedMatchSpec` | 修改并重新解析源文本，随后读取先前的 `LockedMatchSpec` | 修改剖面的 `clear_preview_ticks` 24→20 | 先前 `LockedMatchSpec` 的数值与摘要保持冻结时的值；重新解析产生的是另一份规格，两者互不影响 | [Confirmed] [规则配置：冻结对局规格](../../development/design/rule-configuration.md#冻结对局规格) |
| TC-009 | 规则数据缺失或校验失败时冻结入口不产生对局规格 | P0 | Component | Content Validation | Configuration | 冻结入口可接收缺失或未通过校验的数据 | 参数化提交三种不可用输入 | 剖面文本缺失；所选 `character_id` 在该剖面下无玩法数据；语义校验已失败的数据集 | 三组均返回冻结失败；不产生 `LockedMatchSpec`；不回退到任何内置默认规则数据 | [Confirmed] [规则配置：冻结对局规格](../../development/design/rule-configuration.md#冻结对局规格)；[玩法设计 §7](../../gameplay.md) |
| TC-010 | 时长以 tick 表达且 margin 由整数表下标取值 | P1 | Component | Content Validation | Configuration | 已冻结的 `LockedMatchSpec` | 读取全部时长字段与 margin 表；推进 margin 阶段后读取 `TP` | `clear_preview_ticks=24`；分裂延迟轴心 1 tick、从球 2 tick；`TP` 初值 120；margin 表首次衰减后为 90 | 全部时长字段为整数 tick，无秒或浮点字段；`TP` 由表下标查得（下标 0→120，下标 1→90）；取值路径不含实时换算 | [Confirmed] [玩法设计 §4.1、§7](../../gameplay.md)；[规则配置：数据模型](../../development/design/rule-configuration.md#数据模型) |
| TC-011 | 定义档位参数生成的两张表与原作逐点相等 | P1 | Component | Content Validation | Configuration；Rules | 原作 Fever 2 档位表已作为 fixture 录入 | 以定义档位参数生成普通盘与 Fever 盘整数表并与原作档位逐点比较 | `A=400 τ=1.00 g=0.25` 对普通盘档位（Amitie/Lemres，第 10 项 400）；`F=40 σ=1.00` 对 Fever 盘档位（Amitie/Arle，表尾 800）；普通盘原表止于饱和列，第 22～24 项取该列值 | 两张表均为 24 项且与原作对应档位含饱和段 24/24 逐点相等 | [Confirmed] [连锁强度曲线：共享形状](../../development/design/chain-power-curve.md#共享形状)；[List of attack powers](https://puyonexus.com/wiki/List_of_attack_powers) |
| TC-012 | 交叉验证档位参数生成的两张表与原作逐点相等 | P1 | Component | Content Validation | Configuration；Rules | 同 TC-011 | 以交叉验证档位参数生成两张表并与原作档位逐点比较 | `A=360 τ=1.00 g=0.25` 对普通盘档位（Dapper Bones/Hoho/Yu & Rei，第 10 项 360）；`F=36 σ=1.00` 对 Fever 盘档位（Accord，表尾 720） | 两张表均 24/24 逐点相等；Fever 表第 11、12、13 项分别为 252、259、308，这三项把形状曲线与同为 720 表尾的其它档位区分开 | [Confirmed] [连锁强度曲线：共享形状](../../development/design/chain-power-curve.md#共享形状)；[List of attack powers](https://puyonexus.com/wiki/List_of_attack_powers) |
| TC-013 | 角色配置内的整数表与其生成参数重新生成的结果一致 | P0 | Component | Content Validation | Configuration；Rules | 两名角色的配置同时含整数表与生成参数 | 以配置中的参数重新生成四张表并与配置中的整数表逐点比较 | 角色 A `A=440 τ=0.90 g=0.26` / `F=42 σ=0.95`；角色 B `A=380 τ=0.95 g=0.25` / `F=47 σ=0.92` | 四张表全部 24/24 相等；抽样点 `A.normal[10]=440`、`A.normal[15]=999`、`A.fever[24]=840`、`B.normal[17]=999`、`B.fever[24]=940` | [Confirmed] [连锁强度曲线：角色参数与生成表](../../development/design/chain-power-curve.md#角色参数与生成表) |
| TC-014 | 曲线表长、取值域与表尾外索引行为 | P1 | Component | — | Configuration；Rules | 已冻结的角色曲线 | 按连锁步查询普通盘与 Fever 盘曲线 | 连锁步 1、24、25、100；角色 A 普通盘 | 表长为 24 且全部取值落在 `[1, 999]`；步 1 得 4，步 24 得 999；步 25 与步 100 均返回表尾值 999，不越界也不回绕 | [Confirmed] [连锁强度曲线：曲线族](../../development/design/chain-power-curve.md#曲线族)；[玩法设计 §3.3](../../gameplay.md) |
| TC-015 | 手改单格的曲线在运行期通过而在 CI 校验中失败 | P1 | Component | Content Validation | Configuration；Rules | 一份有效角色玩法数据 | 只改动整数表中的一格，分别经运行期语义校验与 CI 一致性校验 | 角色 A `normal[7]` 由 170 改为 171，生成参数保持 `A=440 τ=0.90 g=0.26` | 运行期语义校验通过（表长与取值域仍合法）；CI 一致性校验失败并报出差异位置为 `normal[7]`，期望 170、实得 171 | [Confirmed] [连锁强度曲线：校验整数表与参数一致](../../development/design/chain-power-curve.md#校验整数表与参数一致) |

## 风险查漏

三层语义校验的每一层、摘要的稳定性与可定位性、冻结后的不可变性、无内置默认值，以及曲线的生成、取值域、表尾与一致性校验均有直接用例；曲线数值由定义档位与交叉验证档位双向锁定。
