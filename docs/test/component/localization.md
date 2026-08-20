# 测试用例设计：本地化

**关联设计：** [本地化运行时](../../development/design/localization-runtime.md)、[版本化运行数据加载](../../development/design/runtime-data-loading.md)

**关联实现：** `../../../crates/client`

## 需求理解摘要

**功能：** 验证本地化查询、英文回退、诊断以及 catalog 的解析和语义校验。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** 内存中的 `Localization` 与 catalog 解析器行为。
**Test Basis：**

- [Confirmed] [本地化运行时](../../development/design/localization-runtime.md)：默认语言、文本查询、回退、切换和 catalog 校验。
- [Confirmed] [版本化运行数据加载](../../development/design/runtime-data-loading.md)：解析、版本与语义错误分类。

**设计基线：** 以可控文本目录和 JSON 输入验证查询及错误分类，不依赖真实资产加载生命周期。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义 Bevy 资产请求、轮询和 resolution（见 [运行数据与设置持久化](../integration-system/runtime-data.md)），也不定义启动屏障的降级行为（见 [应用生命周期](../integration-system/application-lifecycle.md)）。

## 测试点清单

- 默认语言、当前目录查询、英文回退和 key 占位（TC-001～TC-004）。
- 语言切换后的后续查询（TC-005）。
- catalog 的有效、malformed、unsupported 与无效 locale 输入（TC-006～TC-007；Concern: Content Validation）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 等价类划分 | 当前语言命中、英文回退、全部缺失 | TC-001～TC-004 |
| 状态迁移 | 切换 locale 后的查询结果 | TC-005 |
| 等价类划分 | 有效、malformed、unsupported、invalid catalog | TC-006～TC-007 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 无有效语言设置时本地化默认使用英文 | P1 | Component | — | Client；Configuration | 英文目录已构建，无有效用户 locale | 初始化 `Localization` 并查询已知 key | `main_menu.start` | 当前 locale 为 `en`，返回英文文本 | [Confirmed] [本地化运行时：数据模型](../../development/design/localization-runtime.md#数据模型) |
| TC-002 | 当前语言存在 key 时直接返回当前语言文本 | P1 | Component | — | Client；Configuration | `zh-CN` 与 `en` 均含相同 key，当前 locale=`zh-CN` | 查询 key | `main_menu.start`：中文=`开始`，英文=`Start` | 返回 `开始`，无 missing-key 诊断 | [Confirmed] [本地化运行时：查询文本](../../development/design/localization-runtime.md#查询文本) |
| TC-003 | 当前语言缺 key 时回退英文并记录诊断 | P1 | Component | — | Client；Configuration | 中文缺 key，英文含 key，当前 locale=`zh-CN` | 查询 key | `main_menu.settings` 仅英文=`Settings` | 返回 `Settings`；诊断包含 locale=`zh-CN` 与该 key | [Confirmed] [本地化运行时：查询文本](../../development/design/localization-runtime.md#查询文本) |
| TC-004 | 当前语言和英文均缺 key 时返回 key 并记录诊断 | P1 | Component | — | Client；Configuration | 两目录均缺目标 key | 查询 key | `missing.example` | 返回 `missing.example`；诊断包含 locale 与 key | [Confirmed] [本地化运行时：查询文本](../../development/design/localization-runtime.md#查询文本) |
| TC-005 | 切换语言后后续查询使用新目录 | P1 | Component | — | Client；Configuration | 两种目录均加载，当前 locale=`en` | 查询一次，切换至 `zh-CN`，再次查询 | `main_menu.start` | 首次返回 `Start`，切换后返回中文文本，资源保持可只读查询 | [Confirmed] [本地化运行时：切换语言](../../development/design/localization-runtime.md#切换语言) |
| TC-006 | catalog 有效、malformed 与 unsupported 输入得到对应解析结果 | P1 | Component | Content Validation | Configuration；Client | catalog 内存解析器 | 参数化解析三份 JSON | 有效 schema 1；截断 JSON；schema 255 | 有效输入得到 locale 和 messages；其余分别得到 Parse、UnsupportedSchema 等价错误，由加载层按降级级处置 | [Confirmed] [本地化运行时：加载文本目录](../../development/design/localization-runtime.md#加载文本目录) |
| TC-007 | catalog locale 不属于支持集合时返回 InvalidData | P1 | Component | Content Validation | Configuration；Client | 本地化 catalog 内存解析器支持 schema 1，客户端支持 locale 集合为 `zh-CN`、`en` | 解析结构合法、schema 受支持且 locale 不受支持的 JSON catalog | `schema_version=1`；`locale=fr`；`messages={}` | 返回 InvalidData；错误标明 locale 必须属于当前支持集合，并保留实际值 `fr` | [Confirmed] [本地化运行时：语义验证](../../development/design/localization-runtime.md#语义验证)；[版本化运行数据加载：错误语义](../../development/design/runtime-data-loading.md#错误语义) |

## 风险查漏

默认语言、两级缺失回退、切换、schema 和 locale 语义均有直接用例；真实资产路径与启动降级由集成测试稿覆盖。

