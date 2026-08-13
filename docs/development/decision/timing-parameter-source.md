# DEC-002：活动组时序参数取自 Puyo Puyo Tsu 的逆向工程数据

**关联文档：** [盘面与活动组操控](../design/board-and-falling-group.md)、[玩法设计 §1、§7](../../gameplay.md)

## 背景

规则剖面的参考来源是 Fever 1/2 主机与 PC 版本。自然下落速度、软降速度、横移输入重复、锁定宽限与分裂自由落体时长直接决定操作手感，也进入确定性规则状态，因此必须有确定取值。

Puyo Nexus 对 `Puyo Puyo Tsu`（Mega Drive）做了完整的逐帧逆向工程，公开了上述全部数值并标注 fully reverse-engineered。Fever 1/2 没有等价页面，其余页面只描述规则语义，不含帧数。

## 决策

录入 Tsu 的逆向工程帧数作为初始时序参数，数值见[盘面与活动组操控 §时序参数](../design/board-and-falling-group.md#时序参数)。

配置中把时序来源与规则剖面来源分开记录：`reference_profile` 保持 `fever1_2_console_pc`，时序参数另记 `timing_source = "puyo_puyo_tsu_md"`。两个字段都进入规则摘要。

手感校准只修改配置数值，并同步更新 `timing_source`。

## 依据

- [玩法设计 §1](../../gameplay.md)：数值默认策略是优先在配置层校准并保留引用与验证样本，因此时序参数属于可校准的配置值而非固化常量。
- [玩法设计 §7](../../gameplay.md)：各常数需记录来源与录入日期，来源标注本身已是配置要求。
- [Puyo Puyo Tsu/Frame Data Tables](https://puyonexus.com/wiki/Puyo_Puyo_Tsu/Frame_Data_Tables)：输入重复 8/2 帧、自然下落 16 帧每格、软降 2 帧每格、锁定宽限 32 帧、上抬 8 次上限、分裂延迟与自由落体时长表。
- [Puyo Puyo Tsu/Pair Lateral Movement](https://puyonexus.com/wiki/Puyo_Puyo_Tsu/Pair_Lateral_Movement)：横移当帧生效且不可连续两帧发生。

## 备选方案

| 方案 | 适配条件 | 影响 |
| --- | --- | --- |
| 自行拍定数值 | 已有可试玩版本，能立即用手感判断 | 无任何来源依据，校准时无法判断偏差来自参数还是规则；与「数值默认录入原作」的策略冲突 |
| 留空待实测后录入 | 有条件实测 Fever 1/2 主机版本 | 在实测完成前无法运行完整落子循环，阻塞 S2 之后的全部子任务 |
| 采用 Tsu 逆向数据（已选） | 需要确定值，且接受跨作品来源 | 数值有逐帧依据；来源与剖面不一致的风险由 `timing_source` 显式记录，不会被误认为 Fever 原值 |
