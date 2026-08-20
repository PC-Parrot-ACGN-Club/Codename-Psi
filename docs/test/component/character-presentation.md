# 测试用例设计：角色表现数据

**关联设计：** [角色表现数据](../../development/design/character-presentation.md)、[版本化运行数据加载](../../development/design/runtime-data-loading.md)

**关联实现：** `../../../crates/client`

## 需求理解摘要

**功能：** 验证角色表现目录的解析、语义校验、错误分类与缺失时的替补呈现。
**测试性质：** 新功能
**本轮范围：** 从内存文本解析 `characters.ron` 并查询目录的行为，不访问文件系统。
**Test Basis：**

- [Confirmed] [角色表现数据](../../development/design/character-presentation.md)：字段约束、解析校验与替补语义。
- [Confirmed] [版本化运行数据加载](../../development/design/runtime-data-loading.md)：`DataLoadError` 分类与降级级处置。
- [Confirmed] [PRD §4.2](../../PRD.md)：角色选择影响昵称、配色、UI 标识与语音触发。

**设计基线：** 角色表现数据是降级级客户端数据，缺失时替补始终可用，角色仍可选、对局仍可完成。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义角色玩法数据与掉落组（见 [规则配置与开局规格冻结](rule-configuration.md)）、资源读取路径与 asset 生命周期（见 [运行数据与设置持久化](../integration-system/runtime-data.md)），也不定义表现数据与开局规格冻结的隔离（见 [表现运行时](../integration-system/presentation-runtime.md)）。

## 测试点清单

### Component — Configuration

- 合法目录解析成功并按 `character_id` 索引（TC-001；Concern: Content Validation）。
- 违反语义约束的目录返回 `InvalidData` 并保留违反项（TC-002～TC-004）。
- 不受支持的 schema 版本返回 `UnsupportedSchema`（TC-005）。

### Component — Client

- 目录不可用或缺少角色时使用替补并保留诊断（TC-006～TC-007）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 内容验证 | 与 `roster.ron` 一致的完整目录 | TC-001 |
| 等价类划分 | 未知 id、姿态缺项、音频键缺项、版本不支持 | TC-002～TC-005 |
| 场景法 | 目录整体不可用与目录缺少单个角色 | TC-006～TC-007 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 合法目录解析成功并覆盖 roster 全部角色 | P1 | Component | Content Validation | Configuration；Client | 可提供 roster 与表现目录文本 | 解析 `characters.ron` 并逐个查询 | 与 `assets/data/rules/roster.ron` 一致的两个 `character_id`，各含主色、辅色、徽章参数、八项姿态与六个音频键 | 解析成功；按 `character_id` 可查询到两份完整数据；八项姿态与六个音频键齐全；两个角色的主色可区分 | [Confirmed] [角色表现数据：数据模型](../../development/design/character-presentation.md#数据模型)、[解析](../../development/design/character-presentation.md#解析) |
| TC-002 | 引用 roster 中不存在的角色返回 InvalidData | P1 | Component | Content Validation | Configuration | 已知 roster 的角色集合 | 解析含未知 `character_id` 的目录 | roster 含 `alpha`、`beta`；目录含 `gamma` | 返回 `InvalidData`；错误保留违反的约束与 `gamma`；不产生部分可用目录 | [Confirmed] [角色表现数据：解析](../../development/design/character-presentation.md#解析)；[版本化运行数据加载：错误语义](../../development/design/runtime-data-loading.md#错误语义) |
| TC-003 | 姿态缺项返回 InvalidData | P1 | Component | Content Validation | Configuration | 合法 roster | 解析缺少一项姿态的目录 | 某角色缺 `fever` 姿态，其余七项齐全 | 返回 `InvalidData`；错误保留违反的约束与该 `character_id` | [Confirmed] [角色表现数据：数据模型](../../development/design/character-presentation.md#数据模型) |
| TC-004 | 音频键缺项返回 InvalidData | P2 | Component | Content Validation | Configuration | 合法 roster | 解析缺少一个音频键的目录 | 某角色缺 `fever_enter` 音频键 | 返回 `InvalidData`；错误保留违反的约束与该 `character_id` | [Confirmed] [角色表现数据：数据模型](../../development/design/character-presentation.md#数据模型) |
| TC-005 | 不受支持的 schema 版本返回 UnsupportedSchema | P2 | Component | Content Validation | Configuration | 解析器支持的版本已知 | 解析版本超出支持范围的目录 | `schema_version: 255` | 返回 `UnsupportedSchema`；不产生部分可用目录 | [Confirmed] [版本化运行数据加载：错误语义](../../development/design/runtime-data-loading.md#错误语义) |
| TC-006 | 目录不可用时全部角色使用替补且仍可选 | P0 | Component | — | Client；Configuration | 目录 resolution 为 `Failed` | 查询两个角色的表现数据 | `Failed(Parse)`；roster 含 `alpha`、`beta` | 两次查询均返回完整替补数据：配色按参与者槽位给出、徽章取 `display_name_key` 首字符、姿态为统一中性姿态、音频键为无声键；保留诊断；两个角色仍可被选择 | [Confirmed] [角色表现数据：查询](../../development/design/character-presentation.md#查询)；[版本化运行数据加载：失败分级](../../development/design/runtime-data-loading.md#失败分级) |
| TC-007 | 目录缺少单个角色时只该角色使用替补 | P1 | Component | — | Client；Configuration | 目录解析成功但只含一个角色 | 分别查询两个角色 | 目录只含 `alpha`；roster 含 `alpha`、`beta` | `alpha` 返回目录中的数据；`beta` 返回替补数据并保留一次诊断；`alpha` 的数据不被替补覆盖 | [Confirmed] [角色表现数据：查询](../../development/design/character-presentation.md#查询) |

## 风险查漏

schema 版本、引用关系、字段完整性与替补行为均有直接用例；表现数据与规则摘要的隔离由集成测试稿覆盖。
