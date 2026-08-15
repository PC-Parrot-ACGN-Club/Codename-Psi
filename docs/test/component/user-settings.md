# 测试用例设计：本机用户设置

**关联设计：** [本机用户设置](../../development/design/user-settings.md)、[UI 交互动作](../../development/design/ui-action-input.md)

**关联实现：** `../../../crates/client`

## 需求理解摘要

**功能：** 验证用户设置的默认值、解析恢复、schema 演进、序列化、玩家绑定范围、绑定捕获与冲突检测。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** 内存内可构造和判定的 `UserSettings` 与默认输入绑定行为。
**Test Basis：**

- [Confirmed] [本机用户设置](../../development/design/user-settings.md)：默认值、数据模型、启动恢复、序列化与绑定冲突。
- [Confirmed] [UI 交互动作](../../development/design/ui-action-input.md)：固定绑定与可配置动作的范围。

**设计基线：** 以纯值、内存解析器和采样结果验证设置契约，不装配平台配置目录或完整 Bevy 应用。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义平台配置目录、原子替换流程与设置生效时机（见 [运行数据与设置持久化](../integration-system/runtime-data.md)）、设备采样时序（见 [客户端输入](client-input.md)），也不定义设置页的焦点与控件布局（见 [页面导航与焦点](page-navigation.md)）。

## 测试点清单

