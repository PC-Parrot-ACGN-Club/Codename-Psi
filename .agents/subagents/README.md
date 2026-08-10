# 项目子代理预设

本目录提供按需调用的领域 Prompt。每个角色独立使用，用户决定调用时机、次数和顺序。

| 角色 | 定义 | 首次任务的重点 |
| --- | --- | --- |
| Research | [research.md](research.md) | 调查当前事实、调用链、约束和未知项。 |
| Design | [design.md](design.md) | 设计满足当前目标的最小变更。 |
| Implementation | [implementation.md](implementation.md) | 完成生产代码或测试代码实现切片。 |
| Test Design | [test-design.md](test-design.md) | 将行为要求设计为可实现的测试用例。 |
| Validation | [validation.md](validation.md) | 独立检查改动与验收条件。 |

## 使用

Harness 会在每次启动时加载适用的 `AGENTS.md`。调用提示只需指定一个角色卡，并要求代理在行动前读取：

1. [_shared.md](_shared.md)；
2. 选定角色卡；
3. 角色卡列出的、与当前任务直接相关的指导文档和目标内容。

提供当前任务即可。下列字段按任务需要补充：

```text
TASK: 当前目标
SCOPE: 初始范围
TARGETS: 文件、符号、URL 或资源
INPUT_DOCS: 关联设计、调研或测试文档
ACCEPTANCE_CRITERIA: 可观察验收条件
CONSTRAINTS: 额外限制
OUTPUT_FILE: 临时报告路径（可选）
IMPLEMENTATION_KIND: production | test（仅 Implementation）
```

临时报告默认位于 `tmp/subagents/<task-id>/`。正式开发设计进入 `docs/development/`；正式测试设计进入 `docs/test/design/`。

角色在首次任务中遵守各自的严格职责边界。文档索引用于选择资料，任务无关文档保持关闭。完成一轮后，同一会话可依据 [_shared.md](_shared.md) 的范围扩展规则处理直接相关的相邻问题。
