# 通过 SignPath Foundation 获取免费代码签名

本工具发布后需要消除 Windows SmartScreen 的"未知发布者"警告。由于本项目已开源（MIT），走 **SignPath Foundation** 的开源项目免费签名计划，零费用。

## 现状与前提

| 条件 | 状态 |
|---|---|
| 开源许可证（OSI 认可） | ✅ MIT |
| 公开可访问的仓库 | ✅ GitHub `EscapeTo5D/gemsleuth` |
| 活跃维护 | ✅ |
| 开源 CI 构建 | ✅ GitHub Actions（`.github/workflows/`） |

证书由 SignPath Foundation（携手 Certum）签发，私钥托管在 SignPath 云端 HSM，**不可导出**——所有签名必须通过 SignPath 平台完成。证书一年一续，续期免费。

## 申请步骤（需要仓库所有者本人操作）

1. 打开 <https://signpath.org/>，找到开源项目（Open Source）证书计划的申请入口，填写申请表：项目名、GitHub 仓库地址、许可证、维护者信息、CI 信息等。
2. SignPath Foundation 人工审核后，会约一次**简短视频通话**验证申请者身份（代替传统公证，不限国籍）。
3. 审核通过后会收到 SignPath.io 的组织/项目邀请，登录后配置项目与签名策略。

审核周期通常为几天到数周不等，视申请队列而定。

## 批准后的技术接入（可交给 AI 完成）

批准后拿到 SignPath 组织 ID、项目 ID 与签名策略 ID，然后：

1. 在 GitHub 仓库安装 SignPath GitHub App（或创建 API Token 存入 Actions Secret）；
2. 修改 `.github/workflows/release.yml`：在 `cargo build --release` 之后、上传 Release 之前，插入 SignPath 签名步骤（官方 GitHub Action 或 `SignPath CryptFit` CLI，走 Trusted Build System 证明构建来源）；
3. 此后打 tag 推送即自动产出**已签名**的 `gemsleuth.exe` 并发布到 Release。

> 注意：签名后不要再改动 exe（如重新压缩打包），否则签名失效；分发物以 Release 附件为准。

## 参考

- SignPath Foundation：<https://signpath.org/>
- SignPath 产品文档：<https://about.signpath.io/>
- 开源计划说明与申请条件以官网为准
