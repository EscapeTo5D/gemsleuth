# gemsleuth · 宝石推理求解器

一个 Windows 桌面小工具，用于求解某些游戏中的"宝石推理"谜题（即 Mastermind/猜数字型谜题）：从若干种宝石中**有放回**地随机取出数颗排成答案，玩家根据每条猜测记录的两种反馈推断唯一答案：

- **蓝标**：位置和种类都正确的宝石数量；
- **金标**：种类正确但位置不正确的宝石数量（重复宝石按较少一侧计数，与标准 Mastermind 语义一致）。

## 功能

- **整卷求解**：把谜面全部记录一次性录入，直接算出答案（或列出候选 / 报告矛盾）；
- **陪玩助手**：每猜一次、游戏每反馈一次，实时缩小候选范围，并推荐下一最优猜测；
- **矛盾检测**：候选为 0 时明确提示，支持逐条禁用定位嫌疑记录；
- **多候选处理**：列出全部候选（超过 50 个折叠为计数），并给出消歧推荐猜测；
- **参数可配**：颜色数 4–8（默认 6）、槽位数 3–6（默认 4）、是否允许重复（默认允许）；
- 前瞻 minimax + 熵启发式双策略，rayon 多线程加速，录入即时响应；中文界面，绿色单文件。

## 下载

前往 [Releases](https://github.com/EscapeTo5D/gemsleuth/releases) 下载 `gemsleuth.exe` 直接运行。

> 首次运行 Windows SmartScreen 可能提示"未知发布者"，点"更多信息 → 仍要运行"即可（签名接入中，见 [docs/signpath.md](docs/signpath.md)）。

## 从源码构建

需要 Rust 1.85+（edition 2024）：

```sh
cargo build --release
```

产物：`target/release/gemsleuth.exe`。

运行测试：

```sh
cargo test
```

素材说明：`assets/` 下同名替换图片后重新编译即生效，见 `examples/gen_assets.rs`（占位素材生成器）。

## 素材版权说明

`assets/` 目录中的部分美术素材（背景图、宝石与反馈标图标）取自《饥荒》（*Don't Starve*，Klei Entertainment 出品）相关素材，版权归 **Klei Entertainment** 所有，仅随本免费工具作非商业的陪玩展示用途。本项目与 Klei Entertainment 无官方关联；如权利方有任何异议，请提出，会随时移除相关素材。

## 许可证

代码以 [MIT](LICENSE) 许可证开源；`assets/` 内第三方素材的版权归其原作者（见上节）。
