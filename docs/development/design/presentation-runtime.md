# 表现运行时

**相关模块：** `client::presentation`、`game_core::view`
**关联文档：** [表现与 UI 设计](../../presentation.md)、[小局、BO3 与安全点](match-and-round.md)、[连锁结算](chain-resolution.md)、[Fever 循环](fever-mode.md)、[角色表现数据](character-presentation.md)、[本机用户设置](user-settings.md)、[固定频率规则调度](fixed-tick-simulation.md)

## 目标

建立规则事实到画面、音频与震动的单向路径：把最新规则状态投影为可在任意帧重建的表现快照，把每 tick 的规则事实转成一次性表现事件，并规定设置、设备与资源缺失下的降级。

## 数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| virtual canvas | 全部 UI 与对局元素使用的布局坐标系 | 固定 `1920 × 1080`，缩放规则见[表现与 UI 设计 §2.2](../../presentation.md) |
| `MatchPresentationSnapshot` | 重建当前画面所需的全部事实 | 字段见[表现与 UI 设计 §8](../../presentation.md)；每 tick 整体替换 |
| `PresentationEvent` | 一次已确认规则事实对应的可丢弃演出 | 以 `(match_tick, ordinal)` 唯一标识 |
| `ordinal` | 同一 `match_tick` 内的事件序号 | 从 0 起，按 `MatchStepReport` 的事件顺序分配 |
| presentation budget | 同屏短命实体与并发声音的上限 | 每类独立计数 |

## 行为

### 快照构造

