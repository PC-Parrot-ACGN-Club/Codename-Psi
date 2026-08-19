# 圆框运动模型

**相关模块：** `client::presentation`
**关联文档：** [角色表现数据](character-presentation.md)、[表现运行时：圆框角色演出、动画强度](presentation-runtime.md#圆框角色演出)、[表现与 UI 设计 §3.1](../../presentation.md)、[本机用户设置](user-settings.md)、[TDD](../../TDD.md)

## 目标

为两侧圆框的姿态切换提供可参数化的运动，把[圆框角色演出](presentation-runtime.md#圆框角色演出)选出的姿态与一次性事件转成随 tick 演进的位移、旋转与振幅，并规定其在两档动画强度下如何缩放。

## 数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `MotionKind` | 运动词汇 | `Float`、`Sway`、`Tremble`、`Hop`、`CeilingBump`、`DecayVibration`、`Flip`，见「运动词汇表」 |
| `MotionCategory` | 运动的时间结构 | `Continuous`（随姿态状态持续播放）或 `Impulse`（一次性，播完停止） |
| `MotionSpec` | 一个 `MotionKind` 的参数 | `amplitude`（像素或角度）、`duration_ticks`（`Continuous` 为一个周期，`Impulse` 为播放总时长）、`curve`（缓动标识：`Linear`、`EaseOutIn`、`Decay`） |
| `elapsed_ticks` | 该运动自进入以来经过的规则 tick 数 | 从运动开始 tick 起算，不是 wall-clock；`Continuous` 对 `duration_ticks` 取模，`Impulse` 超出 `duration_ticks` 视为已结束 |

### 运动词汇表

| `MotionKind` | 分类 | `amplitude` | `duration_ticks` | `curve` |
| --- | --- | --- | --- | --- |
| `Float` | `Continuous` | 6px 垂直位移 | 90 | `Linear`（正弦相位） |
| `Sway` | `Continuous` | 8° 旋转 | 120 | `Linear`（正弦相位） |
| `Tremble` | `Continuous` | 3px 水平抖动 | 6 | `Linear`（正弦相位） |
| `Hop` | `Impulse` | 16px 垂直位移 | 24 | `EaseOutIn` |
| `CeilingBump` | `Impulse` | 12px 垂直位移（反向） | 24 | `EaseOutIn` |
| `DecayVibration` | `Impulse` | 10px 水平位移，振幅线性衰减至 0 | 30 | `Decay` |
| `Flip` | `Impulse` | 180° 绕竖直轴旋转 | 12 | `EaseOutIn` |

`Reduced` 强度下所有 `amplitude` 乘以 0.5，`duration_ticks` 不变。

## 行为

### 运动选择

- 输入：该玩家本 tick 的姿态（`idle`/`spell`/`offset`/`damage`/`advantage`，见[圆框角色演出](presentation-runtime.md#圆框角色演出)）、姿态相对上一 tick 是否发生 slot 切换、本次姿态进入是否携带触发事件（Chain 结算、`NuisanceDropped`）。
- 处理：姿态映射到其常驻 `Continuous` 运动——`idle` → `Float`，`advantage` → `Sway`，`damage` 由 `overflow_risk` 持续触发时 → `Tremble`；`spell`、`offset` 与由 `NuisanceDropped` 触发的 `damage` 没有常驻运动，落回 `Float` 基线。姿态发生 slot 切换的同一 tick 叠加一次 `Flip`；`spell` 进入时叠加一次 `Hop`，`offset` 进入时叠加一次方向相反的 `CeilingBump`，`NuisanceDropped` 触发的 `damage` 进入时叠加一次 `DecayVibration`。
- 输出：该玩家圆框本 tick 应播放的一条 `Continuous` 运动，加至多一条 `Impulse` 运动。
- 错误语义：不存在对应姿态数据时圆框保持上一帧的运动状态，不产生新位移。

`Flip` 与该次姿态切换的 `Impulse`（`Hop`/`CeilingBump`/`DecayVibration`，若有）同一 tick 起播，`Flip` 的 12 tick 短于其余三者的 24–30 tick，因此新姿态立绘在旧运动播放过程中换面完成，不需要等旧运动播完。

### 补间求值

- 输入：`MotionSpec`、`elapsed_ticks`、当前 [`AnimationIntensity`](user-settings.md#数据模型)。
- 处理：`Continuous` 运动按 `elapsed_ticks mod duration_ticks` 求相位；`Impulse` 运动按 `elapsed_ticks / duration_ticks` 求归一化进度并套 `curve`；`Reduced` 强度对 `amplitude` 乘 0.5，不改变 `duration_ticks` 与相位计算。
- 输出：该运动本 tick 的位移、旋转或振幅增量，供渲染叠加到圆框基准位置。
- 错误语义：`elapsed_ticks` 超出 `Impulse` 的 `duration_ticks` 时该运动视为已结束，输出零增量；不影响 `Continuous` 运动继续播放。

一名玩家的圆框同时至多播放一条 `Continuous` 与一条 `Impulse`：两者的输出增量相加后应用，互不覆盖。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| 姿态、slot 切换、触发事件 | [表现运行时：圆框角色演出](presentation-runtime.md#圆框角色演出) | 本主题（运动选择） | 每 tick 读取最新一份 |
| `AnimationIntensity` | [本机用户设置](user-settings.md) | 本主题（补间求值） | 立即生效，只缩放 `amplitude` |
| 圆框位移 / 旋转增量 | 本主题 | 画面重建 | 叠加到圆框基准位置渲染，不改变常驻信息的布局位置 |

## 边界

- 本文不定义姿态优先级与触发条件（见[表现运行时：圆框角色演出](presentation-runtime.md#圆框角色演出)）。
- 本文的运动状态、`elapsed_ticks` 与相位不进入确定性校验状态或快照 checksum（见 [TDD](../../TDD.md)、[确定性与快照](determinism-and-snapshot.md)）：同一姿态与事件序列下运动结果可重算，但重算结果本身不参与校验。

## Test Basis

- [表现与 UI 设计 §3.1](../../presentation.md)：运动分持续性周期运动与一次性冲击运动两类，一次性运动的时长必须短于最短的一个连锁步。
- [表现运行时：动画强度](presentation-runtime.md#动画强度)：两档动画强度下规则时序不变，`Reduced` 只缩减幅度。
