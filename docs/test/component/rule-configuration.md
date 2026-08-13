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

- 有效剖面与内容库解析并冻结成两人对局规格（Concern: Content Validation）。
- 断引用、越界坐标、非法时间、缺失题面与非法掉落组各返回稳定错误类别（Concern: Content Validation）。
- 相同规范内容得到相同摘要；字段顺序与 RON 空白变化不改变摘要。
- 只改动一个角色玩法数据文件时，该主体摘要与根摘要变化，其余主体摘要不变。
- 资产源数据在加载完成后被修改，不改变已创建的 `LockedMatchSpec`。
- 规则数据缺失或校验失败时冻结入口返回失败，且不产生 `LockedMatchSpec`（Concern: Content Validation）。
- 所有时长配置以 tick 表达；margin 按整数表取值，运行期不出现实时换算。

### Component — Configuration；Rules

- 以 `A=400 τ=1.00 g=0.25` 与 `F=40 σ=1.00` 生成的两张表与原作对应档位含饱和段逐点相等（Concern: Content Validation）。
- 以 `A=360 τ=1.00 g=0.25` 与 `F=36 σ=1.00` 生成的两张表与原作对应档位含饱和段逐点相等（Concern: Content Validation）。
- 每个角色配置内的整数表与其生成参数重新生成的结果逐点相等。
- 曲线表长为 24、取值落在 `[1, 999]`，连锁步超过 24 时取表尾值。
- 手改单格的配置能被运行期接受、被 CI 拒绝。

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
