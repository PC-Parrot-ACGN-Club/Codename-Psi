---
name: psi-doc
description: 在 codename-psi 仓库编写或修改 docs/development/、docs/test/ 下的设计文档与测试用例设计稿时使用。提供规范位置、模板位置和写作流程。
---

# Psi 文档写作

## 动手前必读

- 规则：[docs/CONVENTIONS.md](../../../docs/CONVENTIONS.md) —— 6 条，全部适用，先读完。
- 模板：[docs/development/template/](../../../docs/development/template/README.md)
- 测试稿格式：[docs/test/design/README.md](../../../docs/test/design/README.md)
- 测试分类：[docs/test/README.md](../../../docs/test/README.md)

## 六条规则（细则见 CONVENTIONS）

1. 只写当前结论
2. 一件事只写一次
3. 一个主题一份文档
4. 职责排除集中在「边界」节并写明归属（行为结果的否定留在行为节）
5. 可判定结果写在测试稿里
6. 篇幅是信号：文档过长时判断是写作重复还是设计本身职责过多

## 新建文档

跨模块运行面用 `system-overview.md`，其余一律用 `module-design.md`。
写每个术语前先 `grep` 一次 `docs/`：已经在别处定义过就链接锚点，不在本文重新展开。

## 修改文档

先确认这条结论归哪份文档——在 `docs/` 里搜术语，找到定义它的那份，改那一份。
其余文档只保留链接。不要在本文里"顺便说明一下"。

## 四个停下来的信号

- 想写"经审核确认…""原设计…""后续将…" → 这属于 commit message，不进文档。
- 想写"不要 X" → X 归谁？写进「边界」节并给出链接；给不出就不写。
- 想为同一主题新开一份文档 → 加一节「协作」到已有那份里。
- 写完发现某节在复述前面 → 删掉那一节，不是加一句"详见上文"。

## 提交

文档改动在主目录进行，不开 worktree（见 CLAUDE.md）。
提交前需用户确认；提交本身即表示这轮审核通过。
