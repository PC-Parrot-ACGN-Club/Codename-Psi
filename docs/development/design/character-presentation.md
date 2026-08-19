# 角色表现数据

**相关模块：** `client::data`、`client::presentation`
**关联文档：** [规则配置与开局规格冻结](rule-configuration.md)、[版本化运行数据加载](runtime-data-loading.md)、[表现运行时](presentation-runtime.md)、[圆框运动模型](portrait-motion.md)、[表现与 UI 设计 §3.1、§4.5](../../presentation.md)、[PRD §4.2、§7](../../PRD.md)

## 目标

为每个角色提供玩家可见的配色、立绘、cut-in 与音频键，使角色标识与演出可以在不改动规则数据的前提下调整。

## 数据模型

角色表现数据保存在 `assets/data/presentation/characters.ron`，带 `schema_version`；立绘位图保存在 `assets/portraits/<character_id>/`。

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `character_id` | 与[角色身份](rule-configuration.md#数据模型)相同的角色 ID | 必须存在于 `roster.ron` |
| 主色与辅色 | 角色标识、圆框边框、名条与替补徽章使用的配色 | 与另一角色、双方槽位配色可区分 |
| 替补徽章 | 立绘句柄不可用时的替补图形 | 取 `display_name_key` 首字符，由图元绘制，不引用位图资源 |
| 姿态立绘集合 | 角色框常驻的 5 项位图：`idle`、`spell`、`offset`、`damage`、`advantage` | 五项齐全，缺一即为无效数据 |
| 连锁 cut-in 集合 | 覆盖棋盘的 5 项位图：`cutin-fever`、`cutin-chain-2-3`、`cutin-chain-4-5`、`cutin-chain-6-7`、`cutin-chain-8-plus` | 五项齐全 |
| 对局结束立绘集合 | 棋盘位置的 2 项位图：`win`、`lose` | 两项齐全 |
| 立绘资源路径 | 上述十二项各自的贴图路径 | `assets/portraits/<character_id>/<slot>.png`，`slot` 取上述项名 |
| 姿态 / 结局画布 | `姿态立绘集合`、`对局结束立绘集合` 共用的画布规格 | 512×512 像素 RGBA PNG；圆框直径 320 居中，主体收在直径 380 的安全区内，发梢、四肢、道具允许延伸到画布边缘 |
| cut-in 画布 | `连锁 cut-in 集合` 的画布规格 | 720×960 像素 RGBA PNG，人物构图居中偏下，顶部预留连锁数文字区域 |
| 音频键集合 | 开局、消除、连锁、进入 Fever、胜利、失败的稳定键 | 只是键，不含音频资源 |

配色、立绘、cut-in 与音频键都是客户端表现数据：它们不进入 `LockedMatchSpec` 的摘要树，改动不影响开局规格冻结与确定性校验。角色的掉落组与连锁强度曲线仍以 `assets/data/rules/` 为权威。

## 行为

### 解析

- 输入：`assets/data/presentation/characters.ron` 的已读取内容，以及 `assets/portraits/<character_id>/` 下的位图资源。
- 处理：反序列化、检查 `schema_version`，校验每个条目的 `character_id` 存在于 `roster.ron`、姿态 / cut-in / 对局结束三组共十二项路径齐全、音频键齐全；按命名约定为每项发起位图加载。
- 输出：按 `character_id` 索引的角色表现目录，每项姿态 / cut-in / 结局携带一个图片句柄。
- 错误语义：RON 结构违反上述约束时整份数据返回 [`InvalidData`](runtime-data-loading.md#错误语义)，保留违反的约束与 `character_id`；单张位图缺失不影响这一步的校验，由「立绘解析」处理。

### 查询

- 输入：`character_id`。
- 处理：从目录取出该角色的主色、辅色与替补徽章；目录不可用或缺少该角色时取内置替补。
- 输出：一份角色标识数据（配色与替补徽章）。
- 错误语义：使用替补时保留一次诊断，不阻止角色被选择或进入对局。

内置替补按参与者槽位给出配色，以角色 `display_name_key` 的首字符作为徽章，音频键退化为无声键。因此角色表现数据缺失时角色仍可选、对局仍可完成，只失去角色间的视觉区分度。

目录尚未发布时的查询同样得到替补，但该结果是暂定的：目录发布后必须重新查询一次。表现层按对局实例缓存查询结果，因此缓存条件同时取决于对局实例与目录是否已发布——只看实例会让一场在目录发布前开始的对局整局停在替补上。

### 立绘解析

- 输入：`character_id`，目标 slot（姿态 / cut-in / 对局结束三组之一）。
- 处理：取该角色目录中对应 slot 的图片句柄；句柄尚未加载完成或加载失败时，姿态类 slot 退回该角色的 `idle` 句柄，cut-in 与对局结束类 slot 退回[替补](#查询)绘制；`idle` 句柄本身不可用时同样退回替补。
- 输出：一个可绘制的图片句柄，或替补绘制指令。
- 错误语义：单张位图缺失只影响该项显示，不阻止其余姿态或角色被选择、不阻止对局进行；缺失诊断按 `character_id` + `slot` 记一次，不逐帧重复。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| 角色表现目录 | `client::data` | [表现运行时：圆框角色演出](presentation-runtime.md#圆框角色演出) | 降级级数据，缺失时使用替补；校验需要 roster，因此在规则解析之后发布 |
| `character_id` | [开局规格冻结](rule-configuration.md) | 本主题 | 一场对局内不变 |
| 姿态 / cut-in / 结局立绘句柄 | 本主题（立绘解析） | [表现运行时：圆框角色演出、连锁与 Fever cut-in、对局结束立绘](presentation-runtime.md#圆框角色演出) | 句柄未就绪或缺失时退回替补，不阻塞演出触发本身 |
| 音频键 | 本主题 | [表现运行时：音频与震动](presentation-runtime.md#音频与震动) | R1 无对应音频资源 |

## 边界

- 本文不定义角色的掉落组、连锁强度曲线与任何影响规则的数值（见[规则配置与开局规格冻结](rule-configuration.md)）。
- 本文不定义资源读取、schema 检查与失败分级机制（见[版本化运行数据加载](runtime-data-loading.md)）。
- 本文不定义姿态、cut-in 与对局结束立绘的触发时机与演出方式（见[表现与 UI 设计 §3.1](../../presentation.md)、[表现运行时](presentation-runtime.md)）。
- 本文不定义角色显示名的文本（见[本地化运行时](localization-runtime.md)）；本主题只使用 `roster.ron` 中的 `display_name_key`。

## Test Basis

- [Issue #13](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/13)：要求两位原创角色可被选择，并正确影响自己的角色标识与掉落组；无正式美术资源时仍可完整游玩。
- [PRD §4.2](../../PRD.md)：角色选择影响昵称、配色、UI 标识、语音触发与掉落组。
- [PRD §7](../../PRD.md)：角色定义以数据驱动方式保存显示名、本地化键、颜色、语音事件资源键与 UI 图标参数。
- [表现与 UI 设计 §4.5](../../presentation.md)：每个角色至少提供角色框姿态、连锁 / Fever cut-in、对局结束立绘共十二项位图。