- 默认设置、缺失或无效输入的安全恢复（TC-001～TC-003）。
- 设置往返、玩家绑定隔离与冲突检测（TC-004～TC-006）。
- 固定绑定不进入持久化和冲突检测范围（TC-007）。
- 内置默认绑定可产生全部六种规则动作（TC-008；Concern: Content Validation）。
- schema 演进按加字段、删字段与不兼容改动分别处置（TC-009）。
- 绑定捕获的写入、取消与拒绝语义（TC-010）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 等价类划分 | 有效、缺失、malformed、unsupported 设置；可配置与固定绑定；三类 schema 改动 | TC-002～TC-003、TC-007、TC-009 |
| 场景法 | 设置序列化恢复与双玩家独立映射 | TC-004～TC-005 |
| 判定表 | 同玩家与跨玩家的绑定冲突 | TC-006 |
| 内容验证 | 默认键盘与手柄绑定覆盖 | TC-008 |
| 状态迁移 | 捕获态的写入、取消与拒绝分支 | TC-010 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 默认构造产生完整安全设置 | P1 | Component | — | Configuration；Client | 无设置输入 | 构造 `UserSettings::default()` 或等价默认值 | language=`en`；window=`Windowed`；master/sfx=`1.0`；vibration=`true`；animation intensity=`Full`；P1/P2 默认绑定范围 | 文档已定义默认值的字段完整且取值正确；P1/P2 的 `PlayerInputBindings` 均覆盖 `SoftDrop`/`HardDrop`/`RotateClockwise`/`RotateCounterClockwise` 四项可配置 `GameAction`，不含 `UIAction` 或 `GameAction::Left`/`Right`；四项默认绑定均非空，取值按[默认输入绑定](../../development/design/user-settings.md#默认输入绑定)定义 | [Confirmed] [本机用户设置：默认值](../../development/design/user-settings.md#默认值)；[UI 交互动作：物理绑定关系](../../development/design/ui-action-input.md#物理绑定关系) |
| TC-002 | 缺失、malformed 与 unsupported 设置均恢复完整默认值并区分诊断 | P1 | Component | — | Configuration；Client | 可调用设置解析/恢复入口并观察结果与诊断 | 分别提交三类输入 | 文件不存在；`(`；`schema_version=255` | 三组结果均为完整默认设置；缺失文件走缺省结果；malformed 与 unsupported 分别留下可区分的解析类、版本不支持类诊断；诊断载体与具体类型由实现决定 | [Confirmed] [本机用户设置：启动加载](../../development/design/user-settings.md#启动加载) |
| TC-003 | 设置解析成功恢复全部持久化字段 | P1 | Component | — | Configuration；Client | 支持的 settings schema | 解析完整 RON | 非默认语言、窗口、音量、两名玩家各自四项可配置 `GameAction` 的键盘与手柄绑定、震动 | 结果逐字段等于输入；只解析四项可配置 `GameAction` 绑定，不含 `UIAction` 或 `GameAction::Left`/`Right` 字段；P1/P2 数据未互换或合并 | [Confirmed] [本机用户设置：数据模型](../../development/design/user-settings.md#数据模型) |
| TC-004 | 设置序列化后重新加载保持值相等 | P1 | Component | — | Configuration；Client | 一份包含全部字段的非默认设置 | 序列化到内存，再解析序列化结果 | language=`zh-CN`；window 非默认；音量边界内非默认值；P1/P2 各四项可配置 `GameAction` 的互异绑定 | 恢复值与原值逐字段相等；四项可配置绑定往返一致；结果不含 `UIAction` 或 `GameAction::Left`/`Right` 字段；schema 版本存在 | [Confirmed] [本机用户设置：保存设置](../../development/design/user-settings.md#保存设置) |
| TC-005 | P1/P2 四项可配置 `GameAction` 的键盘与手柄绑定独立保存恢复 | P1 | Component | — | Input；Configuration | P1/P2 的 `SoftDrop`/`HardDrop`/`RotateClockwise`/`RotateCounterClockwise` 使用可区分映射 | 保存并恢复设置 | P1 SoftDrop=`KeyS`、HardDrop=`KeyW`；P2 SoftDrop=`ArrowDown`、HardDrop=`ArrowUp`；两名玩家各有对应手柄映射 | 两名玩家的四项可配置绑定分别保持原值；修改任一玩家的绑定不会覆盖另一玩家；结果不含 `UIAction` 或 `GameAction::Left`/`Right` 绑定字段 | [Confirmed] [本机用户设置：数据模型](../../development/design/user-settings.md#数据模型) |
| TC-006 | 绑定冲突检测限定在四项可配置 `GameAction` 范围内、按玩家划分 | P1 | Component | — | Input；Configuration | 已存在一个可配置绑定 | 参数化查询新增绑定冲突 | 同一玩家 `KeyA→SoftDrop` 后添加 `KeyA→HardDrop`；同一玩家 `KeyA→SoftDrop` 后添加 `KeyA→RotateClockwise`；另一玩家使用 `KeyA→SoftDrop` | 每组同玩家案例均返回具名冲突；另一玩家案例按独立配置范围处理，不与前者冲突；冲突组设置数据在 UI 决定前不被覆盖 | [Confirmed] [本机用户设置：输入绑定冲突](../../development/design/user-settings.md#输入绑定冲突) |
| TC-007 | 固定绑定动作不出现在 `PlayerInputBindings` 且不参与绑定冲突检测 | P1 | Component | — | Configuration；Input | 默认或已保存的玩家输入设置可查询和序列化 | 检查持久化结果与可配置绑定冲突行为 | `UIAction` 全部六项；`GameAction::Left`、`GameAction::Right`；四项可配置 `GameAction` | 持久化设置只包含四项可配置 `GameAction`；绑定冲突行为只处理这四项；固定绑定动作保持在可配置与冲突检测范围外 | [Confirmed] [UI 交互动作：物理绑定关系](../../development/design/ui-action-input.md#物理绑定关系)；[本机用户设置：数据模型](../../development/design/user-settings.md#数据模型)、[输入绑定冲突](../../development/design/user-settings.md#输入绑定冲突) |
| TC-008 | 默认绑定非空且覆盖六个规则动作 | P1 | Component | Content Validation | Configuration；Input | 使用内置默认设置，无设置文件 | 按默认绑定分别注入键盘与手柄物理输入并采样 | 默认 `UserSettings`；P1/P2 键盘与手柄默认键位 | 键盘与手柄均可产生全部六个 `GameAction`；四项可配置动作的默认绑定均非空；手柄的两个旋转位为 East 与 South，使手柄的返回与确认落在该平台惯例的按键上 | [Confirmed] [本机用户设置：默认输入绑定](../../development/design/user-settings.md#默认输入绑定)；[UI 交互动作：绑定来源表](../../development/design/ui-action-input.md#绑定来源表) |
| TC-009 | schema 演进按三类改动分别处置 | P1 | Component | — | Configuration；Client | 解析器支持的 schema 版本已知 | 参数化解析三类设置文件 | 当前版本但缺少 animation intensity 字段；当前版本且含一个未知字段；`schema_version` 高于支持范围 | 缺字段组只有该字段取默认值 `Full`，其余用户选择逐字段保留；未知字段组忽略该字段且其余字段保留；版本不支持组整体回到内置默认设置并保留诊断 | [Confirmed] [本机用户设置：schema 演进](../../development/design/user-settings.md#schema-演进) |
| TC-010 | 绑定捕获的写入、取消与拒绝语义 | P1 | Component | — | Input；Configuration | 可进入捕获态并注入一次物理输入 | 参数化执行三种捕获过程 | 捕获到未被占用的物理位；捕获期间施加返回输入；捕获到同一玩家同设备已占用的物理位，并重复提交同一物理位一次 | 第一组写入新绑定；第二组取消捕获并保留原绑定；第三组产生具名冲突结果且绑定表逐字段不变，重复提交得到同一结果而非最终写入，占用方保留该物理位；捕获态期间不产生 `UIAction` 也不产生规则动作 | [Confirmed] [本机用户设置：绑定捕获](../../development/design/user-settings.md#绑定捕获)、[输入绑定冲突](../../development/design/user-settings.md#输入绑定冲突) |

## 风险查漏

默认值、版本恢复、schema 演进、往返一致性、玩家隔离、绑定范围、捕获与冲突行为均有直接用例；平台文件替换与设置生效时机由集成测试稿覆盖。

