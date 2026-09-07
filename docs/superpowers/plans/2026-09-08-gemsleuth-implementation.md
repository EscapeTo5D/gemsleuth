# Gemsleuth 宝石推理求解器 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建单文件 exe 的 Rust 桌面伴侣程序:整卷求解 + 陪玩助手两种模式求解"宝石推理"(标准 Mastermind 多重集计数语义)谜题。

**Architecture:** 单 crate 双 target——`lib` 放纯求解核心(model/judge/solver/strategy,零 UI 依赖,全部 TDD 覆盖),`bin` 是 egui 薄壳(素材编译期内嵌,脏标记触发重算)。核心层宝石只是 `0..colors` 索引,颜色语义只在 GUI 调色板层。

**Tech Stack:** Rust edition 2024 / rustc 1.98.1;`eframe 0.36.1`(含 egui)、`image 0.25.10`(仅 png feature,用于解码内嵌素材)。无其他第三方依赖。

**Spec:** `docs/superpowers/specs/2026-09-08-gemsleuth-design.md`(计划以规格为准,执行者应同时阅读规格)

## Global Constraints

- 颜色数 `4..=8`(默认 6)、槽位数 `3..=6`(默认 4)、允许重复(默认 true)——默认值对齐当前游戏
- 记录校验:`exact + partial ≤ slots`;`repeats=false` 时必须 `slots ≤ colors`
- rustc 1.98.1(本会话已由 1.94.1 升级)、edition 2024
- 依赖锁定:`eframe = "0.36.1"`、`image = { version = "0.25.10", default-features = false, features = ["png"] }`;**不引入其他第三方依赖**
- 全中文界面;窗口约 1000×700 可缩放
- 素材接口:`assets/gems/gem_0.png … gem_7.png`、`assets/icons/exact.png / partial.png / unknown.png`;`include_bytes!` 编译期嵌入,同名替换文件重编译即生效,exe 单文件
- 非目标:无会话持久化、无联网、无 OCR/抓屏、无 GUI 自动化测试(核心逻辑全部压在 lib 层测试)
- crate 内模块名为 `core`,会遮蔽裸 `use core::...`;**crate 内一律写 `crate::core::...`**(derive 宏内部用的 `::core` 绝对路径不受影响)
- GUI 只调用已在本机 eframe 0.36.1 上实测编译通过的 API(见下方"已验证 GUI API 清单"),不要凭旧版 egui 记忆写代码

### 已验证 GUI API 清单(eframe 0.36.1 + rustc 1.98.1 实测编译通过)

- trait 方法:**`fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)`**(必需,替代旧 `update`);**`fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame)`**(可选,每帧 UI 前调用,禁止画 UI——用于脏标记重算)
- `eframe::run_native(title, NativeOptions, Box::new(|cc| ...))`;`NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 700.0]), ..Default::default() }`
- `egui::CentralPanel::default().show(ui, |ui| ...)`(可直接吃 `&mut Ui`)
- `egui::Panel::top(egui::Id::new("id")).show(ui, |ui| ...)`(**`TopBottomPanel` 已不存在**)
- 图片:`ui.image(egui::load::SizedTexture::new(tex.id(), [32.0, 32.0]))`;图片按钮:`ui.add(egui::Button::image(sized)).on_hover_text("红")`(**`ImageButton` 已删除,`Button` 没有 `tooltip_text`**)
- 纹理:`ctx.load_texture(name, egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba), egui::TextureOptions::LINEAR)`;解码:`image::load_from_memory(bytes)` → `.to_rgba8()` → `.dimensions()`
- 字体:`egui::FontDefinitions::default()` → `fonts.font_data.insert("cjk".into(), egui::FontData::from_owned(bytes).into())` → `fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "cjk".into())`(`FontFamily` 只有 Proportional/Monospace 两个变体)
- 控件:`egui::ComboBox::from_label(..).selected_text(..).show_ui(ui, ..)`、`ui.add(egui::DragValue::new(&mut v).range(0..=4))`、`ui.selectable_value(&mut tab, val, "文本")`、`ui.checkbox(&mut b, "启用")`、`ui.colored_label(egui::Color32::RED, "..")`、`ui.small_button(..)`、`ui.separator()`、`ui.add_space(4.0)`、`egui::ScrollArea::vertical().max_height(240.0).show(ui, ..)`
- 弹窗:`egui::Window::new("确认修改设置").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| ..)`
- 借用陷阱:在 `|ui|` 闭包里同时要 ctx 时,先 `let ctx = ui.ctx().clone();` 再用

### 与设计文档的偏差(均已论证)

| 偏差 | 理由 |
|---|---|
| rustc 1.94.1 → **1.98.1**,eframe 用 **0.36.1** 而非 0.35 | 本会话用户要求升级工具链;0.36.1 为最新版且全套 API 已实测编译通过 |
| **不引入 egui_extras** | 其 image feature 只是把 image crate 传递进来;我们直接 `image::load_from_memory` 解码内嵌 PNG,直接声明 `image` 依赖更干净,依赖数不增 |
| 规格 §6 judge 示例 `guess=黄黄紫蓝 vs candidate=紫紫紫黄 → (0,2)` 为**笔误** | 按 §4.2 公式(已对真实游戏双重验证)复核为 **(1,1)**:exact=1(第 3 位紫对),overlap=2(黄1+紫1),partial=1。本计划测试以公式为准 |
| 金标准测试 6 条记录中 **4 条为构造补充**(第 1、2 条来自规格 GUI 草图的真实谜面) | 仓库中只留有真实谜面 2 条记录(橙蓝紫红→(1,0)、橙蓝紫绿→(1,0))与唯一解「紫紫紫黄」;构造的 4 条已用穷举脚本验证同样唯一解出 紫紫紫黄。**用户提供其余真实记录后可直接替换,断言不变**(见附录 A) |
| 占位素材为**程序化生成的纯色圆形 PNG**(非"从截图裁切") | 仓库无截图文件;生成器(example)可重复执行,同名替换接口不变 |

---

## 任务总览与文件结构

| 任务 | 内容 | 产出文件 |
|---|---|---|
| 1 | lib 脚手架 | `Cargo.toml`、`src/lib.rs`、`.gitignore` |
| 2 | 数据模型与校验 | `src/core/mod.rs`、`src/core/model.rs` |
| 3 | 反馈判定 judge | `src/core/judge.rs` |
| 4 | 全空间枚举 | `src/core/solver.rs` |
| 5 | 候选过滤 | `src/core/solver.rs` |
| 6 | 熵推荐 + 采样 | `src/core/strategy.rs` |
| 7 | 精确前瞻 + 分层调度 | `src/core/strategy.rs` |
| 8 | solve 编排 + 金标准 | `src/core/solver.rs`、`src/lib.rs` |
| 9 | 占位素材生成器 | `examples/gen_assets.rs`、`assets/**` |
| 10 | GUI 骨架 | `src/main.rs`、`src/gui/mod.rs`、`Cargo.toml` |
| 11 | 素材装载 + 调色板 | `src/gui/assets.rs`、`src/gui/palette.rs` |
| 12 | 记录面板 | `src/gui/records_panel.rs` |
| 13 | 整卷求解面板 | `src/gui/solve_panel.rs` |
| 14 | 陪玩助手面板 | `src/gui/assistant_panel.rs` |
| 15 | 终验与交付 | (无新文件) |

核心层任务(1-8)严格 TDD;GUI 任务(10-14)按规格不做自动化测试,以 `cargo build` + 手动验收清单把关(任务 15 汇总全量验收)。

---

### Task 1: lib 脚手架

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `.gitignore`

**Interfaces:**
- Consumes: 无
- Produces: 可编译的 `gemsleuth` lib crate(edition 2024,此时无任何第三方依赖;`[[bin]]` 与 GUI 依赖推迟到 Task 10 再加,保证核心任务构建快)

- [ ] **Step 1: 创建工程文件**

`Cargo.toml`:

```toml
[package]
name = "gemsleuth"
version = "0.1.0"
edition = "2024"

[lib]
name = "gemsleuth"
path = "src/lib.rs"
```

`src/lib.rs`:

```rust
// gemsleuth 核心求解库。公共导出将在 Task 8 补全。
#[cfg(test)]
mod tests {
    #[test]
    fn harness_works() {
        assert_eq!(2 + 2, 4);
    }
}
```

`.gitignore`:

```gitignore
/target
```

- [ ] **Step 2: 运行测试确认通过**

Run: `cargo test`
Expected: `test result: ok. 1 passed`

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml src/lib.rs .gitignore
git commit -m "工程脚手架:gemsleuth lib crate(edition 2024)"
```

---

### Task 2: 数据模型与校验(core/model.rs)

**Files:**
- Create: `src/core/mod.rs`
- Create: `src/core/model.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: 无
- Produces(后续所有任务依赖,签名精确):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings { pub colors: usize, pub slots: usize, pub repeats: bool }
// Default = { colors: 6, slots: 4, repeats: true }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsError { ColorsOutOfRange, SlotsOutOfRange, SlotsExceedColors }
impl std::fmt::Display for SettingsError   // 中文文案,见 Step 3

impl Settings {
    pub fn validate(&self) -> Result<(), SettingsError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record { pub guess: Vec<u8>, pub exact: u8, pub partial: u8, pub enabled: bool }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordError { WrongLength, ColorOutOfRange, CountsTooLarge }
impl std::fmt::Display for RecordError

impl Record {
    pub fn new(guess: Vec<u8>, exact: u8, partial: u8) -> Record; // enabled = true
    pub fn validate(&self, settings: &Settings) -> Result<(), RecordError>;
}
```

- [ ] **Step 1: 写失败测试**

`src/core/model.rs`(先只写测试与空壳,`todo!()` 让编译通过、测试失败):

```rust
//! Settings / Record 数据模型与校验(§4.1、§4.5)。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub colors: usize,
    pub slots: usize,
    pub repeats: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsError { ColorsOutOfRange, SlotsOutOfRange, SlotsExceedColors }

impl Settings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub guess: Vec<u8>,
    pub exact: u8,
    pub partial: u8,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordError { WrongLength, ColorOutOfRange, CountsTooLarge }

impl Record {
    pub fn new(guess: Vec<u8>, exact: u8, partial: u8) -> Record {
        todo!()
    }
    pub fn validate(&self, settings: &Settings) -> Result<(), RecordError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_matches_game() {
        let s = Settings::default();
        assert_eq!((s.colors, s.slots, s.repeats), (6, 4, true));
    }

    #[test]
    fn settings_validate_bounds() {
        assert!(Settings { colors: 4, slots: 3, repeats: true }.validate().is_ok());
        assert!(Settings { colors: 8, slots: 6, repeats: true }.validate().is_ok());
        assert_eq!(Settings { colors: 3, slots: 4, repeats: true }.validate(), Err(SettingsError::ColorsOutOfRange));
        assert_eq!(Settings { colors: 9, slots: 4, repeats: true }.validate(), Err(SettingsError::ColorsOutOfRange));
        assert_eq!(Settings { colors: 6, slots: 2, repeats: true }.validate(), Err(SettingsError::SlotsOutOfRange));
        assert_eq!(Settings { colors: 6, slots: 7, repeats: true }.validate(), Err(SettingsError::SlotsOutOfRange));
    }

    #[test]
    fn settings_no_repeat_requires_slots_le_colors() {
        assert_eq!(
            Settings { colors: 4, slots: 5, repeats: false }.validate(),
            Err(SettingsError::SlotsExceedColors)
        );
        assert!(Settings { colors: 6, slots: 4, repeats: false }.validate().is_ok());
    }

    #[test]
    fn record_new_defaults_enabled() {
        let r = Record::new(vec![1, 2, 3, 4], 1, 0);
        assert!(r.enabled);
        assert_eq!(r.guess, vec![1, 2, 3, 4]);
        assert_eq!((r.exact, r.partial), (1, 0));
    }

