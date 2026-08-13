# 测试用例设计：运行数据解析

**关联设计：** [版本化运行数据加载](../../development/design/runtime-data-loading.md)

**关联实现：** `../../../crates/game_core`、`../../../crates/client`

## 需求理解摘要

**功能：** 验证版本化 RON/JSON 内存解析器的成功结果和 typed error。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** 支持版本、malformed 内容与未支持 schema 的纯解析行为。
**Test Basis：**

- [Confirmed] [版本化运行数据加载](../../development/design/runtime-data-loading.md)：内存解析边界、schema 版本和错误语义。

**设计基线：** 以最小内存 fixture 直接调用解析器，不装配文件系统或 Bevy Asset。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义 catalog 的 locale 语义（见 [本地化](localization.md)），也不定义资产路径、加载生命周期与消费者 resolution（见 [运行数据与设置持久化](../integration-system/runtime-data.md)）。

## 测试点清单

- 支持版本的 RON 解析为 typed data（TC-001；Concern: Content Validation）。
- malformed RON/JSON 返回 Parse typed error（TC-002；Concern: Content Validation）。
- 未支持 schema 返回 UnsupportedSchema typed error（TC-003；Concern: Content Validation）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 等价类划分 | 有效、malformed 与 unsupported 输入 | TC-001～TC-003 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 支持版本的内存 RON 数据解析为 typed data | P1 | Component | Content Validation | Configuration | `game_core::config` 内存解析器 | 解析最小 `rules.stub.ron` 等价内容 | `schema_version=1` 与当前最小合法字段 | 返回对应 typed data | [Confirmed] [版本化运行数据加载：边界](../../development/design/runtime-data-loading.md#边界) |
| TC-002 | malformed RON/JSON 返回 Parse typed error | P2 | Component | Content Validation | Configuration | game_core/client 两类内存解析器 | 参数化提交损坏文本 | 截断 RON；截断 JSON | 两组均返回 Parse 类错误且保留底层原因 | [Confirmed] [版本化运行数据加载：错误语义](../../development/design/runtime-data-loading.md#错误语义) |
| TC-003 | 未支持 schema 返回 UnsupportedSchema typed error | P2 | Component | Content Validation | Configuration | 已知仅支持 schema 1 | 参数化解析 RON/JSON | `schema_version=255` | 两组均返回 UnsupportedSchema，错误携带实际版本 | [Confirmed] [版本化运行数据加载：错误语义](../../development/design/runtime-data-loading.md#错误语义) |

## 风险查漏

成功解析、语法错误和版本错误均有直接用例；文件上下文和失败分级由集成测试稿覆盖。

