# 开发设计分类与模板

**适用范围：** 本项目的模块设计、接口协作、架构、流程与设计审核

## 1. 目的

设计文档按装配范围分类，使设计确认、实现边界和测试范围使用同一套语言。每份文档选择一个主分类；跨范围设计可列出关联文档与次分类。

| 分类 | 设计范围 | 主要确认内容 | 对应测试层级 |
| --- | --- | --- | --- |
| Component | 单个模块、数据模型、规则、局部状态与公开行为。 | 职责、输入输出、状态、不变量与错误语义。 | Component |
| Component Integration | 两个以上组件的接口、契约与协作流程。 | 调用顺序、数据语义、所有权、错误传播与协作结果。 | Component Integration |
| System | 多模块组成的架构、运行时生命周期与用户流程。 | 模块边界、依赖方向、状态流、关键闭环与验收结果。 | System |

测试分类标准见[测试策略与分类标准](../test/README.md)。设计分类表达实现所需的装配范围；测试层级表达验证行为所需的装配范围。

## 2. 文档类型与分类

| 文档类型 | 用途 | 常用分类 | 产出 |
| --- | --- | --- | --- |
| Spec | 定义一个行为、规则或能力的可观察语义。 | Component、Component Integration、System | 行为范围、输入输出、状态变化、验收条件。 |
| Contract | 定义边界两侧共同遵守的承诺。 | Component、Component Integration、System | 调用者与实现方职责、数据结构、时序、错误语义与版本演进规则。 |
| Architecture | 定义多模块的职责、依赖方向与运行时组合。 | System | 模块图、依赖约束、状态流、启动与终止路径。 |
| Flow | 定义由事件或用户动作推进的业务流程。 | Component Integration、System | 参与者、触发条件、状态迁移、分支与完成条件。 |
| Decision | 记录已确认的设计选择及其依据。 | 任意分类 | 决策、背景、备选方案、影响范围与后续动作。 |

Contract 的分类由承诺覆盖的边界决定。单模块公开 API 的契约属于 Component；两个组件之间的协议属于 Component Integration；跨模块生命周期与运行时规则属于 System。

## 3. 通用编写规则

每份设计文档在开头包含以下元数据：

```markdown
**状态：** Draft | 等待审核 | Confirmed | Replaced
**主分类：** Component | Component Integration | System
**次分类：** 可选
**相关模块：** crate、模块或运行时参与者
**关联文档：** 设计依据、契约或流程
```

设计结论附 Test Basis：用户当前任务与已确认决定、当前有效 PRD/玩法/TDD、已确认审核结论、当前代码与配置、既有测试与 Git 历史。引用同时标记 `Confirmed`、`Inferred` 或 `Unknown`，并定位到来源章节。

一个文档中的验收条件使用可观察结果描述。每项验收条件可映射到 [测试策略与分类标准](../test/README.md) 的 Test Level、Concern 与 Domain。

用正向职责和有效条件定义范围；只有存在真实歧义、冲突所有权或安全约束时，才写显式排除项。

## 4. 模板

[模板目录](template/README.md)提供可直接复制的文档骨架。

| 文档类型 | 模板 |
| --- | --- |
| Component Spec | [component-spec.md](template/component-spec.md) |
| Component Integration Contract | [component-integration-contract.md](template/component-integration-contract.md) |
| System Architecture 与 Flow | [system-architecture-flow.md](template/system-architecture-flow.md) |
| Decision | [decision.md](template/decision.md) |

## 5. 审核顺序

1. 确认主分类与关联模块。
2. 确认 Test Basis、引用与证据状态。
3. 确认行为、契约、状态变化与验收条件。
4. 确认对应的测试分类与实现切片。
5. 将文档状态更新为 `Confirmed`，并同步关联文档。