    #[test]
    fn record_validate() {
        let s = Settings::default();
        assert!(Record::new(vec![0, 1, 2, 3], 4, 0).validate(&s).is_ok());
        assert!(Record::new(vec![5, 5, 5, 5], 2, 2).validate(&s).is_ok());
        assert_eq!(
            Record::new(vec![0, 1, 2], 0, 0).validate(&s),
            Err(RecordError::WrongLength)
        );
        assert_eq!(
            Record::new(vec![0, 1, 2, 6], 0, 0).validate(&s),
            Err(RecordError::ColorOutOfRange)
        );
        // exact + partial > slots(3+2=5 > 4);同时覆盖 exact ≤ slots
        assert_eq!(
            Record::new(vec![0, 1, 2, 3], 3, 2).validate(&s),
            Err(RecordError::CountsTooLarge)
        );
        assert_eq!(
            Record::new(vec![0, 1, 2, 3], 5, 0).validate(&s),
            Err(RecordError::CountsTooLarge)
        );
    }
}
```

`src/core/mod.rs`:

```rust
pub mod model;
```

`src/lib.rs`(整体替换):

```rust
pub mod core;
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test`
Expected: 5 个测试全部 FAIL(`todo!()` panic:"not yet implemented")

- [ ] **Step 3: 最小实现**

替换 `src/core/model.rs` 中的 `todo!()`(并补 `Default`、`Display`):

```rust
impl Default for Settings {
    fn default() -> Self {
        Self { colors: 6, slots: 4, repeats: true }
    }
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ColorsOutOfRange => write!(f, "颜色数必须在 4..=8"),
            Self::SlotsOutOfRange => write!(f, "槽位数必须在 3..=6"),
            Self::SlotsExceedColors => write!(f, "不允许重复时槽位数不能超过颜色数"),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !(4..=8).contains(&self.colors) {
            return Err(SettingsError::ColorsOutOfRange);
        }
        if !(3..=6).contains(&self.slots) {
            return Err(SettingsError::SlotsOutOfRange);
        }
        if !self.repeats && self.slots > self.colors {
            return Err(SettingsError::SlotsExceedColors);
        }
        Ok(())
    }
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongLength => write!(f, "宝石数必须等于槽位数"),
            Self::ColorOutOfRange => write!(f, "宝石索引超出颜色数范围"),
            Self::CountsTooLarge => write!(f, "蓝标数+金标数不能超过槽位数"),
        }
    }
}

impl Record {
    pub fn new(guess: Vec<u8>, exact: u8, partial: u8) -> Record {
        Record { guess, exact, partial, enabled: true }
    }

