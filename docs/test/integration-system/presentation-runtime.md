# 测试用例设计：表现运行时

**关联设计：** [表现运行时](../../development/design/presentation-runtime.md)、[页面导航与焦点](../../development/design/page-navigation.md)、[角色表现数据](../../development/design/character-presentation.md)、[表现与 UI 设计](../../presentation.md)

**关联实现：** `../../../crates/client`

## 需求理解摘要

**功能：** 验证画面从快照重建、页面实体生命周期、虚拟画布缩放、设备与资源缺失时的降级，以及表现配置不改变规则结果。
**测试性质：** 新功能
**本轮范围：** 表现系统与规则、设置、页面状态协作的运行时行为，以及需要完整客户端才能证明的表现不变式。
**Test Basis：**

- [Confirmed] [表现运行时](../../development/design/presentation-runtime.md)：画面重建、动画强度、音频与震动降级、表现预算与虚拟画布。
- [Confirmed] [页面导航与焦点：页面实体生命周期](../../development/design/page-navigation.md#页面实体生命周期)：page entity 与对局表现实体的不同绑定。
- [Confirmed] [表现与 UI 设计 §6.1、§9](../../presentation.md)：跳帧采样、留边不重排与规则时序不变。
- [Confirmed] [PRD §5.3](../../PRD.md)：无外部美术资源时仍可完成可玩流程。

**设计基线：** 表现层是规则事实的消费者；任何表现配置、设备条件或帧率变化都不得改变 fixed tick 的规则结论。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义快照与事件的构造与编号（见 [表现快照与表现事件](../component/presentation-snapshot.md)）、焦点与页面动作判定（见 [页面导航与焦点](../component/page-navigation.md)）、角色表现数据的解析与替补（见 [角色表现数据](../component/character-presentation.md)），也不定义真实窗口启动（见 [构建与启动](build-and-startup.md)）。

## 测试点清单

### Component Integration — Client

- 任意规则阶段都能只从最新快照重建画面，跳帧只采样最新阶段（TC-001～TC-002）。
- page entity 随状态退出销毁，对局表现实体跨 `Paused` 与 `Settings` 保持存在（TC-003）。
- 虚拟画布在非 16:9 窗口只留边不重排，两条缩放路径使用同一因子（TC-004）。
- 音频设备与手柄缺失时静默降级（TC-005～TC-006）。
- 每个规则事实请求一条对应种类的 cue，增益与震动按设置与设备门控（TC-013）。
- 装上 UI 栈后，页面实体随状态退出释放、HUD 随对局实例释放、一次性痕迹自行到期（TC-014）。
- 圆框下的名字来自 roster 的 `display_name_key` 并经当前语言解析（TC-015）。
- 目录在对局开始之后才发布时，圆框由替补换成角色自己的配色与徽章（TC-016）。
- 高反馈场景下短命实体与并发 cue 不超预算（TC-007）。
- 角色表现数据不进入开局规格摘要（TC-008；Concern: Determinism）。

### System — Client

- 多种表现配置下相同输入得到相同校验和与赛果（TC-009；Concern: Determinism）。
- 无正式美术、无音频设备、无手柄时仍可用键盘完成对局（TC-010；Concern: Smoke）。
- 球体线索按色觉辅助设置分档，NEXT 预览与棋盘使用同一套线索（TC-011）。
- 两个有时长的结算阶段按进度投影，并按动画强度分档（TC-012）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 场景 / 协作路径 | 三种规则阶段的重建与跨状态的实体存活 | TC-001、TC-003 |
| 边界值 | 渲染落后一个阶段与落后多个阶段；窗口比例边界 | TC-002、TC-004 |
| 等价类划分 | 音频设备可用/不可用；手柄有/无 | TC-005～TC-006 |
| 判定表 | cue 种类 × 两级音量 × 震动设置 × 手柄数 | TC-013 |
| 场景 / 协作路径 | 逐页进入再逐页退回；对局—暂停—赛果的实体存活 | TC-014 |
| 时序 | 降级级数据在对局开始前 / 后发布 | TC-016 |
| 数据流 | roster → `display_name_key` → 本地化目录 → 屏上文本 | TC-015 |
| 判定表 | 色觉辅助开/关 × 普通球/垃圾球/空格；四种手牌形状 | TC-011 |
| 边界值分析 | 阶段进度取 0、命中占比、1 与越界值；两档动画强度 | TC-012 |
| 压力场景 | 大量垃圾与高连锁同时发生 | TC-007 |
| 对比测试 | 表现配置矩阵下的同一输入日志 | TC-008～TC-009 |
| 错误猜测 | 全部可选资源与设备同时缺失 | TC-010 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 任意规则阶段都能只从最新快照重建画面 | P0 | Component Integration | — | Client | 最小客户端 app，可注入任意 `MatchView` | 在三种阶段各清空全部表现实体后只用最新快照重建，并与连续运行的结果比较 | 普通场落子中；Fever 场进行中；`ClearPreview` 结算中 | 三种阶段重建后的双方棋盘、活动组、NEXT、两条队列数量、Fever 面板、比分与角色标识与连续运行的结果一致；重建不依赖历史 `PresentationEvent` | [Confirmed] [表现运行时：画面重建](../../development/design/presentation-runtime.md#画面重建)；[表现与 UI 设计 §9](../../presentation.md) |
| TC-002 | 渲染落后时直接采样最新阶段且不补播中间帧 | P1 | Component Integration | — | Client | Match 中，表现更新次数可控 | 在一次表现更新之间推进跨越多个结算阶段的规则 tick | 表现更新之间推进 `ClearPreview → ClearCommit → Gravity` 三个阶段 | 画面直接呈现最新阶段；不逐个补播被跨过的阶段；跨越阶段边界时以最新快照重建盘面；规则 tick 数与不跳帧时相同 | [Confirmed] [表现运行时：画面重建](../../development/design/presentation-runtime.md#画面重建)；[表现与 UI 设计 §6.1](../../presentation.md) |
| TC-003 | page entity 随状态退出销毁，对局表现实体跨 Paused 与 Settings 存活 | P0 | Component Integration | — | Client | 已装配页面与对局表现实体，实体数量可观测 | 依次执行 `Match → Paused → Settings → Paused → Match` 与 `MainMenu → Settings → MainMenu` | 各页面的 page entity；对局棋盘与 HUD 实体 | 每次状态退出后该状态的 page entity 全部销毁；对局表现实体在 `Paused` 与 `Settings` 期间保持存在并可见；返回 `Match` 后未被重建；`MainMenu ↔ Settings` 往返后不残留上一页面的实体 | [Confirmed] [页面导航与焦点：页面实体生命周期](../../development/design/page-navigation.md#页面实体生命周期) |
| TC-004 | 非 16:9 窗口只留边不重排且两条缩放路径同因子 | P1 | Component Integration | — | Client | 可设置逻辑窗口尺寸并读取缩放因子与布局结果 | 参数化设置四种窗口尺寸并计算布局 | `1920×1080`；`2560×1440`；`1280×1024`（4:5 偏方）；`2560×1080`（超宽） | 缩放因子等于 `min(w/1920, h/1080)`；UI 缩放与 2D 图元缩放使用同一因子；四种尺寸下各元素在 1920×1080 画布坐标中的相对位置相同，不发生重排；非 16:9 尺寸出现留边 | [Confirmed] [表现运行时：虚拟画布](../../development/design/presentation-runtime.md#虚拟画布)；[表现与 UI 设计 §2.2](../../presentation.md) |
| TC-005 | 音频设备不可用时静默降级并只保留一次诊断 | P1 | Component Integration | — | Client | 音频输出不可用 | 触发若干会产生音频 cue 的表现事件 | 连续 10 个 cue，其中同类 cue 重复出现 | 不产生播放错误导致的中断；保留一次可观察诊断且不逐次重复报告；对局继续推进；规则 tick 数不变 | [Confirmed] [表现运行时：音频与震动](../../development/design/presentation-runtime.md#音频与震动) |
| TC-006 | 无手柄时不产生震动也不产生诊断 | P2 | Component Integration | — | Client；Input | 未连接任何手柄，`vibration=true` | 触发会产生震动的表现事件 | 连锁、垃圾落下、Fever 进入与胜负各一次 | 不产生震动请求；不产生诊断；对局继续推进 | [Confirmed] [表现运行时：音频与震动](../../development/design/presentation-runtime.md#音频与震动) |
| TC-007 | 高反馈场景下短命实体与并发 cue 不超预算且不改变规则 tick | P2 | Component Integration | — | Client；Rules | Match 中可构造高强度表现场景 | 在同一 tick 触发大量垃圾落下与高连锁 | 单 tick 内 30 行垃圾落下 + 8 连锁 | 短命实体与并发 cue 数量均不超过各自上限；同 tick 的多次消除合并为一次粒子批、一个连锁数字与一次 cue；该 tick 的规则结论与低强度场景下的同输入结果一致 | [Confirmed] [表现运行时：表现预算](../../development/design/presentation-runtime.md#表现预算) |
| TC-008 | 角色表现数据不进入开局规格摘要 | P1 | Component Integration | Determinism | Configuration；Client | 可提供两份不同的角色表现数据 | 用同一 `MatchRequest` 分别在两份表现数据下完成冻结 | 两份数据仅配色、徽章与音频键不同；规则数据完全相同 | 两次冻结得到的 `LockedMatchSpec` 摘要相同；以同一输入日志推进得到相同的状态校验和与赛果 | [Confirmed] [角色表现数据：数据模型](../../development/design/character-presentation.md#数据模型)；[规则配置与开局规格冻结](../../development/design/rule-configuration.md) |
| TC-009 | 多种表现配置下相同输入得到相同校验和与赛果 | P0 | System | Determinism | Client；Rules | 已装配客户端，可回放固定输入日志 | 在表现配置矩阵下分别回放同一输入日志至比赛结束 | `AnimationIntensity` 取 `Full` 与 `Reduced`；`vibration` 取开与关；音频设备可用与不可用；表现更新频率取正常与人为降低 | 全部组合的逐 tick 状态校验和序列相同；小局结果、分数、垃圾数量、Fever 状态与最终赛果相同；`match_tick` 总数相同 | [Confirmed] [表现运行时：动画强度](../../development/design/presentation-runtime.md#动画强度)；[PRD §8](../../PRD.md)；[表现与 UI 设计 §6.1](../../presentation.md) |
| TC-010 | 无美术、无音频设备、无手柄时仍可用键盘完成对局 | P0 | System | Smoke | Client；Match Flow | 已装配客户端；角色表现数据 `Failed`；音频输出不可用；未连接手柄 | 只用键盘从主菜单完成一局 BO3 | 表现数据 `Failed(Parse)`；无音频设备；无手柄 | 流程可走完并到达赛果；角色使用替补配色与徽章且两侧可区分；棋盘、NEXT、两条队列、Fever 面板与比分保持可读；各降级项各自保留诊断；规则结论与全部资源可用时的同输入结果一致 | [Confirmed] [PRD §5.3](../../PRD.md)；[角色表现数据：查询](../../development/design/character-presentation.md#查询)；[表现运行时：音频与震动](../../development/design/presentation-runtime.md#音频与震动) |
| TC-011 | 球体线索按色觉辅助分档，NEXT 预览携带手牌的形状与颜色 | P1 | Component | — | Client | 可查询球体线索与 NEXT 预览格的取值 | 参数化取各类占据者在设置开与关下的线索；再参数化取四种手牌形状每个偏移位的预览内容 | 普通球五种颜色 id；垃圾球；空格；`color assist` 取开与关；`I`、`L`、`J`、`ODual`、`OMono` 手牌 | 关闭时普通球无球内符号，开启时五种颜色各得一个互不相同的符号；垃圾球的标记不随设置变化；空格始终无符号；每种手牌的预览在组占据的偏移位给出该位对应的抽取颜色、在未占据的偏移位为空，`L` 与 `J` 的横臂分列两侧，单色手牌不出现第二种抽取颜色 | [Confirmed] [表现运行时：球体线索](../../development/design/presentation-runtime.md#球体线索)；[表现与 UI 设计 §4.1、§4.2、§7](../../presentation.md) |
| TC-012 | `ClearPreview` 与 `Gravity` 按阶段进度投影，`Reduced` 改为吸附 | P1 | Component | — | Client | 可按阶段进度查询被消球的姿态与下落球的位置 | 参数化取整段进度上的姿态与位置，两档动画强度各取一次 | 进度取 `0`、命中占比、`0.5`、`1` 与越界的 `2`；下落起止跨越多格；`AnimationIntensity` 取 `Full` 与 `Reduced` | `Full` 下：被消球在命中段保持原尺寸并完成闪光，其后缩放与不透明度单调下降，到阶段末已淡出；下落球起止落在整格、中途位于两格之间，且位置单调不回退，越界进度被钳制在终点。`Reduced` 下：被消球全程保持一次稳定高亮，下落球停在起点直到阶段结束再吸附到终点。两档均不改变 `duration_ticks` | [Confirmed] [表现运行时：画面重建](../../development/design/presentation-runtime.md#画面重建)、[动画强度](../../development/design/presentation-runtime.md#动画强度)；[表现与 UI 设计 §6.1](../../presentation.md) |
| TC-013 | 每个规则事实请求一条 cue，增益与震动按设置和设备门控 | P1 | Component Integration | — | Client | 可构造含全部事实种类的 `MatchStepReport`，并可运行真实对局 | 对同一份报告请求两次 cue；参数化两级音量四组、震动开关与手柄数各求一次；另在真实对局中推进到首个事实 | 十种事实各一条；音量 `1.0×1.0`、`0.5×0.5`、`0.0×1.0`、`1.0×0.0`；震动开/关；手柄数 2/1/0 | 事实数与 cue 数相等且种类逐项对应，`match_tick` 为事实所在 tick；重复提交同一份报告不再产生 cue；增益为两级音量之积，取 0 时仍产生请求；无音频设备时增益为 0 并只诊断一次；只有连锁、垃圾落下与进入 Fever 带震动，且需要设置开启与该槽位有手柄；真实对局中 cue 的 tick 不超过当前规则 tick | [Confirmed] [表现运行时：音频与震动](../../development/design/presentation-runtime.md#音频与震动)；[PRD §4.2](../../PRD.md)；[表现与 UI 设计 §6](../../presentation.md) |
| TC-014 | 页面与对局实体的存活期与它们所属的东西一致 | P1 | Component Integration | — | Client | 客户端 app 装有 Bevy UI 栈（无渲染后端），可走完页面主路径并建立真实对局实例 | 逐页进入主菜单→模式→角色再逐页退回；进入对局后暂停、恢复、进入赛果；另在对局中触发一次消除并继续推进 | 三层页面各一次往返；暂停与赛果各一次；消除后再推进 120 个 fixed tick | 退回到某页时该页的实体数与首次进入时相同，途经页面不残留；暂停时页面叠在 HUD 之上而不替换它；离开对局后对局实例与全部棋盘格实体一并释放；消除留下的痕迹在自身存活期结束后归零，无需其它系统清理 | [Confirmed] [表现运行时：画面重建](../../development/design/presentation-runtime.md#画面重建)；[页面导航与焦点](../../development/design/page-navigation.md)；[对局实例生命周期](match-lifecycle.md) |
| TC-015 | 圆框下的名字来自 roster 与当前语言，而不是掉落组标识 | P1 | Component Integration | — | Client；Configuration | 客户端 app 装有 Bevy UI 栈，roster 与本地化目录均可用，对局已建立 | 进入对局并等待运行数据落地，读取屏上全部文本 | 双方角色为 `psi-a`、`psi-b`；两者的 `display_name_key` 在当前语言目录中均有条目 | 屏上出现两个角色 `display_name_key` 在当前语言下的文本；不出现角色标识本身；键在目录中缺失时不以键名充当名字 | [Confirmed] [角色表现数据：边界](../../development/design/character-presentation.md#边界)；[本地化运行时](../../development/design/localization-runtime.md) |
| TC-016 | 角色表现目录晚于对局开始发布时圆框重解析 | P1 | Component Integration | — | Client | 客户端 app 装有 Bevy UI 栈，进入对局的一帧上角色表现目录尚未发布 | 进入对局并推进到目录发布，再推进一帧 | 仓库随包的 `characters.ron`；两名角色 | 圆框的边框色与徽章取自目录中该角色自己的取值，而非替补的槽位配色与显示名首字符；目录发布前的替补结果不被保留到本局结束 | [Confirmed] [角色表现数据：查询](../../development/design/character-presentation.md#查询) |

## 风险查漏

快照重建、跳帧、实体生命周期、画布缩放、设备与资源降级、表现预算与规则不变性均有直接用例；单帧可读性等视觉判断由 Linux 实机验收承担，不进入自动化断言。
