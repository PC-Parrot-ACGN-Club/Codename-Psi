# 测试用例设计：运行数据与设置持久化

**关联设计：** [版本化运行数据加载](../../development/design/runtime-data-loading.md)、[本机用户设置](../../development/design/user-settings.md)

**关联实现：** `../../../crates/client`、`../../../assets/data`、`../../../assets/i18n`

## 需求理解摘要

**功能：** 验证文件与 Bevy Asset 边界上的 typed resolution、错误上下文、设置原子保存及消费者装配。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** 资源加载和设置持久化的 Component Integration 行为。
**Test Basis：**

- [Confirmed] [版本化运行数据加载](../../development/design/runtime-data-loading.md)：协作时序、错误上下文、失败分级和消费者接口。
- [Confirmed] [本机用户设置](../../development/design/user-settings.md)：平台配置路径与原子替换保存。

**设计基线：** 使用最小 Bevy Asset App、临时配置目录或项目根插件验证真实协作边界。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不重复内存解析器（见 [运行数据解析](../component/runtime-data-parsing.md)）和本地化查询（见 [本地化](../component/localization.md)），也不定义加载结果如何释放启动屏障（见 [应用生命周期](application-lifecycle.md)）。

## 测试点清单

- 有效与四类失败资源形成带上下文的 resolution（TC-001；Concern: Content Validation）。
- 平台配置目录中的原子保存、恢复与 replace 失败语义（TC-002）。
- 项目根插件装配后消费者取得 resolved typed data，降级级的角色表现目录缺失时不阻塞（TC-003；Concern: Smoke）。
- 设置修改在同一运行实例内于各消费者立即生效（TC-004）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 等价类划分 | 有效、missing、malformed、unsupported、invalid 资源 | TC-001 |
| 场景 / 协作路径 | 设置保存恢复、项目根插件数据消费与设置生效 | TC-002～TC-004 |
| 错误猜测 | 原子 replace 失败 | TC-002 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 资源加载成功与四类失败均形成带上下文的 resolution | P1 | Component Integration | Content Validation | Configuration；Client | 最小 Bevy Asset app 与临时 asset root | 参数化加载有效、缺失、malformed、unsupported、invalid 资源 | 有效及前三类失败使用 `assets/data/*.ron` 与 `assets/i18n/*.json` 等价 fixture；invalid 使用 schema 1、`locale=fr` 的 catalog | 有效资源为 `Loaded(typed_data)`；四类失败为 `Failed(error)` 且不产生替代值；invalid 的 typed cause 为 InvalidData；error 含 path、category、typed cause；两类结果均 resolved | [Confirmed] [版本化运行数据加载：协作时序](../../development/design/runtime-data-loading.md#协作时序)；[版本化运行数据加载：失败分级](../../development/design/runtime-data-loading.md#失败分级)；[本地化运行时：语义验证](../../development/design/localization-runtime.md#语义验证) |
| TC-002 | 平台配置目录中的原子保存成功可恢复，replace 失败保留正式文件与内存值 | P1 | Component Integration | — | Configuration；Client | 平台配置根目录中已有旧正式设置，并可通过实现选择的测试环境构造 replace 失败 | 解析设置路径并执行成功保存、重载；再更新内存值并构造 replace 失败 | 旧 language=`en`；新 language=`zh-CN` | 正式路径位于平台配置根目录；成功重载得到新值且无不完整文件；失败返回可观察错误，内存保持新值，正式文件仍为旧值 | [Confirmed] [本机用户设置：保存设置](../../development/design/user-settings.md#保存设置) |
| TC-003 | 项目根插件装配后消费者取得 resolved typed data | P1 | Component Integration | Smoke | Configuration；Client | 最小 Bevy App 注册项目根插件，asset root 指向仓库真实 `assets/` | 推进应用直到数据加载结束，从消费者侧读取 typed 结果 | 仓库内现有的 `assets/data/*.ron` fixture | 消费者可读到 resolved typed data，成功为 `Loaded`、失败为带诊断的 `Failed`；请求、轮询与注册均由项目根插件完成，测试不自建加载生命周期 | [Confirmed] [版本化运行数据加载：协作](../../development/design/runtime-data-loading.md#协作) |
| TC-004 | 设置修改后各消费者立即生效且不依赖重启 | P1 | Component Integration | — | Client；Configuration | 最小客户端 app 注册项目根插件，语言、窗口、音量、绑定与表现设置的消费者均可观测 | 在同一运行实例内逐项修改设置并在下一次更新后读取消费者状态 | language `en → zh-CN`；window mode 切换；master volume `1.0 → 0.3`；P1 `SoftDrop` 改绑；`AnimationIntensity` `Full → Reduced`；`vibration` 开→关 | 每项修改在不重启、不离开设置页的情况下于消费者侧生效：文本查询走新 locale、窗口模式改变、音频增益改变、运行时采样使用新绑定、表现层读到新的动画强度与震动开关；写盘失败时已生效的内存值不回退 | [Confirmed] [本机用户设置：协作](../../development/design/user-settings.md#协作) |

## 风险查漏

资源路径、typed cause、resolution 完成、原子替换、生产插件消费者和设置生效时机均有直接用例。