    pub fn validate(&self, settings: &Settings) -> Result<(), RecordError> {
        if self.guess.len() != settings.slots {
            return Err(RecordError::WrongLength);
        }
        if self.guess.iter().any(|&g| g as usize >= settings.colors) {
            return Err(RecordError::ColorOutOfRange);
        }
        if self.exact as usize + self.partial as usize > settings.slots {
            return Err(RecordError::CountsTooLarge);
        }
        Ok(())
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test`
Expected: `test result: ok. 5 passed`

- [ ] **Step 5: Commit**

```bash
git add src/core/mod.rs src/core/model.rs src/lib.rs
git commit -m "core: Settings/Record 数据模型与中文校验"
```

---

### Task 3: 反馈判定 judge(core/judge.rs)

**Files:**
- Create: `src/core/judge.rs`
- Modify: `src/core/mod.rs`

**Interfaces:**
- Consumes: 无
- Produces:

```rust
/// 标准 Mastermind 多重集计数(§4.2,已对真实游戏验证)。
/// 返回 (exact, partial):蓝标=位置与种类都对;金标=种类对位置错。
/// 长度相等由调用方保证(debug_assert)。
pub fn judge(guess: &[u8], candidate: &[u8]) -> (u8, u8);
```

- [ ] **Step 1: 写失败测试**

`src/core/judge.rs`:

```rust
//! 反馈判定:标准 Mastermind 多重集计数(§4.2)。
//!
//! exact   = Σ_i [guess[i] == candidate[i]]
//! overlap = Σ_色c min(count(guess,c), count(candidate,c))
//! partial = overlap − exact
//! 复杂度 O(slots²)(slots ≤ 6,无需更巧)。

/// 判定 guess 对 candidate 的反馈:(蓝标 exact, 金标 partial)。
pub fn judge(guess: &[u8], candidate: &[u8]) -> (u8, u8) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::judge;

    #[test]
    fn all_exact() {
        assert_eq!(judge(&[1, 2, 3, 4], &[1, 2, 3, 4]), (4, 0));
    }

    #[test]
    fn all_miss() {
        assert_eq!(judge(&[0, 0, 0, 0], &[1, 1, 1, 1]), (0, 0));
    }

    #[test]
    fn repeated_multiset_counting() {
        // 规格 §6 示例:guess=黄黄紫蓝 vs candidate=紫紫紫黄。
        // 按 §4.2 公式复核为 (1,1)(规格中 (0,2) 为笔误,见计划偏差表):
        // exact=1(第 3 位紫),overlap=min(黄2,黄1)+min(紫1,紫3)=2,partial=1。
        // 颜色索引:0红 1蓝 2紫 3橙 4黄 5绿。
        assert_eq!(judge(&[4, 4, 2, 1], &[2, 2, 2, 4]), (1, 1));
        // 重复宝石按较少一侧计数:紫黄紫紫 vs 紫紫紫黄
        assert_eq!(judge(&[2, 4, 2, 2], &[2, 2, 2, 4]), (2, 2));
    }

    #[test]
    fn real_puzzle_records() {
        // 真实谜面两条记录(规格 §5.1 GUI 草图)对唯一解 紫紫紫黄 的反馈——锁死判定语义
        assert_eq!(judge(&[3, 1, 2, 0], &[2, 2, 2, 4]), (1, 0)); // 橙蓝紫红
        assert_eq!(judge(&[3, 1, 2, 5], &[2, 2, 2, 4]), (1, 0)); // 橙蓝紫绿
    }

    #[test]
    fn partial_only_and_three_slots() {
        assert_eq!(judge(&[0, 0, 0, 0], &[0, 0, 0, 1]), (3, 0));
        assert_eq!(judge(&[0, 1, 2], &[2, 0, 1]), (0, 3));
    }
}
```

`src/core/mod.rs` 追加一行:

```rust
pub mod judge;
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test judge`
Expected: 5 个 judge 测试 FAIL(`todo!()` panic)

- [ ] **Step 3: 最小实现**

替换 `judge` 的 `todo!()`:

```rust
pub fn judge(guess: &[u8], candidate: &[u8]) -> (u8, u8) {
    debug_assert_eq!(guess.len(), candidate.len());
    let exact = guess.iter().zip(candidate).filter(|(g, c)| g == c).count() as u8;
    let mut overlap = 0usize;
    for (i, &color) in guess.iter().enumerate() {
        // 只在该颜色首次出现时统计一次,避免重复累加
        if guess[..i].contains(&color) {
            continue;
        }
        let g = guess.iter().filter(|&&x| x == color).count();
        let c = candidate.iter().filter(|&&x| x == color).count();
        overlap += g.min(c);
    }
    (exact, (overlap as u8) - exact)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test`
Expected: 全部通过(此前 5 个 model 测试 + 5 个 judge 测试)

- [ ] **Step 5: Commit**

```bash
git add src/core/judge.rs src/core/mod.rs
git commit -m "core: judge 多重集计数判定(含真实谜面金标准用例)"
```

---

### Task 4: 全空间枚举 enumerate_space(core/solver.rs)

**Files:**
- Create: `src/core/solver.rs`
- Modify: `src/core/mod.rs`

**Interfaces:**
- Consumes: `crate::core::model::Settings`(Task 2)
- Produces:

```rust
/// 枚举全部可能答案(§4.3 第 1 步)。
/// repeats=true → colors^slots 个,里程表序(末位变化最快,即 itertools.product 序);
/// repeats=false → 无重复全排列 P(colors, slots),字典序。
/// 要求 settings 已通过 validate(debug_assert)。
pub fn enumerate_space(settings: &Settings) -> Vec<Vec<u8>>;
```

**关键**:里程表序**必须**是"末位变化最快"(v[0] 是最高位),这是 Task 6/7 中"熵推荐开局 = [0,1,2,3]"等已验证期望值的序前提。

- [ ] **Step 1: 写失败测试**

`src/core/solver.rs`:

```rust
//! 全空间枚举 + 过滤 + 求解编排(§4.3)。

use crate::core::model::Settings;

/// 枚举全部可能答案。
pub fn enumerate_space(settings: &Settings) -> Vec<Vec<u8>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_space_size() {
        assert_eq!(enumerate_space(&Settings { colors: 6, slots: 4, repeats: true }).len(), 1296);
        assert_eq!(enumerate_space(&Settings { colors: 8, slots: 6, repeats: true }).len(), 262_144);
        assert_eq!(enumerate_space(&Settings { colors: 4, slots: 3, repeats: true }).len(), 64);
    }

    #[test]
    fn repeats_space_odometer_order() {
        let s = enumerate_space(&Settings { colors: 6, slots: 4, repeats: true });
        // 末位变化最快(与 itertools.product 一致;后续策略测试的期望值依赖此序)
        assert_eq!(s[0], vec![0, 0, 0, 0]);
        assert_eq!(s[1], vec![0, 0, 0, 1]);
        assert_eq!(s[6], vec![0, 0, 1, 0]);
        assert_eq!(s[1295], vec![5, 5, 5, 5]);
    }

    #[test]
    fn permutation_space() {
        let s = enumerate_space(&Settings { colors: 6, slots: 4, repeats: false });
        assert_eq!(s.len(), 360); // P(6,4)
        assert_eq!(s[0], vec![0, 1, 2, 3]);
        assert_eq!(s[1], vec![0, 1, 2, 4]);
        assert_eq!(s[359], vec![5, 4, 3, 2]);
        assert!(s.iter().all(|c| c.iter().collect::<std::collections::HashSet<_>>().len() == 4));
    }

    #[test]
    fn permutation_space_8_colors() {
        assert_eq!(enumerate_space(&Settings { colors: 8, slots: 4, repeats: false }).len(), 1680);
    }
}
```

`src/core/mod.rs` 追加:

```rust
pub mod solver;
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test solver`
Expected: 4 个测试 FAIL

- [ ] **Step 3: 最小实现**

```rust
pub fn enumerate_space(settings: &Settings) -> Vec<Vec<u8>> {
    debug_assert!(settings.validate().is_ok());
    let colors = settings.colors;
    let slots = settings.slots;
    let mut out = Vec::new();
    if settings.repeats {
        let total = colors.pow(slots as u32);
        out.reserve(total);
        for i in 0..total {
            // v[0] 是最高位:末位变化最快(itertools.product 序)
            let mut v = vec![0u8; slots];
            let mut i = i;
            for pos in (0..slots).rev() {
                v[pos] = (i % colors) as u8;
                i /= colors;
            }
            out.push(v);
        }
    } else {
        out.reserve(colors * (colors - 1).max(1));
        let mut cur = Vec::with_capacity(slots);
        let mut used = vec![false; colors];
        fn dfs(colors: usize, slots: usize, cur: &mut Vec<u8>, used: &mut [bool], out: &mut Vec<Vec<u8>>) {
            if cur.len() == slots {
                out.push(cur.clone());
                return;
            }
            for c in 0..colors {
                if used[c] {
                    continue;
                }
                used[c] = true;
                cur.push(c as u8);
                dfs(colors, slots, cur, used, out);
                cur.pop();
                used[c] = false;
            }
        }
        dfs(colors, slots, &mut cur, &mut used, &mut out);
    }
    out
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test`
Expected: 全部通过(14 个)

- [ ] **Step 5: Commit**

```bash
git add src/core/solver.rs src/core/mod.rs
git commit -m "core: 全空间枚举(重复幂空间里程表序/无重复排列字典序)"
```

---

### Task 5: 候选过滤 filter_candidates

**Files:**
- Modify: `src/core/solver.rs`

**Interfaces:**
- Consumes: `crate::core::judge::judge`(Task 3)、`crate::core::model::Record`(Task 2)、`enumerate_space`(Task 4)
- Produces:

```rust
/// 返回使所有 enabled 记录的判定反馈与录入值完全一致的候选(§4.3 第 2 步)。
/// 禁用的记录不参与过滤(F4/F5)。保持枚举序。
pub fn filter_candidates(settings: &Settings, records: &[Record]) -> Vec<Vec<u8>>;
```

- [ ] **Step 1: 写失败测试**

在 `src/core/solver.rs` 的 `mod tests` 中追加:

```rust
    use crate::core::judge::judge;
    use crate::core::model::Record;

    // 真实谜面两条记录(附录 A 金标准的前两条)
    fn ab_records() -> Vec<Record> {
        vec![
            Record::new(vec![3, 1, 2, 0], 1, 0), // 橙蓝紫红
            Record::new(vec![3, 1, 2, 5], 1, 0), // 橙蓝紫绿
        ]
    }

    #[test]
    fn filter_empty_records_is_full_space() {
        let s = Settings::default();
        assert_eq!(filter_candidates(&s, &[]).len(), 1296);
    }

    #[test]
    fn filter_real_two_records_leaves_24() {
        // 已由穷举脚本验证:橙蓝紫红(1,0) + 橙蓝紫绿(1,0) → 恰好 24 个候选
        let s = Settings::default();
        let cands = filter_candidates(&s, &ab_records());
        assert_eq!(cands.len(), 24);
        assert!(cands.iter().any(|c| c == &vec![2, 2, 2, 4])); // 紫紫紫黄
        assert!(cands.iter().any(|c| c == &vec![1, 1, 1, 1]));
        // 枚举序中第一个候选是 [1,1,1,1]
        assert_eq!(cands[0], vec![1, 1, 1, 1]);
    }

    #[test]
    fn filter_respects_all_enabled_records() {
        let s = Settings::default();
        let cands = filter_candidates(&s, &ab_records());
        assert!(cands.iter().all(|c| {
            judge(&[3, 1, 2, 0], c) == (1, 0) && judge(&[3, 1, 2, 5], c) == (1, 0)
        }));
    }

    #[test]
    fn disabled_record_ignored() {
        let s = Settings::default();
        let mut r = Record::new(vec![0, 1, 2, 3], 4, 0); // 声称红蓝紫橙即答案
        r.enabled = false;
        assert_eq!(filter_candidates(&s, &[r]).len(), 1296);
    }

    #[test]
    fn filter_contradiction_is_empty() {
        let s = Settings::default();
        let mut rs = ab_records();
        rs.push(Record::new(vec![0, 1, 2, 3], 4, 0)); // 与前两条矛盾
        assert!(filter_candidates(&s, &rs).is_empty());
    }
```

同时把 `filter_candidates` 的空壳加到 `src/core/solver.rs` 顶部实现区:

```rust
use crate::core::judge::judge;
use crate::core::model::Record;

/// 返回使所有 enabled 记录反馈一致的候选。
pub fn filter_candidates(settings: &Settings, records: &[Record]) -> Vec<Vec<u8>> {
    todo!()
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test filter`
Expected: 5 个 filter 测试 FAIL

- [ ] **Step 3: 最小实现**

```rust
pub fn filter_candidates(settings: &Settings, records: &[Record]) -> Vec<Vec<u8>> {
    enumerate_space(settings)
        .into_iter()
        .filter(|cand| {
            records
                .iter()
                .filter(|r| r.enabled)
                .all(|r| judge(&r.guess, cand) == (r.exact, r.partial))
        })
        .collect()
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test`
Expected: 全部通过(19 个)

- [ ] **Step 5: Commit**

```bash
git add src/core/solver.rs
git commit -m "core: 候选过滤(禁用记录不参与)"
```

---

### Task 6: 熵推荐 + 确定性采样(core/strategy.rs)

**Files:**
- Create: `src/core/strategy.rs`
- Modify: `src/core/mod.rs`

**Interfaces:**
- Consumes: `judge`(Task 3)、`enumerate_space`/`filter_candidates`(Task 4/5)、`Settings`/`Record`(Task 2)
- Produces(Task 7 依赖):

```rust
pub const FULL_SPACE_LIMIT: usize = 20_000;  // 全空间 ≤ 此值时熵/前瞻全量计算
pub const SAMPLE_SIZE: usize = 2048;         // 超过时固定种子采样

/// 猜测空间:全空间 ≤ FULL_SPACE_LIMIT 全量;否则固定种子采样 SAMPLE_SIZE 个(§4.4)。
pub(crate) fn guess_space(settings: &Settings) -> Vec<Vec<u8>>;

/// 无依赖确定性采样:xorshift64 固定种子取互异下标,保持原序;n ≥ space.len() 时返回全量。
pub(crate) fn sample_guesses(space: &[Vec<u8>], n: usize) -> Vec<Vec<u8>>;

/// 熵最大化推荐(§4.4 >30 候选层):选反馈分桶熵最大的猜测;
/// 平手(ε=1e-9)优先仍属候选集者,再取先枚举者。
/// 返回 (guess, entropy_bits, worst_bucket)。
pub(crate) fn entropy_best(candidates: &[Vec<u8>], guesses: &[Vec<u8>]) -> (Vec<u8>, f64, usize);
```

- [ ] **Step 1: 写失败测试**

`src/core/strategy.rs`:

```rust
//! 分层自适应推荐(§4.4):熵最大化 / 精确前瞻。

use std::collections::HashMap;

use crate::core::model::{Record, Settings};
use crate::core::solver::{enumerate_space, filter_candidates};

pub const FULL_SPACE_LIMIT: usize = 20_000;
pub const SAMPLE_SIZE: usize = 2048;

pub(crate) fn guess_space(settings: &Settings) -> Vec<Vec<u8>> {
    todo!()
}

pub(crate) fn sample_guesses(space: &[Vec<u8>], n: usize) -> Vec<Vec<u8>> {
    todo!()
}

pub(crate) fn entropy_best(candidates: &[Vec<u8>], guesses: &[Vec<u8>]) -> (Vec<u8>, f64, usize) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_space() -> Vec<Vec<u8>> {
        enumerate_space(&Settings::default())
    }

    fn ab_candidates() -> Vec<Vec<u8>> {
        filter_candidates(
            &Settings::default(),
            &[
                Record::new(vec![3, 1, 2, 0], 1, 0),
                Record::new(vec![3, 1, 2, 5], 1, 0),
            ],
        )
    }

    #[test]
    fn entropy_first_move_on_6x4_is_0123() {
        // 穷举脚本已验证:全 1296 空间上熵最大者(先枚举序取首个 argmax)
        let space = default_space();
        let (g, h, worst) = entropy_best(&space, &space);
        assert_eq!(g, vec![0, 1, 2, 3]); // 红蓝紫橙
        assert!((h - 3.056_671).abs() < 1e-4);
        assert_eq!(worst, 312);
    }

    #[test]
    fn entropy_after_two_real_records() {
        // 24 候选场景(脚本验证):最佳猜测 [3,4,4,1](橙黄黄蓝,不在候选集内)
        let cands = ab_candidates();
        let space = default_space();
        let (g, h, worst) = entropy_best(&cands, &space);
        assert_eq!(g, vec![3, 4, 4, 1]);
        assert!((h - 3.173_533).abs() < 1e-4);
        assert_eq!(worst, 5);
    }

    #[test]
    fn entropy_is_deterministic() {
        let cands = ab_candidates();
        let space = default_space();
        assert_eq!(entropy_best(&cands, &space), entropy_best(&cands, &space));
    }

    #[test]
    fn sample_covers_small_space_entirely() {
        // n ≥ 空间大小时返回全量(采样路径与全量路径一致的回归基础,规格 §6)
        let space = enumerate_space(&Settings { colors: 4, slots: 4, repeats: true });
        assert_eq!(space.len(), 256);
        assert_eq!(sample_guesses(&space, SAMPLE_SIZE), space);
    }

    #[test]
    fn sample_is_deterministic_and_distinct() {
        let space = enumerate_space(&Settings { colors: 8, slots: 6, repeats: true });
        let a = sample_guesses(&space, SAMPLE_SIZE);
        let b = sample_guesses(&space, SAMPLE_SIZE);
        assert_eq!(a, b); // 固定种子 → 结果可复现
        assert_eq!(a.len(), SAMPLE_SIZE);
        let uniq: std::collections::HashSet<_> = a.iter().collect();
        assert_eq!(uniq.len(), SAMPLE_SIZE); // 互不相同
    }

    #[test]
    fn guess_space_full_below_limit_and_sampled_above() {
        assert_eq!(guess_space(&Settings::default()).len(), 1296);           // 6^4 全量
        assert_eq!(guess_space(&Settings { colors: 8, slots: 6, repeats: true }).len(), SAMPLE_SIZE); // 8^6 采样
    }
}
```

`src/core/mod.rs` 追加:

```rust
pub mod strategy;
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test strategy`
Expected: 6 个测试 FAIL

- [ ] **Step 3: 最小实现**

替换三个 `todo!()`:

```rust
pub(crate) fn guess_space(settings: &Settings) -> Vec<Vec<u8>> {
    let space = enumerate_space(settings);
    if space.len() <= FULL_SPACE_LIMIT {
        space
    } else {
        sample_guesses(&space, SAMPLE_SIZE)
    }
}

pub(crate) fn sample_guesses(space: &[Vec<u8>], n: usize) -> Vec<Vec<u8>> {
    if n >= space.len() {
        return space.to_vec();
    }
    let mut rng: u64 = 0x9E37_79B9_7F4A_7C15; // 固定种子,结果可复现(§4.4)
    let mut used = vec![false; space.len()];
    let mut chosen: Vec<usize> = Vec::with_capacity(n);
    while chosen.len() < n {
        // xorshift64*
        rng ^= rng >> 12;
        rng ^= rng << 25;
        rng ^= rng >> 27;
        let idx = (rng.wrapping_mul(0x2545_F491_4F6C_DD1D) as usize) % space.len();
        if !used[idx] {
            used[idx] = true;
            chosen.push(idx);
        }
    }
    chosen.sort_unstable(); // 保持枚举序,保证平手裁决确定
    chosen.into_iter().map(|i| space[i].clone()).collect()
}

pub(crate) fn entropy_best(candidates: &[Vec<u8>], guesses: &[Vec<u8>]) -> (Vec<u8>, f64, usize) {
    let n = candidates.len() as f64;
    let cand_set: std::collections::HashSet<&Vec<u8>> = candidates.iter().collect();
    let mut best: Option<(Vec<u8>, f64, usize, bool)> = None; // (guess, 熵, 最坏桶, 是否属候选集)
    for g in guesses {
        let mut buckets: HashMap<(u8, u8), usize> = HashMap::new();
        for c in candidates {
            *buckets.entry(crate::core::judge::judge(g, c)).or_insert(0) += 1;
        }
        // 对桶大小排序后求和,消除 HashMap 遍历序带来的浮点误差
        let mut sizes: Vec<usize> = buckets.values().copied().collect();
        sizes.sort_unstable();
        let h = -sizes
            .iter()
            .map(|&v| { let p = v as f64 / n; p * p.log2() })
            .sum::<f64>();
        let worst = sizes[sizes.len() - 1];
        let in_cand = cand_set.contains(g);
        let replace = match &best {
            None => true,
            Some((_, bh, _, b_in)) => {
                h > bh + 1e-9 || ((h - bh).abs() <= 1e-9 && in_cand && !*b_in)
            }
        };
        if replace {
            best = Some((g.clone(), h, worst, in_cand));
        }
    }
    let (g, h, w, _) = best.expect("guesses 非空");
    (g, h, w)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test`
Expected: 全部通过(25 个)。注意 `entropy_first_move_on_6x4_is_0123` 涉及 1296×1296 次判定,debug 下约数秒,属正常。

- [ ] **Step 5: Commit**

```bash
git add src/core/strategy.rs src/core/mod.rs
git commit -m "core: 熵最大化推荐与固定种子采样"
```

---

### Task 7: 精确前瞻 + 分层调度(core/strategy.rs)

**Files:**
- Modify: `src/core/strategy.rs`

**Interfaces:**
- Consumes: Task 6 的 `guess_space`/`entropy_best`、Task 5 的 `filter_candidates`
- Produces(Task 8 与 GUI 依赖):

```rust
pub const LOOKAHEAD_CANDIDATE_LIMIT: usize = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum Bound {
    /// 精确前瞻:最多还需 N 步(含本次猜测)
    GuaranteedSteps(usize),
    /// 熵推荐:期望参考,非保证(期望信息量 bits / 最坏桶大小)
    Expected { entropy_bits: f64, worst_bucket: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Recommendation {
    Answer(Vec<u8>),                          // 剩余候选唯一
    Guess { guess: Vec<u8>, bound: Bound },   // 推荐猜测 + 最坏情况说明
}

/// 公开入口(§4.4):过滤出候选后分层推荐。
pub fn recommend(settings: &Settings, records: &[Record]) -> Recommendation;

/// 由已知候选集直接推荐(GUI 每帧缓存后调用;要求候选非空)。
pub(crate) fn recommend_for(settings: &Settings, candidates: &[Vec<u8>]) -> Recommendation;

/// minimax 前瞻:返回保证最少剩余步数(含本次猜测)的猜测与该步数;
/// 深度预算用尽无法证明 → None(理论下 ≤30 候选、cap=3 不会发生)。
/// 步数语义:steps(C)=1+min_g max_{非终局桶} steps(桶);反馈=(slots,0) 为终局桶;
/// 单候选桶 steps=1(直接提交)。
pub(crate) fn lookahead_best(candidates: &[Vec<u8>], guesses: &[Vec<u8>], depth_cap: usize)
    -> Option<(Vec<u8>, usize)>;
```

分层规则(§4.4 表):`=1` 直接答案;`=2` 推荐首个候选、保证 ≤2 步;`3..=30` 精确前瞻(cap 3),失败回退熵并如实标注;`>30` 熵最大化。

- [ ] **Step 1: 写失败测试**

在 `src/core/strategy.rs` 的 `mod tests` 中追加,并在实现区加 `todo!()` 空壳(见 Step 3 结构,先放空壳):

```rust
    fn c3() -> Vec<Vec<u8>> {
        vec![vec![0, 0, 0, 0], vec![0, 0, 0, 1], vec![1, 1, 1, 1]]
    }

    #[test]
    fn lookahead_c3_guarantees_2() {
        // 脚本验证:argmin(先枚举序)= [0,0,0,0],最优保证 2 步(3 候选不可能 1 步)
        let guesses = default_space();
        assert_eq!(lookahead_best(&c3(), &guesses, 3), Some((vec![0, 0, 0, 0], 2)));
    }

    #[test]
    fn lookahead_depth0_falls_back_to_none() {
        let guesses = default_space();
        assert_eq!(lookahead_best(&c3(), &guesses, 0), None);
    }

    #[test]
    fn lookahead_24_candidates_guarantees_3() {
        // 脚本验证:两条真实记录后 24 候选,最优保证 3 步,猜测 [0,0,4,4](红红黄黄)
        let guesses = default_space();
        assert_eq!(lookahead_best(&ab_candidates(), &guesses, 3), Some((vec![0, 0, 4, 4], 3)));
    }

    #[test]
    fn recommend_first_move_is_entropy() {
        // 1296 候选 > 30 → 熵层(熵值用近似比较,f64 精确相等不可靠)
        let rec = recommend(&Settings::default(), &[]);
        match rec {
            Recommendation::Guess {
                guess,
                bound: Bound::Expected { entropy_bits, worst_bucket },
            } => {
                assert_eq!(guess, vec![0, 1, 2, 3]);
                assert!((entropy_bits - 3.056_671).abs() < 1e-4);
                assert_eq!(worst_bucket, 312);
            }
            other => panic!("应熵推荐,实际 {other:?}"),
        }
    }

    #[test]
    fn recommend_two_candidates_special_case() {
        // 脚本验证:这 5 条记录后恰好剩 {紫紫紫黄, 黄紫紫紫} 两个候选
        let records = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
            Record::new(vec![0, 1, 2, 3], 1, 0),
            Record::new(vec![0, 0, 1, 1], 0, 0),
            Record::new(vec![2, 4, 2, 2], 2, 2),
        ];
        let rec = recommend(&Settings::default(), &records);
        assert_eq!(
            rec,
            Recommendation::Guess { guess: vec![2, 2, 2, 4], bound: Bound::GuaranteedSteps(2) }
        );
    }

    #[test]
    fn recommend_24_candidates_uses_lookahead() {
        let records = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
        ];
        let rec = recommend(&Settings::default(), &records);
        assert_eq!(
            rec,
            Recommendation::Guess { guess: vec![0, 0, 4, 4], bound: Bound::GuaranteedSteps(3) }
        );
    }

    #[test]
    fn recommend_dispatch_boundary() {
        // 31 候选 → 熵层;唯一候选 → Answer
        let space = default_space();
        let s = Settings::default();
        let c31 = &space[..31];
        assert!(matches!(
            recommend_for(&s, c31),
            Recommendation::Guess { bound: Bound::Expected { .. }, .. }
        ));
        assert_eq!(
            recommend_for(&s, &space[..1]),
            Recommendation::Answer(vec![0, 0, 0, 0])
        );
    }

    #[test]
    fn guarantee_simulated_to_end() {
        // 规格 §6:模拟到终局核对保证步数确实成立。
        // worst_case_steps:对每个可能真答案,沿推荐猜测递归模拟,返回最大提交次数。
        fn worst_case_steps(settings: &Settings, records: &[Record]) -> usize {
            let cands = filter_candidates(settings, records);
            if cands.len() <= 1 {
                return 1;
            }
            let guess = match recommend_for(settings, &cands) {
                Recommendation::Answer(_) => return 1,
                Recommendation::Guess { guess, .. } => guess,
            };
            let mut worst = 0usize;
            for ans in &cands {
                let fb = crate::core::judge::judge(&guess, ans);
                if fb == (settings.slots as u8, 0) {
                    worst = worst.max(1);
                    continue;
                }
                let mut next = records.to_vec();
                next.push(Record { guess: guess.clone(), exact: fb.0, partial: fb.1, enabled: true });
                worst = worst.max(1 + worst_case_steps(settings, &next));
            }
            worst
        }
        let s = Settings::default();
        let ab = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
        ];
        assert_eq!(worst_case_steps(&s, &ab), 3); // 与 GuaranteedSteps(3) 一致
        let two = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
            Record::new(vec![0, 1, 2, 3], 1, 0),
            Record::new(vec![0, 0, 1, 1], 0, 0),
            Record::new(vec![2, 4, 2, 2], 2, 2),
        ];
        assert_eq!(worst_case_steps(&s, &two), 2); // 与 GuaranteedSteps(2) 一致
    }
}
```

(注意:上面最后一段测试代码以 `}` 结束时,是关闭原有的 `mod tests`;执行时把整段并入现有 `mod tests`,确保括号配平。)

同时在实现区追加空壳:

```rust
pub const LOOKAHEAD_CANDIDATE_LIMIT: usize = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum Bound {
    GuaranteedSteps(usize),
    Expected { entropy_bits: f64, worst_bucket: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Recommendation {
    Answer(Vec<u8>),
    Guess { guess: Vec<u8>, bound: Bound },
}

pub fn recommend(settings: &Settings, records: &[Record]) -> Recommendation {
    todo!()
}

pub(crate) fn recommend_for(settings: &Settings, candidates: &[Vec<u8>]) -> Recommendation {
    todo!()
}

pub(crate) fn lookahead_best(
    candidates: &[Vec<u8>],
    guesses: &[Vec<u8>],
    depth_cap: usize,
) -> Option<(Vec<u8>, usize)> {
    todo!()
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test strategy`
Expected: 新增 8 个测试 FAIL(`todo!()` panic)

- [ ] **Step 3: 最小实现**

```rust
pub fn recommend(settings: &Settings, records: &[Record]) -> Recommendation {
    let candidates = filter_candidates(settings, records);
    assert!(
        !candidates.is_empty(),
        "recommend: 候选为空属于矛盾,调用方(solve/GUI)应先检查"
    );
    recommend_for(settings, &candidates)
}

pub(crate) fn recommend_for(settings: &Settings, candidates: &[Vec<u8>]) -> Recommendation {
    match candidates.len() {
        1 => Recommendation::Answer(candidates[0].clone()),
        2 => Recommendation::Guess {
            // 猜其中之一:命中即结束,未中则另一个即答案,必 ≤2 步(§4.4)
            guess: candidates[0].clone(),
            bound: Bound::GuaranteedSteps(2),
        },
        n if n <= LOOKAHEAD_CANDIDATE_LIMIT => {
            let guesses = guess_space(settings);
            if let Some((guess, steps)) = lookahead_best(candidates, &guesses, 3) {
                Recommendation::Guess { guess, bound: Bound::GuaranteedSteps(steps) }
            } else {
                // 深度上限内无法给出保证(理论下不会发生):回退熵推荐并如实标注为期望值
                let (guess, h, worst) = entropy_best(candidates, &guesses);
                Recommendation::Guess {
                    guess,
                    bound: Bound::Expected { entropy_bits: h, worst_bucket: worst },
                }
            }
        }
        _ => {
            let guesses = guess_space(settings);
            let (guess, h, worst) = entropy_best(candidates, &guesses);
            Recommendation::Guess {
                guess,
                bound: Bound::Expected { entropy_bits: h, worst_bucket: worst },
            }
        }
    }
}

/// 记忆化键:(剩余深度, 候选集扁平化)。值 None = 该预算下无法证明。
fn lookahead_steps(
    cands: &[Vec<u8>],
    guesses: &[Vec<u8>],
    terminal: (u8, u8),
    budget: usize,
    memo: &mut HashMap<(usize, Vec<u8>), Option<usize>>,
) -> Option<usize> {
    if cands.len() == 1 {
        return Some(1); // 已知答案,提交 1 次
    }
    if budget == 0 {
        return None;
    }
    let key = (budget, cands.iter().flat_map(|c| c.iter().copied()).collect::<Vec<u8>>());
    if let Some(cached) = memo.get(&key) {
        return *cached;
    }
    let mut best: Option<usize> = None;
    for g in guesses {
        let mut buckets: HashMap<(u8, u8), Vec<Vec<u8>>> = HashMap::new();
        for c in cands {
            buckets.entry(crate::core::judge::judge(g, c)).or_default().push(c.clone());
        }
        let mut worst = 0usize;
        let mut provable = true;
        for (fb, bucket) in &buckets {
            if *fb == terminal {
                continue; // 猜中答案,终局
            }
            match lookahead_steps(bucket, guesses, terminal, budget - 1, memo) {
                Some(v) => worst = worst.max(v),
                None => {
                    provable = false;
                    break;
                }
            }
        }
        if provable {
            let total = 1 + worst;
            if best.is_none_or(|b| total < b) {
                best = Some(total);
            }
        }
    }
    memo.insert(key, best);
    best
}

pub(crate) fn lookahead_best(
    candidates: &[Vec<u8>],
    guesses: &[Vec<u8>],
    depth_cap: usize,
) -> Option<(Vec<u8>, usize)> {
    debug_assert!(!candidates.is_empty() && !guesses.is_empty());
    let slots = candidates[0].len() as u8;
    let terminal = (slots, 0);
    let mut memo: HashMap<(usize, Vec<u8>), Option<usize>> = HashMap::new();
    let best = lookahead_steps(candidates, guesses, terminal, depth_cap, &mut memo)?;
    // 找到达成该保证的第一个猜测(枚举序,裁决确定)
    for g in guesses {
        let mut buckets: HashMap<(u8, u8), Vec<Vec<u8>>> = HashMap::new();
        for c in candidates {
            buckets.entry(crate::core::judge::judge(g, c)).or_default().push(c.clone());
        }
        let mut worst = 0usize;
        let mut provable = true;
        for (fb, bucket) in &buckets {
            if *fb == terminal {
                continue;
            }
            match lookahead_steps(bucket, guesses, terminal, depth_cap - 1, &mut memo) {
                Some(v) => worst = worst.max(v),
                None => {
                    provable = false;
                    break;
                }
            }
        }
        if provable && 1 + worst == best {
            return Some((g.clone(), best));
        }
    }
    unreachable!("已由 lookahead_steps 证明存在达成保证的猜测")
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test`
Expected: 全部通过(33 个)。`lookahead_24_candidates_guarantees_3` 与 `guarantee_simulated_to_end` 是最重的两个测试,debug 下合计约 40 秒,属正常(release 下秒级)。

- [ ] **Step 5: Commit**

```bash
git add src/core/strategy.rs
git commit -m "core: 精确前瞻(记忆化 minimax)与分层调度,终局模拟核对保证步数"
```

---

### Task 8: solve 编排 + 嫌疑定位 + 金标准(core/solver.rs)

**Files:**
- Modify: `src/core/solver.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `filter_candidates`(Task 5)、`recommend_for`/`Recommendation`(Task 7)
- Produces(GUI 与最终用户依赖):

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum SolveOutcome {
    Unique(Vec<u8>),
    Ambiguous { candidates: Vec<Vec<u8>>, recommendation: Recommendation },
    Contradiction,
}

pub fn solve(settings: &Settings, records: &[Record]) -> SolveOutcome;
/// 矛盾排查(F5):当前整体矛盾时,返回"禁用后候选恢复非空"的记录下标;非矛盾返回空。
pub fn suspect_records(settings: &Settings, records: &[Record]) -> Vec<usize>;
```

`src/lib.rs` 最终公共导出:

```rust
pub mod core;

pub use core::judge::judge;
pub use core::model::{Record, RecordError, Settings, SettingsError};
pub use core::solver::{enumerate_space, filter_candidates, solve, suspect_records, SolveOutcome};
pub use core::strategy::{recommend, Bound, Recommendation};
```

- [ ] **Step 1: 写失败测试**

在 `src/core/solver.rs` 的 `mod tests` 顶部补导入并追加测试:

```rust
    use crate::core::strategy::{Bound, Recommendation};

    // 金标准:6 条记录唯一解出 紫紫紫黄(附录 A;前 2 条为真实谜面,后 4 条构造补充)
    fn golden_records() -> Vec<Record> {
        vec![
            Record::new(vec![3, 1, 2, 0], 1, 0), // 橙蓝紫红(真实)
            Record::new(vec![3, 1, 2, 5], 1, 0), // 橙蓝紫绿(真实)
            Record::new(vec![0, 1, 2, 3], 1, 0), // 红蓝紫橙
            Record::new(vec![0, 0, 1, 1], 0, 0), // 红红蓝蓝
            Record::new(vec![4, 5, 0, 1], 0, 1), // 黄绿红蓝
            Record::new(vec![2, 4, 2, 2], 2, 2), // 紫黄紫紫
        ]
    }

    fn contradiction_records() -> Vec<Record> {
        let mut rs = golden_records();
        rs.truncate(2);
        rs.push(Record::new(vec![0, 1, 2, 3], 4, 0)); // 声称红蓝紫橙即答案,与前两条矛盾
        rs
    }

    #[test]
    fn golden_puzzle_solves_unique() {
        let oc = solve(&Settings::default(), &golden_records());
        assert_eq!(oc, SolveOutcome::Unique(vec![2, 2, 2, 4])); // 紫紫紫黄
    }

    #[test]
    fn solve_is_deterministic() {
        let s = Settings::default();
        assert_eq!(solve(&s, &golden_records()), solve(&s, &golden_records()));
    }

    #[test]
    fn solve_contradiction_and_suspects() {
        let s = Settings::default();
        let rs = contradiction_records();
        assert_eq!(solve(&s, &rs), SolveOutcome::Contradiction);
        assert_eq!(suspect_records(&s, &rs), vec![2]); // 第 3 条(4,0)是嫌疑
        // 禁用嫌疑记录后恢复为 24 候选的 Ambiguous
        let mut fixed = rs.clone();
        fixed[2].enabled = false;
        match solve(&s, &fixed) {
            SolveOutcome::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), 24),
            other => panic!("应恢复为 Ambiguous,实际 {other:?}"),
        }
    }

    #[test]
    fn solve_empty_records_ambiguous_with_entropy_recommendation() {
        match solve(&Settings::default(), &[]) {
            SolveOutcome::Ambiguous { candidates, recommendation } => {
                assert_eq!(candidates.len(), 1296);
                match recommendation {
                    Recommendation::Guess {
                        guess,
                        bound: Bound::Expected { entropy_bits, worst_bucket },
                    } => {
                        assert_eq!(guess, vec![0, 1, 2, 3]);
                        assert!((entropy_bits - 3.056_671).abs() < 1e-4);
                        assert_eq!(worst_bucket, 312);
                    }
                    other => panic!("应熵推荐,实际 {other:?}"),
                }
            }
            other => panic!("应 Ambiguous,实际 {other:?}"),
        }
    }

    #[test]
    fn solve_two_candidates_ambiguous() {
        let rs = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
            Record::new(vec![0, 1, 2, 3], 1, 0),
            Record::new(vec![0, 0, 1, 1], 0, 0),
            Record::new(vec![2, 4, 2, 2], 2, 2),
        ];
        match solve(&Settings::default(), &rs) {
            SolveOutcome::Ambiguous { candidates, recommendation } => {
                assert_eq!(candidates, vec![vec![2, 2, 2, 4], vec![4, 2, 2, 2]]);
                assert_eq!(
                    recommendation,
                    Recommendation::Guess { guess: vec![2, 2, 2, 4], bound: Bound::GuaranteedSteps(2) }
                );
            }
            other => panic!("应 Ambiguous,实际 {other:?}"),
        }
    }

    #[test]
    fn permutation_mode_smoke() {
        // 规格 §6 排列模式冒烟:repeats=false 下求解正确性(性质断言)
        // 注:不能选 (1,0) 这类反馈——6 色中猜测外仅剩 2 色,凑不出"只共享 1 色"的 4 互异候选,必然矛盾
        let s = Settings { colors: 6, slots: 4, repeats: false };
        let rs = [Record::new(vec![0, 1, 2, 3], 2, 2)];
        let cands = filter_candidates(&s, &rs);
        assert!(!cands.is_empty());
        assert!(cands.iter().any(|c| c == &vec![1, 0, 2, 3])); // 换位前两个槽即满足 (2,2)
        assert!(cands.iter().all(|c| {
            c.iter().collect::<std::collections::HashSet<_>>().len() == 4  // 无重复
                && judge(&[0, 1, 2, 3], c) == (2, 2)                       // 满足记录
        }));
    }
```

实现区空壳:

```rust
use crate::core::strategy::{recommend_for, Recommendation};

#[derive(Debug, Clone, PartialEq)]
pub enum SolveOutcome {
    Unique(Vec<u8>),
    Ambiguous { candidates: Vec<Vec<u8>>, recommendation: Recommendation },
    Contradiction,
}

pub fn solve(settings: &Settings, records: &[Record]) -> SolveOutcome {
    todo!()
}

pub fn suspect_records(settings: &Settings, records: &[Record]) -> Vec<usize> {
    todo!()
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test`
Expected: 新增 6 个测试 FAIL(`todo!()` panic)

- [ ] **Step 3: 最小实现**

```rust
pub fn solve(settings: &Settings, records: &[Record]) -> SolveOutcome {
    let candidates = filter_candidates(settings, records);
    match candidates.len() {
        0 => SolveOutcome::Contradiction,
        1 => SolveOutcome::Unique(candidates.into_iter().next().unwrap()),
        _ => SolveOutcome::Ambiguous {
            recommendation: recommend_for(settings, &candidates),
            candidates,
        },
    }
}

pub fn suspect_records(settings: &Settings, records: &[Record]) -> Vec<usize> {
    if !filter_candidates(settings, records).is_empty() {
        return vec![]; // 整体不矛盾时无嫌疑可谈
    }
    (0..records.len())
        .filter(|&i| {
            records[i].enabled && {
                let mut others = records.to_vec();
                others[i].enabled = false;
                !filter_candidates(settings, &others).is_empty()
            }
        })
        .collect()
}
```

- [ ] **Step 4: 运行测试确认通过并更新 lib 导出**

把 `src/lib.rs` 整体替换为上面 Interfaces 中给出的最终导出,然后:

Run: `cargo test`
Expected: 全部通过(39 个)

- [ ] **Step 5: Commit**

```bash
git add src/core/solver.rs src/lib.rs
git commit -m "core: solve 三分支编排+嫌疑定位;金标准测试锁死唯一解紫紫紫黄"
```

---

### Task 9: 占位素材生成器(examples/gen_assets.rs)

**Files:**
- Create: `examples/gen_assets.rs`
- Create(生成): `assets/gems/gem_0.png … gem_7.png`、`assets/icons/exact.png`、`assets/icons/partial.png`、`assets/icons/unknown.png`

**Interfaces:**
- Consumes: 无(零依赖,手写 PNG 编码:zlib 存储块 + adler32 + crc32)
- Produces: 符合规格 §5.2 素材接口的占位图(宝石 128×128 圆形、图标 64×64)。真实素材到位后**同名替换文件**即可,不改任何代码。像素解码正确性由 Task 11 的 `image::load_from_memory` 测试兜底验证。

- [ ] **Step 1: 写生成器**

`examples/gen_assets.rs`:

```rust
//! 占位素材生成器(零第三方依赖,手写最小 PNG 编码)。
//! 运行:`cargo run --example gen_assets`
//! 真实素材到位后同名替换 assets/ 下文件重编译即可(规格 §5.2)。

use std::fs;
use std::io;
use std::path::Path;

// ---------- PNG 编码:zlib 存储块(无压缩)+ adler32 + crc32 ----------

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// zlib 流,全部用存储(不压缩)块:格式合法即可,体积无关紧要。
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    if raw.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    } else {
        let chunks: Vec<&[u8]> = raw.chunks(65535).collect();
        for (i, c) in chunks.iter().enumerate() {
            let last = i == chunks.len() - 1;
            let len = c.len() as u16;
            out.push(if last { 1 } else { 0 });
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(c);
        }
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for y in 0..height {
        raw.push(0); // 每行 filter = None
        for x in 0..width {
            raw.extend_from_slice(&pixel(x, y));
        }
    }
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8bit, RGBA, 无压缩/滤波/隔行
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

// ---------- 绘制(圆形占位) ----------

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 4] {
    let m = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), 255]
}

