# 角色表现数据

**相关模块：** `client::data`、`client::presentation`
**关联文档：** [规则配置与开局规格冻结](rule-configuration.md)、[版本化运行数据加载](runtime-data-loading.md)、[表现运行时](presentation-runtime.md)、[表现与 UI 设计 §3.1、§4.5](../../presentation.md)、[PRD §4.2、§7](../../PRD.md)

## 目标

为每个角色提供玩家可见的配色、徽章、姿态与音频键，使角色标识与演出可以在不改动规则数据的前提下调整。

## 数据模型

角色表现数据保存在 `assets/data/presentation/characters.ron`，带 `schema_version`。

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `character_id` | 与[角色身份](rule-configuration.md#数据模型)相同的角色 ID | 必须存在于 `roster.ron` |
| 主色与辅色 | 角色标识、边框与轨道演出使用的配色 | 与另一角色、双方槽位配色可区分 |
| 徽章图形参数 | 圆框内原创剪影或简化头像的几何参数 | 由图元绘制，不引用位图资源 |
| 姿态集合 | 待机、进攻、抵消、防御、紧张、Fever、胜利、失败 | 八项齐全，缺一即为无效数据 |
| 音频键集合 | 开局、消除、连锁、进入 Fever、胜利、失败的稳定键 | 只是键，不含音频资源 |

配色、徽章、姿态与音频键都是客户端表现数据：它们不进入 `LockedMatchSpec` 的摘要树，改动不影响开局规格冻结与确定性校验。角色的掉落组与连锁强度曲线仍以 `assets/data/rules/` 为权威。

## 行为

### 解析

- 输入：`assets/data/presentation/characters.ron` 的已读取内容。
- 处理：反序列化、检查 `schema_version`，校验每个条目的 `character_id` 存在于 `roster.ron`、八项姿态齐全、音频键齐全。
- 输出：按 `character_id` 索引的角色表现目录。
- 错误语义：违反上述约束时返回 [`InvalidData`](runtime-data-loading.md#错误语义)，保留违反的约束与 `character_id`。

### 查询

- 输入：`character_id`。
- 处理：从目录取出该角色的表现数据；目录不可用或缺少该角色时取内置替补。
- 输出：一份完整的角色表现数据。
- 错误语义：使用替补时保留一次诊断，不阻止角色被选择或进入对局。

内置替补按参与者槽位给出配色，以角色 `display_name_key` 的首字符作为徽章，姿态退化为统一的中性姿态，音频键退化为无声键。因此角色表现数据缺失时角色仍可选、对局仍可完成，只失去角色间的视觉区分度。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| 角色表现目录 | `client::data` | [表现运行时](presentation-runtime.md) | 降级级数据，缺失时使用替补 |
| `character_id` | [开局规格冻结](rule-configuration.md) | 本主题 | 一场对局内不变 |
| 音频键 | 本主题 | [表现运行时：音频与震动](presentation-runtime.md#音频与震动) | R1 无对应音频资源 |

## 边界

- 本文不定义角色的掉落组、连锁强度曲线与任何影响规则的数值（见[规则配置与开局规格冻结](rule-configuration.md)）。
- 本文不定义资源读取、schema 检查与失败分级机制（见[版本化运行数据加载](runtime-data-loading.md)）。
- 本文不定义姿态的触发时机与演出方式（见[表现与 UI 设计 §3.1](../../presentation.md)、[表现运行时](presentation-runtime.md)）。
- 本文不定义角色显示名的文本（见[本地化运行时](localization-runtime.md)）；本主题只使用 `roster.ron` 中的 `display_name_key`。

## Test Basis

- [Issue #13](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/13)：要求两位原创角色可被选择，并正确影响自己的角色标识与掉落组；无正式美术资源时仍可完整游玩。
- [PRD §4.2](../../PRD.md)：角色选择影响昵称、配色、UI 标识、语音触发与掉落组。
- [PRD §7](../../PRD.md)：角色定义以数据驱动方式保存显示名、本地化键、颜色、语音事件资源键与 UI 图标参数。
- [表现与 UI 设计 §4.5](../../presentation.md)：每个角色至少提供八个姿态，静态首版由同一基础图配合位移、缩放、边框与遮罩完成。
