# 测试用例设计：应用状态表

**关联设计：** [应用状态机](../../development/design/application-state-machine.md)

**关联实现：** `../../../crates/client`

## 需求理解摘要

**功能：** 验证应用状态表对合法边和表外边的纯判定。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** 不装配 Bevy 生命周期的状态边验证。
**Test Basis：**

- [Confirmed] [应用状态机](../../development/design/application-state-machine.md)：基础状态与有效状态转移。

**设计基线：** 参数化遍历基础状态表，并为每个源状态提供至少一个表外目标。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义状态提交、生命周期、请求去重和仲裁（见 [应用生命周期](../integration-system/application-lifecycle.md)）。

## 测试点清单

- 基础状态表的每条合法边均有效（TC-001）。
- 每个源状态的代表性表外目标均非法（TC-002）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 状态迁移 | 完整合法边和各源状态的表外目标 | TC-001～TC-002 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 基础状态表的每条合法边均被判为有效 | P1 | Component | — | Client | 可对纯状态表执行合法边判断 | 参数化判断每条基础状态边 | Boot→MainMenu；MainMenu→ModeSelect；ModeSelect→CharacterSelect；CharacterSelect→Match；Match→Paused；Paused→Match；Match→Result；Result→MainMenu | 每组均被判为有效边 | [Confirmed] [应用状态机：有效状态转移](../../development/design/application-state-machine.md#有效状态转移) |
| TC-002 | 表外状态边均被判为非法 | P2 | Component | — | Client | 可对纯状态表执行合法边判断 | 参数化判断未列入对应源状态允许目标的边 | 至少每个源状态一个表外目标，含 Boot→Match、Paused→Result、Result→Match | 每组均被判为非法边 | [Confirmed] [应用状态机：有效状态转移](../../development/design/application-state-machine.md#有效状态转移) |

## 风险查漏

基础合法边与代表性非法边均有直接用例；运行时提交行为由集成测试稿覆盖。