fn circle_png(size: u32, fill: [u8; 3], highlight: bool, ring: Option<([u8; 3], f64, f64)>) -> Vec<u8> {
    let c = size as f64 / 2.0;
    let r = c - 2.0;
    png(size, size, |x, y| {
        let (fx, fy) = (x as f64 + 0.5 - c, y as f64 + 0.5 - c);
        let d2 = fx * fx + fy * fy;
        if d2 > r * r {
            return [0, 0, 0, 0]; // 透明背景
        }
        if let Some((ring_rgb, r0, r1)) = ring {
            let rr = d2.sqrt();
            if rr >= r * r0 && rr <= r * r1 {
                return [ring_rgb[0], ring_rgb[1], ring_rgb[2], 255];
            }
        }
        if highlight {
            let (hx, hy) = (fx + r * 0.35, fy + r * 0.35);
            if hx * hx + hy * hy <= (r * 0.45) * (r * 0.45) {
                return mix(fill, [255, 255, 255], 0.45); // 左上高光
            }
        }
        [fill[0], fill[1], fill[2], 255]
    })
}

// ---------- 主流程 ----------

fn assert_png(b: &[u8], w: u32, h: u32) {
    assert_eq!(&b[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "PNG 签名");
    assert_eq!(u32::from_be_bytes(b[16..20].try_into().unwrap()), w, "宽度");
    assert_eq!(u32::from_be_bytes(b[20..24].try_into().unwrap()), h, "高度");
}

fn write(path: &str, bytes: &[u8]) -> io::Result<()> {
    fs::write(Path::new(path), bytes)?;
    println!("{path} ({} 字节)", bytes.len());
    Ok(())
}

fn main() -> io::Result<()> {
    // 颜色索引:0红 1蓝 2紫 3橙 4黄 5绿 6青 7白(规格 §5.2)
    const GEM_COLORS: [[u8; 3]; 8] = [
        [229, 57, 53],   // 红
        [30, 136, 229],  // 蓝
        [142, 36, 170],  // 紫
        [251, 140, 0],   // 橙
        [253, 216, 53],  // 黄
        [67, 160, 71],   // 绿
        [0, 172, 193],   // 青
        [224, 224, 224], // 白
    ];

    fs::create_dir_all("assets/gems")?;
    fs::create_dir_all("assets/icons")?;

    for (i, rgb) in GEM_COLORS.iter().enumerate() {
        let bytes = circle_png(128, *rgb, true, None);
        assert_png(&bytes, 128, 128);
        write(&format!("assets/gems/gem_{i}.png"), &bytes)?;
    }

    let icons = [
        ("assets/icons/exact.png", circle_png(64, [30, 136, 229], false, None)),   // 蓝标
        ("assets/icons/partial.png", circle_png(64, [255, 193, 7], false, None)),  // 金标
        ("assets/icons/unknown.png", circle_png(64, [158, 158, 158], false, Some(([96, 96, 96], 0.5, 0.68)))),
    ];
    for (path, bytes) in icons {
        assert_png(&bytes, 64, 64);
        write(path, &bytes)?;
    }
    Ok(())
}
```

- [ ] **Step 2: 运行生成并自检**

Run: `cargo run --example gen_assets`
Expected: 打印 11 个文件路径与字节数。采用 zlib 存储块(不压缩),宝石约 65KB、图标约 17KB——体积无关紧要,真实素材同名替换后即为正常体积。无 panic;`ls assets/gems assets/icons` 可见 11 个 PNG。

- [ ] **Step 3: Commit**

```bash
git add examples/gen_assets.rs assets/
git commit -m "素材接口:零依赖占位图生成器(宝石128px/图标64px,同名替换即生效)"
```

---

### Task 10: GUI 骨架(bin + eframe + 设置栏 + Tab + 中文字体)

**Files:**
- Modify: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/gui/mod.rs`
- Modify: `src/lib.rs`(追加 `pub mod gui;`)

**Interfaces:**
- Consumes: `Settings`/`Record`(Task 2)及 lib 导出(Task 8)
- Produces(Task 11-14 依赖):

```rust
// src/gui/mod.rs
pub struct SessionState { pub settings: Settings, pub records: Vec<Record> }
#[derive(Clone, Copy, PartialEq)] pub enum Tab { Solve, Assistant }
#[derive(Default)]
pub struct CachedAnalysis {
    pub candidates: Vec<Vec<u8>>,
    pub recommendation: Option<Recommendation>, // 候选空时为 None
    pub suspects: Vec<usize>,                   // 矛盾时的嫌疑记录下标
}
pub struct GemsleuthApp { /* session, tab, editor, pending_settings, dirty, cached, solve_outcome, assets, font_warning */ }
impl GemsleuthApp { pub fn new(cc: &eframe::CreationContext) -> Self }
// eframe::App 实现:logic() 里做脏标记重算(filter+recommend+suspects),ui() 里画界面
```

依赖加入 `Cargo.toml`(锁定版本,GUI API 均已实测):

```toml
[dependencies]
eframe = "0.36.1"
image = { version = "0.25.10", default-features = false, features = ["png"] }

[[bin]]
name = "gemsleuth"
path = "src/main.rs"
```

**说明**:Task 11 之前 `Assets` 尚不存在,本任务的 `GemsleuthApp` 暂不持有 assets 字段(用 `()` 占位不行——直接推迟:本任务 App 只有 session/tab/pending/dirty/cached/font_warning;Task 11 再加入 assets 字段)。`ui()` 里 records/结果区先用占位标签,Task 12-14 逐个替换——这是**集成顺序安排**,不是偷懒:每步都 `cargo build` + 启动验收。

- [ ] **Step 1: 写 main.rs 与 App 骨架**

`src/main.rs`:

```rust
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([700.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Gemsleuth 宝石推理求解器",
        options,
        Box::new(|cc| Ok(Box::new(gemsleuth::gui::GemsleuthApp::new(cc)))),
    )
}
```

`src/lib.rs` 追加:

```rust
pub mod gui;
```

`src/gui/mod.rs`:

```rust
//! egui 薄壳:会话状态、脏标记重算、设置栏、Tab 切换(§5)。

use eframe::egui;

use gemsleuth::{Record, Recommendation, Settings, SolveOutcome};

pub mod assistant_panel;
pub mod palette;
pub mod records_panel;
pub mod solve_panel;
pub mod assets;

#[derive(Default)]
pub struct SessionState {
    pub settings: Settings,
    pub records: Vec<Record>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tab { Solve, Assistant }

#[derive(Default)]
pub struct CachedAnalysis {
    pub candidates: Vec<Vec<u8>>,
    pub recommendation: Option<Recommendation>,
    pub suspects: Vec<usize>,
}

pub struct GemsleuthApp {
    pub session: SessionState,
    pub tab: Tab,
    pub pending_settings: Option<Settings>, // 待确认的新设置(确认弹窗)
    pub dirty: bool,                        // 任一会话变更 → 重算
    pub cached: CachedAnalysis,
    pub solve_outcome: Option<SolveOutcome>, // 整卷求解快照(点击求解时更新)
    pub font_warning: bool,
}

impl GemsleuthApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let font_warning = !install_cjk_fonts(&cc.egui_ctx);
        Self {
            session: SessionState::default(),
            tab: Tab::Solve,
            pending_settings: None,
            dirty: true,
            cached: CachedAnalysis::default(),
            solve_outcome: None,
            font_warning,
        }
    }
}

/// 运行时探测系统中文字体(不分发字体文件,规避许可,§5.3)。
fn install_cjk_fonts(ctx: &egui::Context) -> bool {
    const CANDIDATES: &[&str] = &[
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".into(), egui::FontData::from_owned(bytes).into());
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.get_mut(&fam).unwrap().insert(0, "cjk".into());
            }
            ctx.set_fonts(fonts);
            return true;
        }
    }
    false
}

fn settings_bar(ui: &mut egui::Ui, session: &mut SessionState, pending: &mut Option<Settings>) {
    ui.horizontal(|ui| {
        ui.label("设置:");
        let mut next = session.settings;
        egui::ComboBox::from_label("颜色数")
            .selected_text(format!("{}", session.settings.colors))
            .show_ui(ui, |ui| {
                for c in 4..=8 {
                    ui.selectable_value(&mut next.colors, c, format!("{c}"));
                }
            });
        // 不允许重复时,槽位数选项收窄到 ≤ 颜色数(§4.5 禁止非法组合)
        egui::ComboBox::from_label("槽位数")
            .selected_text(format!("{}", session.settings.slots))
            .show_ui(ui, |ui| {
                let max = if next.repeats { 6 } else { next.colors.min(6) };
                for s in 3..=max {
                    ui.selectable_value(&mut next.slots, s, format!("{s}"));
                }
            });
        ui.checkbox(&mut next.repeats, "允许重复");
        if !next.repeats && next.slots > next.colors {
            ui.colored_label(egui::Color32::RED, "不允许重复时槽位数不能超过颜色数");
        } else if next != session.settings {
            *pending = Some(next); // 任何设置变化都弹确认(确认后清空记录,§4.5)
        }
        if ui.button("重置会话").clicked() {
            *pending = Some(session.settings); // 设置不变,确认后仅清空记录
        }
    });
}

fn confirm_dialog(ui: &mut egui::Ui, pending: &mut Option<Settings>, session: &mut SessionState, dirty: &mut bool) {
    if pending.is_none() {
        return;
    }
    let ctx = ui.ctx().clone();
    egui::Window::new("确认修改设置")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.label("修改设置将清空当前全部记录,确定吗?");
            ui.horizontal(|ui| {
                if ui.button("确定").clicked() {
                    session.settings = pending.take().unwrap();
                    session.records.clear();
                    *dirty = true;
                }
                if ui.button("取消").clicked() {
                    *pending = None;
                }
            });
        });
}

impl eframe::App for GemsleuthApp {
    /// 每帧 UI 前调用,禁止画 UI——正好承载脏标记重算(§5.1 实时刷新且不卡帧)。
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.dirty {
            return;
        }
        let candidates = gemsleuth::filter_candidates(&self.session.settings, &self.session.records);
        let (recommendation, suspects) = if candidates.is_empty() {
            (
                None,
                gemsleuth::suspect_records(&self.session.settings, &self.session.records),
            )
        } else {
            (
                Some(gemsleuth::core::strategy::recommend_for(&self.session.settings, &candidates)),
                vec![],
            )
        };
        self.cached = CachedAnalysis { candidates, recommendation, suspects };
        self.dirty = false;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            if self.font_warning {
                ui.colored_label(egui::Color32::YELLOW, "警告:未找到系统中文字体,中文可能无法显示");
            }
            egui::Panel::top(egui::Id::new("settings")).show(ui, |ui| {
                settings_bar(ui, &mut self.session, &mut self.pending_settings);
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Solve, "整卷求解");
                ui.selectable_value(&mut self.tab, Tab::Assistant, "陪玩助手");
            });
            ui.separator();
            // Task 12-14 将替换以下占位为 records_panel / solve_panel / assistant_panel
            ui.label(format!("记录数:{}", self.session.records.len()));
            ui.label(format!("实时候选数:{}", self.cached.candidates.len()));
            confirm_dialog(ui, &mut self.pending_settings, &mut self.session, &mut self.dirty);
        });
    }
}
```

同时创建空面板模块(Task 12-14 填充),让编译通过——`src/gui/assets.rs`、`src/gui/palette.rs`、`src/gui/records_panel.rs`、`src/gui/solve_panel.rs`、`src/gui/assistant_panel.rs` 每个文件先只放一行注释如 `//! Task 12 填充。`。

- [ ] **Step 2: 构建 + 启动验收**

Run: `cargo build`
Expected: 编译通过(首次拉取编译 eframe 约数分钟)。

Run: `cargo run`,人工验收后关闭窗口:
- 窗口标题「Gemsleuth 宝石推理求解器」,约 1000×700,可缩放
- **中文正常渲染**(设置:/颜色数/槽位数/允许重复/整卷求解/陪玩助手),无方块
- 下拉框可选 4..8 / 3..6;勾选掉「允许重复」后槽位数选项收窄
- 修改任一设置 → 中央弹出「确认修改设置」;点取消不变,点确定后界面重置
- 若中文字体加载失败会有黄色警告条(msyh.ttc 正常应无警告)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/gui/ src/lib.rs
git commit -m "gui: eframe 骨架(中文字体探测/设置确认弹窗/Tab 切换/脏标记重算)"
```

---

### Task 11: 素材装载 + 调色板(gui/assets.rs, gui/palette.rs)

**Files:**
- Modify: `src/gui/assets.rs`(替换占位注释)
- Modify: `src/gui/palette.rs`(替换占位注释)
- Modify: `src/gui/mod.rs`(App 持有 `Assets`,创建时加载)

**Interfaces:**
- Consumes: Task 9 生成的 `assets/**.png`(经 `include_bytes!` 嵌入)
- Produces(Task 12-14 依赖):

```rust
// src/gui/assets.rs
pub struct Assets {
    pub gems: Vec<egui::TextureHandle>, // 8 个,索引即颜色
    pub exact: egui::TextureHandle,
    pub partial: egui::TextureHandle,
    pub unknown: egui::TextureHandle,
}
impl Assets {
    pub fn load(ctx: &egui::Context) -> Self;
    pub fn gem(&self, idx: u8) -> &egui::TextureHandle;
}
pub mod asset_bytes {
    pub const GEMS: [&[u8]; 8];      // include_bytes! gem_0..7
    pub const EXACT: &[u8]; pub const PARTIAL: &[u8]; pub const UNKNOWN: &[u8];
}

// src/gui/palette.rs
pub const NAMES: [&str; 8] = ["红", "蓝", "紫", "橙", "黄", "绿", "青", "白"];
pub fn name(idx: u8) -> &'static str;
pub fn small_gem(ui: &mut egui::Ui, assets: &Assets, idx: u8);        // 32px 展示
pub fn big_gem(ui: &mut egui::Ui, assets: &Assets, idx: u8);          // 64px 展示
pub fn gem_button(ui: &mut egui::Ui, assets: &Assets, idx: u8) -> egui::Response; // 32px 可点,悬停显示颜色名
pub fn icon_count(ui: &mut egui::Ui, assets: &Assets, exact: bool, n: u8); // 图标×N 计数
```

- [ ] **Step 1: 写失败测试(素材有效性,无头可跑)**

`src/gui/assets.rs`:

```rust
//! 嵌入素材装载(§5.2):include_bytes! → image 解码 → ColorImage 纹理。

use eframe::egui;

pub mod asset_bytes {
    /// 同名替换文件后重新编译即生效,无需改代码。
    pub const GEMS: [&[u8]; 8] = [
        include_bytes!("../../assets/gems/gem_0.png"),
        include_bytes!("../../assets/gems/gem_1.png"),
        include_bytes!("../../assets/gems/gem_2.png"),
        include_bytes!("../../assets/gems/gem_3.png"),
        include_bytes!("../../assets/gems/gem_4.png"),
        include_bytes!("../../assets/gems/gem_5.png"),
        include_bytes!("../../assets/gems/gem_6.png"),
        include_bytes!("../../assets/gems/gem_7.png"),
    ];
    pub const EXACT: &[u8] = include_bytes!("../../assets/icons/exact.png");
    pub const PARTIAL: &[u8] = include_bytes!("../../assets/icons/partial.png");
    pub const UNKNOWN: &[u8] = include_bytes!("../../assets/icons/unknown.png");
}

pub struct Assets {
    pub gems: Vec<egui::TextureHandle>,
    pub exact: egui::TextureHandle,
    pub partial: egui::TextureHandle,
    pub unknown: egui::TextureHandle,
}

fn tex(ctx: &egui::Context, name: &str, bytes: &[u8]) -> egui::TextureHandle {
    let img = image::load_from_memory(bytes).expect("内置素材解码失败");
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba),
        egui::TextureOptions::LINEAR,
    )
}

impl Assets {
    pub fn load(ctx: &egui::Context) -> Self {
        let mut gems = Vec::with_capacity(8);
        for (i, b) in asset_bytes::GEMS.iter().enumerate() {
            gems.push(tex(ctx, &format!("gem_{i}"), b));
        }
        Self {
            gems,
            exact: tex(ctx, "exact", asset_bytes::EXACT),
            partial: tex(ctx, "partial", asset_bytes::PARTIAL),
            unknown: tex(ctx, "unknown", asset_bytes::UNKNOWN),
        }
    }

    pub fn gem(&self, idx: u8) -> &egui::TextureHandle {
        &self.gems[idx as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::asset_bytes;

    fn png_dims(b: &[u8]) -> (u32, u32) {
        assert_eq!(&b[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "PNG 签名");
        let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
        let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
        (w, h)
    }

    #[test]
    fn embedded_assets_decode_with_expected_dims() {
        // 同时验证 Task 9 手写 PNG 编码器产物可被真正解码
        for (i, b) in asset_bytes::GEMS.iter().enumerate() {
            assert_eq!(png_dims(b), (128, 128), "gem_{i} 尺寸");
            image::load_from_memory(b).expect("gem 解码失败");
        }
        for (name, b) in [
            ("exact", asset_bytes::EXACT),
            ("partial", asset_bytes::PARTIAL),
            ("unknown", asset_bytes::UNKNOWN),
        ] {
            assert_eq!(png_dims(b), (64, 64), "{name} 尺寸");
            image::load_from_memory(b).expect("{name} 解码失败");
        }
    }
}
```

- [ ] **Step 2: 运行测试确认通过**

Run: `cargo test assets`
Expected: 1 个测试 PASS(这一步同时兜底验证了 Task 9 的 PNG 编码正确性;若失败,回头修 `examples/gen_assets.rs` 的编码器而不是改此测试)

- [ ] **Step 3: 写调色板并接入 App**

`src/gui/palette.rs`:

```rust
//! 颜色索引 → 名称/素材映射(§5.2)。颜色语义只存在于这一层。

use eframe::egui;

use crate::gui::assets::Assets;

pub const NAMES: [&str; 8] = ["红", "蓝", "紫", "橙", "黄", "绿", "青", "白"];

pub fn name(idx: u8) -> &'static str {
    NAMES[idx as usize]
}

pub fn small_gem(ui: &mut egui::Ui, assets: &Assets, idx: u8) {
    ui.image(egui::load::SizedTexture::new(assets.gem(idx).id(), [32.0, 32.0]));
}

pub fn big_gem(ui: &mut egui::Ui, assets: &Assets, idx: u8) {
    ui.image(egui::load::SizedTexture::new(assets.gem(idx).id(), [64.0, 64.0]));
}

pub fn gem_button(ui: &mut egui::Ui, assets: &Assets, idx: u8) -> egui::Response {
    let sized = egui::load::SizedTexture::new(assets.gem(idx).id(), [32.0, 32.0]);
    ui.add(egui::Button::image(sized)).on_hover_text(name(idx))
}

pub fn icon_count(ui: &mut egui::Ui, assets: &Assets, exact: bool, n: u8) {
    let tex = if exact { &assets.exact } else { &assets.partial };
    ui.image(egui::load::SizedTexture::new(tex.id(), [20.0, 20.0]));
    ui.label(format!("×{n}"));
}
```

`src/gui/mod.rs` 修改:
- 顶部模块声明处加 re-export:`pub use assets::Assets;`(Task 12-14 的面板都从 `gemsleuth::gui::Assets` 引用)
- `GemsleuthApp` 增加字段 `pub assets: Assets`,`new()` 中 `assets: Assets::load(&cc.egui_ctx)`(在字体安装之后加载)
- 占位区把候选数字行替换为素材冒烟(任务 12 会移进正式面板):

```rust
            ui.horizontal(|ui| {
                ui.label("素材冒烟:");
                for i in 0..self.session.settings.colors as u8 {
                    palette::small_gem(ui, &self.assets, i);
                }
                palette::icon_count(ui, &self.assets, true, 2);
                palette::icon_count(ui, &self.assets, false, 1);
                ui.image(egui::load::SizedTexture::new(self.assets.unknown.id(), [32.0, 32.0]));
            });
```

- [ ] **Step 4: 构建 + 启动验收**

Run: `cargo build && cargo run`
Expected: 编译通过;窗口出现「素材冒烟」一行:默认 6 色圆点 + 蓝点×2 + 金点×1 + 灰色问号位图。

- [ ] **Step 5: Commit**

```bash
git add src/gui/assets.rs src/gui/palette.rs src/gui/mod.rs
git commit -m "gui: 素材内嵌装载与调色板(含解码有效性测试)"
```

---

### Task 12: 记录面板(gui/records_panel.rs)

**Files:**
- Modify: `src/gui/records_panel.rs`(替换占位注释)
- Modify: `src/gui/mod.rs`(接入面板,移除 Task 11 的素材冒烟行;App 增加 `editor` 字段)

**Interfaces:**
- Consumes: `Assets`/palette(Task 11)、`Record::validate`(Task 2)、`SessionState`/`dirty`(Task 10)
- Produces(Task 13/14 消费同一渲染入口):

```rust
pub struct RecordEditor {
    pub slots: Vec<u8>,   // 已填宝石(按槽位序)
    pub exact: u8,
    pub partial: u8,
    pub editing: Option<usize>, // Some(i) = 修改第 i 条
}
impl RecordEditor { fn reset(&mut self); } // Default + 清空

/// 记录区(两 Tab 共享,§5.1)。任何变更置 dirty。
pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    dirty: &mut bool,
    suspects: &[usize],
);
```

- [ ] **Step 1: 实现面板**

`src/gui/records_panel.rs`:

```rust
//! 记录录入/编辑区(§5.1,两 Tab 共享):增、删、改、启用/禁用、嫌疑标记。

use eframe::egui;

use gemsleuth::Record;
use gemsleuth::gui::{palette, Assets, SessionState};

#[derive(Default)]
pub struct RecordEditor {
    pub slots: Vec<u8>,
    pub exact: u8,
    pub partial: u8,
    pub editing: Option<usize>,
}

impl RecordEditor {
    pub fn reset(&mut self) {
        self.slots.clear();
        self.exact = 0;
        self.partial = 0;
        self.editing = None;
    }
}

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    dirty: &mut bool,
    suspects: &[usize],
) {
    ui.heading("记录");
    if session.records.is_empty() {
        ui.label("(暂无记录,请在下方新增)");
    }
    let mut delete = None;
    for i in 0..session.records.len() {
        let guess = session.records[i].guess.clone();
        let (exact, partial) = (session.records[i].exact, session.records[i].partial);
        ui.horizontal(|ui| {
            ui.label(format!("{}.", i + 1));
            if !session.records[i].enabled {
                ui.set_enabled(false); // 禁用行整体变灰(F5 排查用)
            }
            for &g in &guess {
                palette::small_gem(ui, assets, g);
            }
            palette::icon_count(ui, assets, true, exact);
            palette::icon_count(ui, assets, false, partial);
            if suspects.contains(&i) {
                ui.colored_label(egui::Color32::RED, "嫌疑");
            }
        });
        // 启用/编辑/删除单独一行外右侧(避免与变灰冲突)
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut session.records[i].enabled, "启用")
                .changed()
            {
                *dirty = true;
            }
            if ui.small_button("编辑").clicked() {
                editor.slots = guess;
                editor.exact = exact;
                editor.partial = partial;
                editor.editing = Some(i);
            }
            if ui.small_button("删除").clicked() {
                delete = Some(i);
            }
        });
    }
    if let Some(i) = delete {
        session.records.remove(i);
        if editor.editing == Some(i) {
            editor.reset();
        }
        *dirty = true;
    }

    ui.separator();
    ui.label(if editor.editing.is_some() { "修改记录" } else { "新增记录" });

    // 槽位显示:点击已填槽位 = 清空该槽及之后
    ui.horizontal(|ui| {
        for slot in 0..session.settings.slots {
            if let Some(&g) = editor.slots.get(slot) {
                if palette::gem_button(ui, assets, g).clicked() {
                    editor.slots.truncate(slot);
                }
            } else {
                ui.image(egui::load::SizedTexture::new(assets.unknown.id(), [32.0, 32.0]));
            }
        }
    });
    // 点击色盘依次填入空槽
    ui.horizontal(|ui| {
        ui.label("点击填入:");
        for color in 0..session.settings.colors as u8 {
            if palette::gem_button(ui, assets, color).clicked()
                && editor.slots.len() < session.settings.slots
            {
                editor.slots.push(color);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("蓝标(位置和种类都对)");
        ui.add(
            egui::DragValue::new(&mut editor.exact)
                .range(0..=session.settings.slots as u8),
        );
        ui.label("金标(种类对位置错)");
        ui.add(
            egui::DragValue::new(&mut editor.partial)
                .range(0..=session.settings.slots as u8),
        );
    });

    let candidate = Record {
        guess: editor.slots.clone(),
        exact: editor.exact,
        partial: editor.partial,
        enabled: true,
    };
    let valid_and_full = editor.slots.len() == session.settings.slots
        && candidate.validate(&session.settings).is_ok();
    if valid_and_full {
        ui.horizontal(|ui| {
            if let Some(i) = editor.editing {
                if ui.button("保存修改").clicked() {
                    session.records[i] = candidate;
                    editor.reset();
                    *dirty = true;
                }
                if ui.button("取消").clicked() {
                    editor.reset();
                }
            } else if ui.button("添加记录").clicked() {
                session.records.push(candidate);
                editor.reset();
                *dirty = true;
            }
        });
    } else if editor.slots.len() == session.settings.slots {
        // 已填满但计数不合法 → 红字提示,不给添加(§4.5)
        ui.colored_label(
            egui::Color32::RED,
            format!("计数不合法:{}", candidate.validate(&session.settings).unwrap_err()),
        );
    } else {
        ui.label("(请填满所有槽位)");
    }
}
```

`src/gui/mod.rs` 接入:
- App 增加字段 `pub editor: RecordEditor`(Default)
- 移除 Task 11 的「素材冒烟」行,替换为:

```rust
            ui.separator();
            records_panel::show(
                ui,
                &self.assets,
                &mut self.session,
                &mut self.editor,
                &mut self.dirty,
                &self.cached.suspects,
            );
            ui.separator();
            // Task 13/14 接入结果区,暂以占位标签过渡
            ui.label(format!("实时候选数:{}", self.cached.candidates.len()));
```

- [ ] **Step 2: 构建 + 手动验收**

Run: `cargo build && cargo run`

按金标准谜面前两条操作验收:
1. 点色盘红/蓝/紫/橙 → 4 个槽位依次填充;点第 2 个槽位 → 该槽及之后清空
2. 蓝标设 1、金标 0 → 「添加记录」→ 行 1 显示 4 宝石图 + 蓝×1 + 金×0
3. 蓝标拖到 5(超限)→ 红字「计数不合法」且无添加按钮
4. 录入橙蓝紫绿(1,0) → 实时候选数变为 24
5. 「编辑」某行 → 编辑器载入,改计数「保存修改」生效;「删除」生效
6. 取消勾选「启用」→ 该行变灰,实时候选数回升(禁用不参与过滤)

- [ ] **Step 3: Commit**

```bash
git add src/gui/records_panel.rs src/gui/mod.rs
git commit -m "gui: 记录面板(槽位录入/校验/增删改/启用禁用/嫌疑标记)"
```

---

### Task 13: 整卷求解面板(gui/solve_panel.rs)

**Files:**
- Modify: `src/gui/solve_panel.rs`(替换占位注释)
- Modify: `src/gui/mod.rs`(接入 Solve Tab;共享的推荐渲染函数 `recommendation_row` 放 `gui/mod.rs`)

**Interfaces:**
- Consumes: `SolveOutcome`/`suspects`(Task 8)、palette/Assets(Task 11)、`CachedAnalysis`(Task 10)
- Produces(Task 14 复用):

```rust
// gui/mod.rs 中新增(两结果面板共用)
pub fn recommendation_row(ui: &mut egui::Ui, assets: &Assets, rec: &Recommendation);
// 渲染:推荐下一猜 + 大宝石图 + Bound 标注(「最多还需 N 步」/「期望信息量…最坏桶…」)

// gui/solve_panel.rs
pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &SessionState,
    outcome: &mut Option<SolveOutcome>, // 点击求解时的快照
    suspects: &[usize],
);
```

- [ ] **Step 1: 实现共享推荐行 + 求解面板**

`src/gui/mod.rs` 追加(Task 10 已导入 `Recommendation`,这里只补 `Bound`,避免重复导入):

```rust
use gemsleuth::Bound;

/// 推荐猜测展示行(两结果面板共用,§5.1)。
pub fn recommendation_row(ui: &mut egui::Ui, assets: &Assets, rec: &Recommendation) {
    ui.horizontal(|ui| {
        ui.label("推荐下一猜:");
        match rec {
            Recommendation::Answer(ans) => {
                for &g in ans {
                    palette::big_gem(ui, assets, g);
                }
            }
            Recommendation::Guess { guess, bound } => {
                for &g in guess {
                    palette::big_gem(ui, assets, g);
                }
                match bound {
                    Bound::GuaranteedSteps(n) => {
                        ui.label(format!("(精确前瞻:最多还需 {n} 步)"));
                    }
                    Bound::Expected { entropy_bits, worst_bucket } => {
                        ui.label(format!(
                            "(熵推荐:期望信息量 {entropy_bits:.2} 比特,最坏情况剩 {worst_bucket} 个)"
                        ));
                    }
                }
            }
        }
    });
}
```

`src/gui/solve_panel.rs`:

```rust
//! 整卷求解结果区(§5.1):[求解] 按钮 + 三分支展示。

use eframe::egui;

use gemsleuth::{gui::palette, gui::recommendation_row, Assets, SessionState, SolveOutcome};

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &SessionState,
    outcome: &mut Option<SolveOutcome>,
    suspects: &[usize],
) {
    ui.heading("整卷求解");
    if ui.button("求解").clicked() {
        *outcome = Some(gemsleuth::solve(&session.settings, &session.records));
    }
    let Some(oc) = outcome else {
        ui.label("录入全部记录后点击「求解」");
        return;
    };
    match oc {
        SolveOutcome::Unique(ans) => {
            ui.label("唯一答案:");
            ui.horizontal(|ui| {
                for &g in ans {
                    palette::big_gem(ui, assets, g);
                }
            });
        }
        SolveOutcome::Contradiction => {
            ui.colored_label(egui::Color32::RED, "记录矛盾!请检查录入,可逐条禁用定位。");
            if !suspects.is_empty() {
                let list = suspects.iter().map(|i| i + 1).collect::<Vec<_>>().join("、");
                ui.label(format!("嫌疑记录:第 {list} 条(禁用后候选恢复非空)"));
            }
        }
        SolveOutcome::Ambiguous { candidates, recommendation } => {
            ui.label(format!("共 {} 个候选:", candidates.len()));
            if candidates.len() > 50 {
                ui.label("(超过 50 个,折叠为计数;继续录入记录或按推荐消歧)");
            } else {
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for c in candidates {
                            ui.horizontal(|ui| {
                                for &g in c {
                                    palette::small_gem(ui, assets, g);
                                }
                            });
                        }
                    });
            }
            recommendation_row(ui, assets, recommendation);
        }
    }
}
```

`src/gui/mod.rs` 的 `ui()` 中,把占位的实时候选行替换为按 Tab 分发:

```rust
            match self.tab {
                Tab::Solve => solve_panel::show(
                    ui,
                    &self.assets,
                    &self.session,
                    &mut self.solve_outcome,
                    &self.cached.suspects,
                ),
                Tab::Assistant => {
                    ui.label("陪玩助手面板在 Task 14 接入");
                }
            }
```

- [ ] **Step 2: 构建 + 手动验收(金标准谜面全流程)**

Run: `cargo build && cargo run`

1. 录入附录 A 全部 6 条金标准记录 → 切「整卷求解」→ 点「求解」→ **唯一答案:紫紫紫黄(大图标)**
2. 删掉第 6 条(紫黄紫紫 2,2)再求解 → 多候选 + 折叠提示或列表 + 推荐行
3. 添加一条 红蓝紫橙→蓝4金0(矛盾)→ 求解 → 红色矛盾提示 + 「嫌疑记录:第 X 条」
4. 禁用嫌疑行 → 再求解 → 恢复多候选展示

- [ ] **Step 3: Commit**

```bash
git add src/gui/solve_panel.rs src/gui/mod.rs
git commit -m "gui: 整卷求解面板(三分支+嫌疑提示+折叠候选)"
```

---

### Task 14: 陪玩助手面板(gui/assistant_panel.rs)

**Files:**
- Modify: `src/gui/assistant_panel.rs`(替换占位注释)
- Modify: `src/gui/mod.rs`(接入 Assistant Tab)

**Interfaces:**
- Consumes: `CachedAnalysis`(Task 10,含 `candidates`/`recommendation`/`suspects`)、`recommendation_row`(Task 13)、palette(Task 11)
- Produces: 无(最终消费方)

```rust
pub fn show(ui: &mut egui::Ui, assets: &Assets, cached: &CachedAnalysis);
```

- [ ] **Step 1: 实现面板**

`src/gui/assistant_panel.rs`:

```rust
//! 陪玩助手结果区(§5.1):逐条增长记录,实时(无需按钮)显示候选与推荐。

use eframe::egui;

use gemsleuth::gui::{palette, recommendation_row, Assets, CachedAnalysis};

pub fn show(ui: &mut egui::Ui, assets: &Assets, cached: &CachedAnalysis) {
    ui.heading("陪玩助手");
    match cached.candidates.len() {
        0 => {
            ui.colored_label(egui::Color32::RED, "记录矛盾!剩余候选为 0,请检查录入。");
            if !cached.suspects.is_empty() {
                let list = cached.suspects.iter().map(|i| i + 1).collect::<Vec<_>>().join("、");
                ui.label(format!("嫌疑记录:第 {list} 条(禁用后候选恢复非空)"));
            }
        }
        1 => {
            ui.label("答案确定:");
            ui.horizontal(|ui| {
                for &g in &cached.candidates[0] {
                    palette::big_gem(ui, assets, g);
                }
            });
        }
        n => {
            ui.label(format!("剩余候选 {n} 个"));
            if let Some(rec) = &cached.recommendation {
                recommendation_row(ui, assets, rec);
            }
            if n > 50 {
                ui.label("(候选超过 50 个,折叠显示)");
            } else {
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for c in &cached.candidates {
                        ui.horizontal(|ui| {
                            for &g in c {
                                palette::small_gem(ui, assets, g);
                            }
                        });
                    }
                });
            }
        }
    }
}
```

`src/gui/mod.rs` 中替换 Assistant 占位:

```rust
                Tab::Assistant => assistant_panel::show(ui, &self.assets, &self.cached),
```

- [ ] **Step 2: 构建 + 手动验收**

Run: `cargo build && cargo run`

陪玩流程(答案以 紫紫紫黄 为真值,自查反馈):
1. 初始:剩余候选 1296 个 + 推荐红蓝紫橙(标注期望信息量≈3.06 比特/最坏 312)——**无需任何按钮,随录入实时变化**
2. 按推荐猜一次并录入游戏反馈(如 红蓝紫橙→(1,0))→ 候选数立刻下降
3. 录入橙蓝紫绿(1,0)后 → 剩余 24 个 + 推荐 红红黄黄(精确前瞻:最多还需 3 步)
4. 继续录入直到 剩余 2 个 → 推荐其中之一(最多还需 2 步);再录一条 → 答案确定(大图标)
5. 录入一条矛盾反馈 → 红色矛盾 + 嫌疑标记,禁用嫌疑行后实时恢复

性能观察:每次改动后界面应在可感知的瞬时(<1s,默认参数毫秒级)刷新。

- [ ] **Step 3: Commit**

```bash
git add src/gui/assistant_panel.rs src/gui/mod.rs
git commit -m "gui: 陪玩助手面板(实时候选/分层推荐标注/矛盾排查)"
```

---

### Task 15: 终验与交付

**Files:**
- Modify: 无(只验证;若验收发现小问题,修复后追加提交)

**Interfaces:**
- Consumes: 全部
- Produces: `target/release/gemsleuth.exe` 单文件交付物

- [ ] **Step 1: 全量测试(release 下也跑一遍,覆盖 8⁶ 大空间的正确性/性能)**

Run: `cargo test`
Expected: 全部通过(40 个:核心 39 + 素材 1)

Run: `cargo test --release`
Expected: 全部通过,总耗时应明显短于 debug。

- [ ] **Step 2: Release 构建 + 冒烟**

Run: `cargo build --release`
Expected: 产出 `target/release/gemsleuth.exe`(单文件,无 dll 依赖)。

Run: `./target/release/gemsleuth.exe`(人工确认可启动后关闭)

- [ ] **Step 3: 手动验收总清单(规格 §6 GUI 手动验收)**

逐项确认(全部满足才算完成):

1. 窗口标题「Gemsleuth 宝石推理求解器」,约 1000×700,可缩放;中文全部正常渲染
2. **整卷求解**:录入附录 A 金标准 6 条 → 求解 → 唯一答案 紫紫紫黄
3. **陪玩助手**:逐条录入时实时候选数从 1296 单调收敛;推荐与标注随分层切换(期望比特 ↔ 最多还需 N 步)
4. **矛盾排查**:录入矛盾记录 → 红色提示 + 嫌疑行标记;禁用嫌疑行 → 实时恢复
5. 记录增/删/改/禁用全部生效;计数非法(蓝+金>槽位)被拒绝且红字提示
6. 设置修改 → 确认弹窗 → 确定后记录清空、取消不生效;「允许重复」关闭时槽位数选项收窄
7. 候选 >50 折叠为计数
8. 素材:8 色宝石与 3 个图标正常显示(替换任一同名 PNG 后重编译生效)
9. 两 Tab 共享同一份记录(在整卷 Tab 录入,切到助手 Tab 数据一致)

- [ ] **Step 4: 最终提交**

```bash
git add -A
git commit -m "终验:全量测试与 release 单文件交付"
```

(若验收单有未过项,逐项修复并提交后再回到 Step 1。)

---

## 附录 A:金标准测试数据(全部经穷举脚本验证)

颜色索引:`0红 1蓝 2紫 3橙 4黄 5绿 6青 7白`。唯一解:`[2,2,2,4]`(紫紫紫黄)。

| # | 记录(guess → exact,partial) | 来源 |
|---|---|---|
| 1 | `[3,1,2,0]` → (1,0) 橙蓝紫红 | **真实谜面**(规格 §5.1) |
| 2 | `[3,1,2,5]` → (1,0) 橙蓝紫绿 | **真实谜面**(规格 §5.1) |
| 3 | `[0,1,2,3]` → (1,0) 红蓝紫橙 | 构造补充 |
| 4 | `[0,0,1,1]` → (0,0) 红红蓝蓝 | 构造补充 |
| 5 | `[4,5,0,1]` → (0,1) 黄绿红蓝 | 构造补充 |
| 6 | `[2,4,2,2]` → (2,2) 紫黄紫紫 | 构造补充 |

**用户提供其余真实记录后**:直接替换上表 3-6 行(断言不变,仍应唯一解出 紫紫紫黄)。相关测试:`solver::tests::golden_puzzle_solves_unique`、`judge::tests::real_puzzle_records`(后者只用前 2 条真实记录,不会受影响)。

## 附录 B:已验证的期望值常量(写死在各测试中,均有穷举脚本背书)

| 场景 | 期望值 |
|---|---|
| 两条真实记录后候选数 | 24(首个候选 `[1,1,1,1]`) |
| 6⁴ 全空间熵推荐开局 | `[0,1,2,3]`,熵 3.056671 bit,最坏桶 312 |
| 24 候选下熵最佳(白盒) | `[3,4,4,1]`,熵 3.173533 bit,最坏桶 5 |
| 24 候选精确前瞻(cap 3) | `[0,0,4,4]`(红红黄黄),保证 3 步 |
| 3 候选 `{0000,0001,1111}` 前瞻 | `[0,0,0,0]`,保证 2 步;cap=0 → None |
| 5 条记录得 2 候选 `{2224,4222}` | 推荐 `[2,2,2,4]`,保证 2 步 |
| 空间规模 | 6⁴=1296、8⁶=262144、P(6,4)=360、P(8,4)=1680、4⁴=256 |
| 采样 | n≥空间 → 全量;固定种子两次调用结果相同;8⁶ 采 2048 互异 |

## 附录 C:执行注意事项

- **任务 1-8 的全部代码与测试期望值已在 eframe 0.36.1 / rustc 1.98.1 环境下端到端跑通(39/39 绿)**,照抄即可;若测试失败优先怀疑抄录偏差而非期望值。
- Windows + Git Bash 环境;全量 `cargo test`(debug)约 1 分钟,耗时以前瞻与终局模拟测试为主,属正常。
- crate 内统一 `crate::core::...` 路径,避免与内建 `core` crate 歧义。
- 每个任务收尾 `cargo test` 全绿再提交;GUI 任务(10-14)以 `cargo build` + 手动清单代替自动化测试(规格 §2.2)。
- 8⁶(8 色 6 槽)极端配置的推荐计算约秒级(采样 2048 已保证),规格接受;不要试图在测试里跑 8⁶ 全量推荐。
