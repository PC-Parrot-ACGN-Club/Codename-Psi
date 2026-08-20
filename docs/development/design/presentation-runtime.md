# 表现运行时

**相关模块：** `client::presentation`、`game_core::view`
**关联文档：** [表现与 UI 设计](../../presentation.md)、[小局、BO3 与安全点](match-and-round.md)、[连锁结算](chain-resolution.md)、[Fever 循环](fever-mode.md)、[角色表现数据](character-presentation.md)、[圆框运动模型](portrait-motion.md)、[本机用户设置](user-settings.md)、[固定频率规则调度](fixed-tick-simulation.md)

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

### 圆框角色演出

- 输入：该玩家本次结算是否触发 Chain、己方垃圾窗口是否非空、本 tick 是否发生 `NuisanceDropped`、己方与对方的 `overflow_risk`，以及该角色的[表现数据](character-presentation.md)。
- 处理：按固定优先级取一个姿态——`offset` → `spell` → `damage` → `advantage` → `idle`。触发 Chain 结算时取 `offset`（己方垃圾窗口非空）或 `spell`（为空），显示窗口 90 tick；`damage` 由本 tick `NuisanceDropped` 触发（同为 90 tick 窗口）或己方 `overflow_risk` 为真（持续期间常驻）二者取或；`advantage` 为对方 `overflow_risk` 真且己方假；均不成立时取 `idle`。
- 输出：两侧圆框的配色与当前姿态立绘。
- 错误语义：角色表现目录或对应立绘句柄不可用时用[替补](character-presentation.md#立绘解析)绘制，只失去角色间的区分度；缺失诊断按角色与 slot 各记一次。

小局与比赛结束后由[对局结束立绘](#对局结束立绘)接管圆框，不再走本优先级链。

姿态与运动在两档[动画强度](#动画强度)下都保留，低强度只缩减幅度。演出不是攻防信息的唯一来源——连锁数画在棋盘内，攻防的量由两条垃圾队列的数字给出，两者互不替代。姿态到运动词汇的映射、参数与补间求值见[圆框运动模型](portrait-motion.md)。

### 连锁与 Fever cut-in

- 输入：该玩家本 tick 是否为 Fever 状态的进入 tick；[连锁结算](chain-resolution.md)对该玩家本次结算前瞻得到的「是否为末连」与最终连锁数。
- 处理：进入 Fever 的 tick 播放该角色的 `cutin-fever`；末连的 `ClearCommit` 且总连锁数 ≥ 2 时，按 2~3 / 4~5 / 6~7 / ≥8 取对应 cut-in，与该连的[提示音](#音频与震动)同刻触发，整条连锁只播一次。
- 输出：压在棋盘之上的一次性 cut-in 演出。
- 错误语义：对应立绘句柄不可用时不播放该次 cut-in，不影响连锁结算与音效正常进行。

### 对局结束立绘

- 输入：[小局、BO3 与安全点](match-and-round.md)的 `RoundOutcome`。
- 处理：己方获胜时在棋盘位置显示 `win` 立绘，落败显示 `lose` 立绘；和局不指定胜负方，双方圆框仍按[圆框角色演出](#圆框角色演出)的优先级链取姿态。
- 输出：对局结束画面中棋盘位置的立绘。
- 错误语义：对应立绘句柄不可用时用[替补](character-presentation.md#立绘解析)绘制，不影响胜负判定与结算流程。

### 题面预设标记

- 输入：该玩家当前 Fever 题面的标识与冻结的题面数据。
- 处理：把题面自带的格子标为起始标记，绘制时降低不透明度。
- 输出：棋盘上可与玩家自己堆出的球区分的预设球。
- 错误语义：没有开启的 Fever 场次、或题面标识在冻结数据中查不到时不标记任何格子，不影响其余绘制。

标记是题面自己的格子，因此预设球被消掉后该格自然不再有标记；被消除动画覆盖的格子按[动画强度](#动画强度)绘制，标记不与之叠加。

### 题面重现

- 输入：本 tick `MatchStepReport` 中该玩家的 `FeverPuzzleAdvanced`，以及重建快照给出的新题面预设格。
- 处理：客户端按自身时钟播放一次分行落入——从题面最靠下的一行起，每隔若干 tick 唤起上一行；被唤起的格子从上方短距离落入终点，尚未被唤起的格子按空绘制，不叠加[题面预设标记](#题面预设标记)。`Reduced` 强度保留分行节奏、省略落入插值，被唤起的格子直接落地。
- 输出：一次覆盖新题面全部预设格、自底向上推进的一次性演出。
- 错误语义：重现进行期间再次收到 `FeverPuzzleAdvanced` 时，新一轮直接替换旧的；快照在事实发生的同一 tick 已是终态，本演出不影响规则时序或棋盘读值。

### 垃圾队列分级

- 输入：一条通道的精确队列数量。
- 处理：按[表现与 UI 设计 §4.3](../../presentation.md) 的单位从大到小贪心分解为图标序列，图标数超出面板槽位时截断尾部。
- 输出：图标序列与并排显示的精确数量。
- 错误语义：截断只丢最轻的图标，精确数量始终完整；队列为零时不出图标。

分解结果只用于绘制，不参与任何规则判断；两条通道用同一套单位与符号。

### 文字反馈

- 输入：本 tick `MatchStepReport` 中该玩家的 `ChainSettled` 与 `AttackArbitrated`。
- 处理：一个 tick 对一名玩家最多留下一行，优先级为全消 → 送出的攻击量 → 抵消量；没有可说的事实时保留上一行，不清屏。行的存活期以规则 tick 计，到期后自行消失。
- 输出：该玩家的当前行。攻击量与抵消量已经是两条垃圾队列的数字变化，不写到屏上；只有全消这类没有别处可去的事实绘制在棋盘内。[圆框角色演出](#圆框角色演出)的姿态不读本节的行，直接从 `ChainSettled`、`NuisanceDropped` 与 `overflow_risk` 取值。
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

「粒子与闪光」的分档按每颗被消除的球花费粒子密度：`Full` 每颗球留下若干痕迹，`Reduced` 每颗为零，此时整个连锁步仍保留一处痕迹——那正是「每次事实只保留一次提示」。同屏痕迹数受 presentation budget 约束，超出后本 tick 不再新建。

「屏幕级光带与镜头位移」的分档各自成立：进入 Fever 的光带在 `Full` 下扫过并淡出，在 `Reduced` 下改为同样时长的静态边框；攻击到达的画面位移在 `Reduced` 下不发生，构图保持固定。两者的时长都以规则 tick 计，因此不改变任何规则时序。

痕迹的散开方向由格子坐标与自身序号推出，不取任何随机流：规则的随机流只属于规则。

### 球体线索

- 输入：`color assist` 设置。
- 处理：关闭时普通球只绘制纯色填充；开启时按颜色 id 额外绘制互不相同的球内符号。垃圾球的标记与该设置无关，始终绘制。
- 输出：棋盘格与 NEXT 预览格使用同一套线索，两处始终一致。
- 错误语义：该设置只改变绘制，不改变任何规则时序，也不进入确定性验证状态。

球内符号的取值范围与无障碍职责见[表现与 UI 设计 §4.1、§7](../../presentation.md)。

### 音频与震动

- 输入：本 tick 的 `PresentationEvent` 序列、`master volume`、`sfx volume`、`vibration`。
- 处理：每个事实映射到一种 cue；增益为两级音量之积，设备不可用时为 0。震动只对连锁、垃圾落下与进入 Fever 三类事实产生模式，并需要 `vibration` 开启且该槽位有手柄。
- 输出：每个事实一条播放请求，其中一部分带震动模式。
- 错误语义：音频设备不可用时静默降级并保留一次诊断，不重复报告；没有连接手柄时不产生震动，也不产生诊断。

同一 `(match_tick, ordinal)` 只请求一次，因此一帧看到两次同一份报告不会请求两次。增益为 0 是静音而不是取消：请求照常发生，触发了什么、在哪个 tick 触发仍然可读。

R1 不随包分发音频资源（见 [PRD §4.2](../../PRD.md)），因此播放请求没有对应采样数据。cue 的种类、触发 tick 与音量计算照常可观察。

震动只作用于本地手柄，按玩家槽位分别下发；没有归属玩家的事实（小局与比赛结束）不经这条路径下发震动。

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
| `FeverPuzzleAdvanced` 事实 | [Fever 循环](fever-mode.md#题面循环) | 本主题 | 每次触发一轮[题面重现](#题面重现)；新题面的预设格从快照读取，事实本身不携带 |
| 角色配色、替补徽章与姿态 / cut-in / 结局立绘句柄 | [角色表现数据](character-presentation.md) | 本主题 | 缺失时使用内置替补 |
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
