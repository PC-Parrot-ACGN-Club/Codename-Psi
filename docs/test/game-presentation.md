# 游戏表现测试索引

**关联需求：** [Issue #13](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/13)

**关联设计：** [表现与 UI 设计](../presentation.md)、[模块设计](../development/design/)

本索引组织游戏表现与页面交互的测试设计。焦点、快照投影和表现数据解析等在内存中即可判定的行为按被测能力收录在 `component/`，需要装配客户端或完整运行栈的行为收录在 `integration-system/`；具体断言只在对应测试稿中定义。

## Component 测试

| 测试稿 | 覆盖主题 |
| --- | --- |
| [页面导航与焦点](component/page-navigation.md) | 焦点环移动、确认与返回、页面动作映射、禁用项与双玩家输入归属。 |
| [表现快照与表现事件](component/presentation-snapshot.md) | 快照完整性、事件编号与去重、可重建性与动画强度不变性。 |
| [角色表现数据](component/character-presentation.md) | 目录解析、语义校验、错误分类与替补呈现。 |

## Component Integration 与 System 测试

| 测试稿 | 覆盖主题 |
| --- | --- |
| [对局实例生命周期](integration-system/match-lifecycle.md) | client 侧对局实例按 cause 新建、保留、重建与释放，以及本地 BO3 端到端。 |
| [表现运行时](integration-system/presentation-runtime.md) | 画面重建、页面实体生命周期、虚拟画布、降级与表现不变式。 |

## 与其它测试索引的关系

顶层状态边、迁移提交、仲裁与启动屏障属于[游戏基础设施测试](game-infrastructure.md)；`MatchState` 内部的小局、BO3 与确定性属于[规则引擎测试](rules-engine.md)。本索引下的测试稿消费这两者已经确立的行为，不重复验证。

设置的默认值、schema 演进与绑定捕获收录在[本机用户设置](component/user-settings.md)，设置的生效时机收录在[运行数据与设置持久化](integration-system/runtime-data.md)。

## 不进入自动化的验收

以下 Issue #13 验收项依赖人工观察，由 Linux 实机验收承担，不写成自动化断言：

- 单帧截图下双方棋盘、比分、NEXT、垃圾、Fever 与胜负状态的可读性。
- Fever、连锁、垃圾、全消、危险与胜负反馈的可辨认程度。
- 两把真实手柄的实机操作与震动效果。

自动化侧以这些验收项的可判定替代物覆盖：常驻信息由快照完整性（`component/presentation-snapshot::TC-001`）与画面重建（`integration-system/presentation-runtime::TC-001`）保证存在且可重建，非颜色线索由内容检查在实机验收中确认。