- 输入：最新 `MatchView`、本 tick 的 `MatchStepReport`、`LockedMatchSpec`、[角色表现数据](character-presentation.md)与当前 [`AnimationIntensity`](user-settings.md#数据模型)。
- 处理：把规则事实与客户端表现数据合并为一份 `MatchPresentationSnapshot`；`momentum` 按[表现与 UI 设计 §3.1](../../presentation.md) 从待接收垃圾、溢出风险、本次结算净攻击和 Fever 状态推导。
- 输出：唯一的最新快照。
- 错误语义：不存在对局实例时不产生快照，HUD 不显示对局信息。

规则事实只从 `MatchView` 与 `MatchStepReport` 取得。表现层不比较 UI 的前后帧来反推规则事实，也不保存与规则平行的对局状态。

快照同时携带当前 `AnimationIntensity` 解出的演出参数，画面重建因此不必在绘制时另读设置，「只依赖快照」这一条才成立。演出参数是快照中唯一随该设置变化的部分，规则字段逐项不变。

### 表现事件发布

- 输入：本 tick `MatchStepReport` 的事件序列。
- 处理：按该序列顺序分配 `ordinal`，映射为 `PresentationEvent`。
- 输出：本 tick 的 `PresentationEvent` 序列。
- 错误语义：同一 `(match_tick, ordinal)` 只演出一次；事件丢失只影响该次演出，下一帧的快照仍然完整。

### 垃圾队列分级

- 输入：一条通道的精确队列数量。
- 处理：按[表现与 UI 设计 §4.3](../../presentation.md) 的单位从大到小贪心分解为图标序列，图标数超出面板槽位时截断尾部。
- 输出：图标序列与并排显示的精确数量。
- 错误语义：截断只丢最轻的图标，精确数量始终完整；队列为零时不出图标。

分解结果只用于绘制，不参与任何规则判断；两条通道用同一套单位与符号。

### 文字反馈

- 输入：本 tick `MatchStepReport` 中该玩家的 `ChainSettled` 与 `AttackArbitrated`。
- 处理：一个 tick 对一名玩家最多留下一行文字，优先级为全消 → 送出的攻击量 → 抵消量；没有可说的事实时保留上一行，不清屏。行的存活期以规则 tick 计，到期后自行消失。
- 输出：该玩家棋盘上方的一行文字，内容为事实名称加精确数量；全消只有名称。
- 错误语义：文字只读事实，不回写规则；对局暂停时 tick 不前进，因此当时显示的那一行留在屏幕上。

事实名称随[界面语言](user-settings.md#数据模型)本地化，数量是规则的精确整数。连锁数是结算期间一直成立的状态而不是一次性事实，因此由快照的 `chain_count` 直接绘制成 `CHAIN n`，不走本节。

### 画面重建

- 输入：最新快照。
- 处理：更新常驻实体的盘面、活动组、NEXT、双垃圾队列、Fever 面板、比分与角色标识；结算阶段的插值按[表现与 UI 设计 §6.1](../../presentation.md) 的阶段映射进行。
- 输出：与最新快照一致的画面。
- 错误语义：渲染落后于规则进度时直接采样最新阶段，不补播中间帧。

常驻信息只依赖快照。任何一帧丢弃全部 `PresentationEvent` 后，画面仍完整表达当前对局状态。

### 动画强度

| 表现 | `Full` | `Reduced` |
| --- | --- | --- |
| 粒子与闪光 | 全量 | 每次事实只保留一次提示 |
| 阶段插值 | 全程插值 | 阶段结束时吸附到终点 |
| 屏幕级光带与镜头位移 | 播放 | 用静态边框与固定构图替代 |

两档共同保留：全部文字、数字与图标，圆框角色的位移与姿态切换，以及规则时序——`duration_ticks`、规则事件 tick、下一组生成时机、Fever 时间与攻击到达时间在两档下相同。

「阶段结束时吸附到终点」对两个有时长的结算阶段分别成立：`Gravity` 的球停在起点直到本阶段结束再落到终点；`ClearPreview` 用一次持续的高亮代替缩放与淡出的过程，被消除的球仍然可辨认。

### 球体线索

- 输入：`color assist` 设置。
- 处理：关闭时普通球只绘制纯色填充；开启时按颜色 id 额外绘制互不相同的球内符号。垃圾球的标记与该设置无关，始终绘制。
- 输出：棋盘格与 NEXT 预览格使用同一套线索，两处始终一致。
- 错误语义：该设置只改变绘制，不改变任何规则时序，也不进入确定性验证状态。

球内符号的取值范围与无障碍职责见[表现与 UI 设计 §4.1、§7](../../presentation.md)。

### 音频与震动

- 输入：本 tick 的 `PresentationEvent` 序列、`master volume`、`sfx volume`、`vibration`。
- 处理：事件按类别映射到音频 cue 与震动模式；音频 cue 经过完整混音路径并应用两级音量。
- 输出：一次播放请求与一次震动请求。
- 错误语义：音频设备不可用时静默降级并保留一次诊断，不重复报告；没有连接手柄时不产生震动，也不产生诊断。

R1 不随包分发音频资源（见 [PRD §4.2](../../PRD.md)），因此播放请求没有对应采样数据。cue 的种类、触发 tick 与音量计算照常可观察。

震动只作用于本地手柄，按玩家槽位分别下发。

### 表现预算

- 输入：本 tick 待创建的短命实体与待播放的 cue。
- 处理：超出 presentation budget 时合并同类演出——同一 tick 的多次消除合并为一次粒子批、一个连锁数字与一次 cue。
- 输出：不超过上限的演出集合。
- 错误语义：仍然超限时丢弃最旧的可丢弃演出；快照驱动的常驻信息与规则 tick 不受影响。

## 虚拟画布

全部 UI 以 `1920 × 1080` 为设计尺寸、用像素长度书写。运行时缩放因子为 `min(窗口宽 / 1920, 窗口高 / 1080)`，剩余空间留边。

缩放沿两条路径生效，两者使用同一个因子：UI 层由 `UiScale` 缩放，盘面与其它 2D 图元由相机投影缩放。`UiScale` 只作用于固定像素长度，因此 UI 布局不使用百分比长度；两条路径的因子不一致时 UI 与盘面互相错位。

## 字体与数字

客户端随包分发 `assets/fonts/SourceHanSansCN-Bold.otf`，覆盖 `zh-CN` 与 `en` 的全部界面文本，不依赖目标系统已安装字体。许可与来源见 [assets/README.md](../../../assets/README.md)。

[表现与 UI 设计 §7](../../presentation.md) 要求的等宽数字由固定宽度的数字槽位逐位布局实现，不依赖字体的 OpenType tabular figures 特性。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `MatchView` | [小局、BO3 与安全点](match-and-round.md) | 本主题 | 每 tick 读取最新一份 |
| `MatchStepReport` | 同上 | 本主题 | 每 tick 恰好一份 |
| 结算阶段与进度 | [连锁结算](chain-resolution.md#表现协议) | 本主题 | 表现层读取阶段与进度，不返回完成信号 |
| 剩余时间与题面等级 | [Fever 循环](fever-mode.md) | 本主题 | 只读投影，秒数向下取整 |
| 角色配色、徽章与姿态 | [角色表现数据](character-presentation.md) | 本主题 | 缺失时使用内置替补 |
| `AnimationIntensity`、`vibration`、音量 | [本机用户设置](user-settings.md) | 本主题 | 立即生效 |

表现系统在普通 `Update` 中运行，读取由 `FixedGameSet::Rules` 产生的最新结果，其执行次数不改变规则语义。

## 边界

- 本文不定义布局位置、配色语言、常驻组件的视觉细节与结算阶段的视觉映射（见[表现与 UI 设计](../../presentation.md)）。
- 本文不定义规则阶段本身的时长与推进（见[连锁结算](chain-resolution.md)、[Fever 循环](fever-mode.md)）。
- 本文不定义角色表现数据的字段与加载（见[角色表现数据](character-presentation.md)）。
- 本文不定义页面焦点与页面实体生命周期（见[页面导航与焦点](page-navigation.md)）。
- 本文不定义快照编码与 checksum（见[确定性与快照](determinism-and-snapshot.md)）。渲染实体、音频、动画与震动不进入规则状态，也不参与 checksum。

## Test Basis

- [Issue #13](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/13)：要求双方棋盘、NEXT、垃圾、Fever、角色与比分在完整对局中持续可读，且动画、UI 与音频不改变固定 tick 结果。
- [表现与 UI 设计 §8](../../presentation.md)：定义 `MatchPresentationSnapshot` 与 `PresentationEvent` 的组成和职责划分。
- [表现与 UI 设计 §6.1](../../presentation.md)：结算阶段投影、跳帧采样，以及动画强度不得改变规则时序。
- [表现与 UI 设计 §9](../../presentation.md)：单帧截图可识别全部常驻信息。
- [PRD §5.3](../../PRD.md)：首版以图元绘制，无外部美术资源时仍可启动并完成可玩流程。
