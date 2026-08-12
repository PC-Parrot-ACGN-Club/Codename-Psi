# 测试策略与分类标准

**适用范围：** 项目的测试设计、实现、审阅与回归

写作规则见[文档写作规范](../CONVENTIONS.md)。测试用例设计流程、文档模板与交付前检查见[测试用例设计约定](design/README.md)。

## 1. 目标

测试以可追溯的产品和工程证据为基础，以最低充分装配范围验证行为。每个测试用例使用 Test Level、Concern 与 Domain 三个独立维度记录范围、验证目的和游戏职责；测试代码按 Cargo crate 与测试层级组织。

## 2. Test Basis 与证据状态

预期结果的依据按以下权威顺序使用：

1. 用户当前任务与已确认决定。
2. [PRD](../PRD.md)、[玩法设计](../gameplay.md)、[技术设计](../TDD.md) 与 [开发设计文档](../development/README.md)。
3. 当前代码与配置。
4. 既有测试与 Git 历史。

每个用例在 Test Basis 中记录精确引用，并标记一种证据状态：

| 状态 | 用途 |
| --- | --- |
| Confirmed | 行为进入正式规格测试，并使用强断言。 |
| Inferred | 行为在用户确认后进入正式规格测试。 |
| Unknown | 现状保护需求使用 Characterization Test 记录可观察行为；设计确认后更新该测试。 |

## 3. 分类模型

| 维度 | 回答的问题 | 记录规则 |
| --- | --- | --- |
| Test Level | 验证该行为所需的最小装配范围是什么？ | 每个用例选择一个 Level。 |
| Concern | 断言承担哪项跨领域验证目的？ | 直接覆盖该目的时标记，可记录多个。 |
| Domain | 被测行为属于哪段游戏语言和职责？ | 每个用例记录一个主 Domain；跨域行为可补充次 Domain。 |

三个维度分别服务于装配成本控制、专项回归检索与产品语言对齐。

### 3.1 Test Level

按 Component → Component Integration → System 的顺序选择。前一层已完整证明行为时，在该层完成测试设计。

| Level | 定义 | 典型对象 |
| --- | --- | --- |
| Component | 单个公开类型、纯函数或配置解析器的输入、输出与状态转移。 | 棋盘、计分、随机序列、RON/JSON 解析。 |
| Component Integration | 同一领域内多个组件协作，或单个 crate 的轻量运行时流程。 | `MatchState` 与 `RuleProfile`、输入量化到 tick、AI 合法输入。 |
| System | 已装配客户端或跨 crate 主流程。 | Bevy 最小启动、本地 BO3、R2 LAN BO3。 |

Component 的测试在内存中构造所需状态。Component Integration 的测试通过接口协作验证领域流程。System 的测试聚焦完整运行栈才能证明的主路径与高风险闭环，并保持精简。

### 3.2 Concern 注册表

| Concern | 断言目的 | 主要依据 |
| --- | --- | --- |
| Smoke | 验证基本启动、装配、资源读取和关键运行时对象。 | [TDD §4、§7.2](../TDD.md) |
| Content Validation | 验证版本化规则、角色、Fever 题面和文本资源符合 schema 与已确认设计基线。 | [TDD §5](../TDD.md)、[玩法设计 §7](../gameplay.md) |
| Determinism | 验证相同初始状态、seed 与量化输入产生相同状态或校验和。 | [TDD §3、§7.1](../TDD.md)、[PRD §8](../PRD.md) |

新增 Concern 同时具备明确的产品或技术依据、可观察的断言目标、至少一个计划测试场景，以及专项检索价值。

### 3.3 Domain 注册表

| Domain | 覆盖职责 | 主要依据 |
| --- | --- | --- |
| Rules | 棋盘、落子、消除、连锁、计分、攻击、垃圾与 Fever 规则。 | [玩法设计 §2–§5](../gameplay.md) |
| Match Flow | 小局、BO3、胜负、暂停、重开与赛果流程。 | [玩法设计 §2、§6](../gameplay.md)、[PRD §5](../PRD.md) |
| Configuration | `RuleProfile`、角色掉落组、Fever 题面、RON/JSON schema 与文本资源。 | [TDD §5](../TDD.md)、[玩法设计 §7](../gameplay.md) |
| Client | Bevy 状态、渲染、UI、音频与窗口行为。 | [TDD §4](../TDD.md)、[表现与 UI 设计](../presentation.md) |
| Input | 键盘/手柄映射与 tick 动作量化。 | [TDD §3–§4](../TDD.md)、[PRD §5.2](../PRD.md) |
| AI | 通过合法动作选择落点的对手行为。 | [PRD §4.3](../PRD.md) |
| Network | LAN 握手、同步、回滚、断线与状态一致性。 | [TDD §6](../TDD.md)、[PRD §6.1](../PRD.md) |

## 4. 测试用例设计资料

[测试用例设计约定](design/README.md)是测试设计流程、文档模板、字段标准和交付前检查的唯一入口。

[测试用例设计方法参考](design/techniques.md)定义各方法的适用信号、输入构造、Psi 示例和组合规则。

## 5. 优先级

用例使用连续的 `TC-001` 编号和 P0–P3 优先级：P0 覆盖阻断主路径，P1 覆盖主要分支，P2 覆盖常见风险场景，P3 覆盖边缘组合。

## 6. 设计变更

设计变更同步更新关联用例的依据、预期结果与证据状态。
