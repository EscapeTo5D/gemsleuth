# gemsleuth 宝石推理求解器 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一个 Rust + egui 桌面工具，为"宝石推理"谜题提供整卷求解与陪玩助手双模式。

**Architecture:** 单 crate 双 target：`lib`（`core` 模块，纯求解逻辑、零 UI 依赖、可无头测试）+ `bin`（`gui` 模块，eframe/egui 薄壳）。两模式共享同一 `SessionState`（settings + records），区别仅是结果区呈现。

**Tech Stack:** Rust edition 2024（rustc 1.94.1）、eframe/egui、egui_extras（image loader + `include_image!`）。

**Spec:** `docs/superpowers/specs/2026-09-08-gemsleuth-design.md`（本计划从规格出发，规格随行；执行者须先读规格）。

## Global Constraints

- 工作目录：`D:\hong_projects\gemsleuth`（已 git init，`main` 分支）。
- 仅两个第三方依赖：`eframe`、`egui_extras`（features=["image"]）；**解析后必须锁精确版本** `=x.y.z`；禁用其他 crate（采样用内置 SplitMix64，不用 rand）。
- 参数：颜色数 4..=8（默认 6）；槽位数 3..=6（默认 4）；允许重复默认 `true`；`repeats=false` 时要求 `slots ≤ colors`。
- 判定语义（标准 Mastermind 多重集计数）：`exact = Σ[guess[i]==cand[i]]`；`partial = Σ_色 min(猜测计数, 候选计数) − exact`。
- 策略常量：`FULL_SPACE_LIMIT = 20_000`；`SAMPLE_SIZE = 2048`；SplitMix64 黄金增量 `0x9E3779B97F4A7C15`（常数，勿作种子）、采样种子 `0x0123_4567_89AB_CDEF`；`LOOKAHEAD_LIMIT = 30`；前瞻深度上限 `3`。
- 调色板固定映射：`0红 1蓝 2紫 3橙 4黄 5绿 6青 7白`。
- 素材路径：`assets/gems/gem_0.png … gem_7.png`、`assets/icons/exact.png|partial.png|unknown.png`；编译期 `include_image!` 嵌入，exe 单文件。
- UI 全中文；`lib` 零 UI 依赖（gui 挂在 bin 侧：`main.rs` 内 `mod gui;`）。
- 金标准：6 条记录（见 Task 4）必须唯一解 `vec![2,2,2,4]`（紫紫紫黄）。
- 每个 core 任务结束跑 `cargo test --lib`；GUI 任务结束 `cargo build` + 手动运行核对；每任务一提交。

---

### Task 1: 工程脚手架与构建基线

**Files:**
- Create: `Cargo.toml`（cargo init 生成后改写）、`.gitignore`、`src/lib.rs`、`src/core/mod.rs`、`src/main.rs`

**Interfaces:**
- Produces: 可编译的 lib+bin 骨架；`gemsleuth::core` 空模块；最小 eframe 窗口（后续任务替换其内容）。

- [ ] **Step 1: 初始化工程**

```bash
cd /d/hong_projects/gemsleuth
cargo init --name gemsleuth --vcs none
```

- [ ] **Step 2: 写 Cargo.toml / .gitignore**

`Cargo.toml`（版本号先占位，Step 3 用 cargo add 填入后再锁死）：

```toml
[package]
name = "gemsleuth"
version = "0.1.0"
edition = "2024"

[lib]
name = "gemsleuth"
path = "src/lib.rs"

[[bin]]
name = "gemsleuth"
path = "src/main.rs"

[dependencies]
```

`.gitignore`:

```
/target
```

- [ ] **Step 3: 添加并锁定依赖**

```bash
cargo add eframe
cargo add egui_extras --features image
```

然后把 `Cargo.toml` 中两个依赖改成解析到的精确版本（`cargo add` 写入的 `0.x.y` 前加 `=`，如 `eframe = "=0.33.0"`——以实际解析为准），并在本任务内不再变动。

- [ ] **Step 4: 写 lib/bin 最小骨架**

`src/lib.rs`:

```rust
//! gemsleuth 求解核心：与 UI 无关的纯函数层。

pub mod core;
```

`src/core/mod.rs`:

```rust
//! 求解核心子模块集合（judge / solver / strategy / model 在后续任务加入）。
```

`src/main.rs`:

```rust
fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("宝石推理求解器 gemsleuth"),
        ..Default::default()
    };
    eframe::run_native(
        "gemsleuth",
        opts,
        Box::new(|_cc| Ok(Box::new(ShellApp))),
    )
}

struct ShellApp;

impl eframe::App for ShellApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("gemsleuth 启动成功");
        });
    }
}
```

注：若解析到的 eframe 版本 `run_native` 闭包签名不同（如不要求 `Ok(...)` 包装），按编译器提示做最小适配，不改变结构。

- [ ] **Step 5: 构建并运行验证**

```bash
cargo build
cargo run   # 窗口出现"gemsleuth 启动成功"，手动关闭
```

Expected: 编译零 error；窗口正常出现。

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: 工程脚手架（lib+bin 双 target，eframe 最小窗口）"
```

---

### Task 2: core::model 数据模型与校验

**Files:**
- Create: `src/core/model.rs`
- Modify: `src/core/mod.rs`（追加 `pub mod model;`）

**Interfaces:**
- Produces（后续任务依赖的精确签名）:

```rust
pub const MIN_COLORS: usize = 4;
pub const MAX_COLORS: usize = 8;
pub const MIN_SLOTS: usize = 3;
pub const MAX_SLOTS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings { pub colors: usize, pub slots: usize, pub repeats: bool }
impl Default for Settings { /* 6, 4, true */ }
impl Settings {
    pub fn is_valid(&self) -> bool;
    pub fn space_size(&self) -> usize;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record { pub guess: Vec<u8>, pub exact: u8, pub partial: u8, pub enabled: bool }
impl Record { pub fn validate(&self, settings: &Settings) -> Result<(), &'static str>; }

pub type Feedback = (u8, u8); // (exact, partial)
```

- [ ] **Step 1: 写测试与 todo!() 桩（RED）**

`src/core/model.rs` 先写以下内容（类型定义完整，三个 impl 方法体 `todo!()`），并写入 `#[cfg(test)] mod tests`；`src/core/mod.rs` 追加 `pub mod model;`。

类型与桩：

```rust
//! 数据模型：Settings / Record / Feedback。

pub const MIN_COLORS: usize = 4;
pub const MAX_COLORS: usize = 8;
pub const MIN_SLOTS: usize = 3;
pub const MAX_SLOTS: usize = 6;

pub type Feedback = (u8, u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub colors: usize,
    pub slots: usize,
    pub repeats: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub guess: Vec<u8>,
    pub exact: u8,
    pub partial: u8,
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self { todo!() }
}

impl Settings {
    pub fn is_valid(&self) -> bool { todo!() }
    /// 候选全空间大小：repeats=true → colors^slots；否则 P(colors, slots)。
    pub fn space_size(&self) -> usize { todo!() }
}

impl Record {
    pub fn validate(&self, settings: &Settings) -> Result<(), &'static str> { todo!() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_align_with_game() {
        let s = Settings::default();
        assert_eq!(s, Settings { colors: 6, slots: 4, repeats: true });
    }

    #[test]
    fn is_valid_rules() {
        assert!(Settings { colors: 6, slots: 4, repeats: true }.is_valid());
        assert!(Settings { colors: 4, slots: 3, repeats: true }.is_valid());
        assert!(Settings { colors: 8, slots: 6, repeats: true }.is_valid());
        // 越界
        assert!(!Settings { colors: 3, slots: 4, repeats: true }.is_valid());
        assert!(!Settings { colors: 6, slots: 7, repeats: true }.is_valid());
        // 不允许重复时槽位不得超过颜色数
        assert!(!Settings { colors: 4, slots: 5, repeats: false }.is_valid());
        assert!(Settings { colors: 4, slots: 4, repeats: false }.is_valid());
    }

    #[test]
    fn space_sizes() {
        assert_eq!(Settings { colors: 6, slots: 4, repeats: true }.space_size(), 1296);
        assert_eq!(Settings { colors: 8, slots: 6, repeats: true }.space_size(), 262_144);
        // P(6,4) = 6*5*4*3
        assert_eq!(Settings { colors: 6, slots: 4, repeats: false }.space_size(), 360);
    }

    #[test]
    fn record_validation() {
        let s = Settings::default();
        let ok = Record { guess: vec![0, 1, 2, 3], exact: 1, partial: 2, enabled: true };
        assert_eq!(ok.validate(&s), Ok(()));
        // 长度不符
        let bad_len = Record { guess: vec![0, 1, 2], exact: 0, partial: 0, enabled: true };
        assert!(bad_len.validate(&s).is_err());
        // 颜色索引越界
        let bad_color = Record { guess: vec![0, 1, 2, 6], exact: 0, partial: 0, enabled: true };
        assert!(bad_color.validate(&s).is_err());
        // exact+partial 超过槽位数
        let bad_sum = Record { guess: vec![0, 1, 2, 3], exact: 3, partial: 2, enabled: true };
        assert!(bad_sum.validate(&s).is_err());
    }
}
```

- [ ] **Step 2: 运行确认 RED**

Run: `cargo test --lib`
Expected: 编译可通过但所有测试 panic（`todo!()`），即 "not yet implemented" FAIL。

- [ ] **Step 3: 实现三个方法（GREEN）**

把 `todo!()` 替换为：

```rust
impl Default for Settings {
    fn default() -> Self {
        Self { colors: 6, slots: 4, repeats: true }
    }
}

impl Settings {
    pub fn is_valid(&self) -> bool {
        (MIN_COLORS..=MAX_COLORS).contains(&self.colors)
            && (MIN_SLOTS..=MAX_SLOTS).contains(&self.slots)
            && (self.repeats || self.slots <= self.colors)
    }

    pub fn space_size(&self) -> usize {
        if self.repeats {
            self.colors.pow(self.slots as u32)
        } else {
            (0..self.slots).fold(1usize, |acc, i| acc * (self.colors - i))
        }
    }
}

impl Record {
    pub fn validate(&self, settings: &Settings) -> Result<(), &'static str> {
        if self.guess.len() != settings.slots {
            return Err("猜测长度与槽位数不符");
        }
        if self.guess.iter().any(|&c| c as usize >= settings.colors) {
            return Err("宝石索引超出颜色数");
        }
        let slots = settings.slots as u8;
        if self.exact > slots || self.partial > slots {
            return Err("计数超过槽位数");
        }
        if self.exact + self.partial > slots {
            return Err("蓝标数+金标数超过槽位数");
        }
        Ok(())
    }
}
```

- [ ] **Step 4: 运行确认 GREEN**

Run: `cargo test --lib`
Expected: 全部 PASS。

- [ ] **Step 5: Commit**

```bash
git add src/core/model.rs src/core/mod.rs
git commit -m "feat(core): Settings/Record 数据模型与校验"
```

---

### Task 3: core::judge 反馈判定

**Files:**
- Create: `src/core/judge.rs`
- Modify: `src/core/mod.rs`（追加 `pub mod judge;`）

**Interfaces:**
- Consumes: `super::model::Feedback`
- Produces: `pub fn judge(guess: &[u8], candidate: &[u8]) -> Feedback;`

- [ ] **Step 1: 写测试与 todo!() 桩（RED）**

`src/core/judge.rs`:

```rust
//! 反馈判定：标准 Mastermind 多重集计数。

use super::model::Feedback;

pub fn judge(guess: &[u8], candidate: &[u8]) -> Feedback { todo!() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_exact() {
        assert_eq!(judge(&[2, 2, 2, 4], &[2, 2, 2, 4]), (4, 0));
    }

    #[test]
    fn exact_only_with_repeat_background() {
        // 记录①语义：橙蓝紫红 vs 紫紫紫黄 → 1 位置对、0 错位
        assert_eq!(judge(&[3, 1, 2, 0], &[2, 2, 2, 4]), (1, 0));
    }

    #[test]
    fn repeats_counted_on_min_side() {
        // 记录④语义：黄蓝黄紫 vs 紫紫紫黄 → (0, 2)
        // 黄在猜测出现 2 次、答案只有 1 个 → 错位黄只算 1；紫同理算 1
        assert_eq!(judge(&[4, 1, 4, 2], &[2, 2, 2, 4]), (0, 2));
    }

    #[test]
    fn all_partial() {
        assert_eq!(judge(&[0, 1, 2, 3], &[3, 2, 1, 0]), (0, 4));
    }

    #[test]
    fn mixed_exact_and_partial() {
        // 记录⑤语义：红黄紫蓝 vs 紫紫紫黄 → (1, 1)
        assert_eq!(judge(&[0, 4, 2, 1], &[2, 2, 2, 4]), (1, 1));
    }
}
```

- [ ] **Step 2: 运行确认 RED**

Run: `cargo test --lib core::judge`
Expected: 5 个测试全部 panic FAIL。

- [ ] **Step 3: 实现（GREEN）**

```rust
pub fn judge(guess: &[u8], candidate: &[u8]) -> Feedback {
    debug_assert_eq!(guess.len(), candidate.len());
    let exact = guess.iter().zip(candidate).filter(|(g, c)| g == c).count() as u8;
    let mut gc = [0u8; 256];
    let mut cc = [0u8; 256];
    for &g in guess { gc[g as usize] += 1; }
    for &c in candidate { cc[c as usize] += 1; }
    let overlap: u32 = (0..256).map(|i| gc[i].min(cc[i]) as u32).sum();
    (exact, overlap as u8 - exact)
}
```

- [ ] **Step 4: 运行确认 GREEN**

Run: `cargo test --lib`
Expected: 全部 PASS。

- [ ] **Step 5: Commit**

```bash
git add src/core/judge.rs src/core/mod.rs
git commit -m "feat(core): judge 多重集反馈判定（含重复宝石语义）"
```

---

### Task 4: core::solver 枚举 / 过滤 / 求解 / 嫌疑排查

**Files:**
- Create: `src/core/solver.rs`
- Modify: `src/core/mod.rs`（追加 `pub mod solver;`）

**Interfaces:**
- Consumes: `model::{Settings, Record, Feedback}`、`super::judge::judge`、`super::strategy::{recommend, Recommendation}`（Task 5 交付；本任务先以 `todo!()` 桩引用其签名——若 Task 5 未执行，本任务最后一步编译不通过属预期，须在 Task 5 完成后回归）
- Produces:

```rust
pub enum SolveOutcome {
    Unique(Vec<u8>),
    Ambiguous { candidates: Vec<Vec<u8>>, recommendation: crate::core::strategy::Recommendation },
    Contradiction,
}
pub fn enumerate_space(settings: &Settings) -> Vec<Vec<u8>>;
pub fn filter_candidates(settings: &Settings, records: &[Record]) -> Vec<Vec<u8>>; // 仅 enabled
pub fn solve(settings: &Settings, records: &[Record]) -> SolveOutcome;
pub fn find_suspects(settings: &Settings, records: &[Record]) -> Vec<usize>;      // 0 基索引
```

> 执行顺序说明：推荐先做 Task 5（strategy 的熵部分），再回归本任务的 `solve` 编译。Task 4/5 互为依赖，建议按 4→5→4 回归或 5→4 顺序执行，两任务全部完成后才允许进入 Task 6。

- [ ] **Step 1: 写测试与 todo!() 桩（RED）**

`src/core/solver.rs`（枚举/过滤/求解的桩全部 `todo!()`；测试含金标准）：

```rust
//! 全空间枚举 + 记录过滤 + 求解 + 矛盾嫌疑排查。

use super::model::{Record, Settings};
use super::strategy::Recommendation;

#[derive(Debug, PartialEq)]
pub enum SolveOutcome {
    Unique(Vec<u8>),
    Ambiguous { candidates: Vec<Vec<u8>>, recommendation: Recommendation },
    Contradiction,
}

pub fn enumerate_space(settings: &Settings) -> Vec<Vec<u8>> { todo!() }

pub fn filter_candidates(settings: &Settings, records: &[Record]) -> Vec<Vec<u8>> { todo!() }

pub fn solve(settings: &Settings, records: &[Record]) -> SolveOutcome { todo!() }

pub fn find_suspects(settings: &Settings, records: &[Record]) -> Vec<usize> { todo!() }

/// 测试辅助：字母序列 → 宝石索引（R0 B1 P2 O3 Y4 G5）。
#[cfg(test)]
fn gems(s: &str) -> Vec<u8> {
    s.bytes().map(|b| match b {
        b'R' => 0, b'B' => 1, b'P' => 2, b'O' => 3, b'Y' => 4, b'G' => 5,
        _ => panic!("未知宝石字母 {b}"),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::strategy::StrategyKind;

    fn rec(s: &str, exact: u8, partial: u8) -> Record {
        Record { guess: gems(s), exact, partial, enabled: true }
    }

    /// 2026-09-08 真机谜题的 6 条记录（金标准）。
    fn golden() -> Vec<Record> {
        vec![rec("OBPR", 1, 0), rec("OBPG", 1, 0), rec("POYB", 1, 1),
             rec("YBYP", 0, 2), rec("RYPB", 1, 1), rec("OGYY", 1, 0)]
    }

    #[test]
    fn space_enumeration() {
        let r = Settings { colors: 6, slots: 4, repeats: true };
        assert_eq!(enumerate_space(&r).len(), 1296);
        let p = Settings { colors: 4, slots: 3, repeats: false };
        assert_eq!(enumerate_space(&p).len(), 24);
        // 排列模式无重复元素
        assert!(enumerate_space(&p).iter().all(|c| {
            let mut v = c.clone(); v.sort(); v.dedup(); v.len() == c.len()
        }));
    }

    #[test]
    fn zero_records_yields_full_space() {
        let s = Settings::default();
        let outcome = solve(&s, &[]);
        match outcome {
            SolveOutcome::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), 1296),
            other => panic!("应为 Ambiguous，实际 {other:?}"),
        }
    }

    #[test]
    fn golden_puzzle_unique_answer() {
        let s = Settings::default();
        match solve(&s, &golden()) {
            SolveOutcome::Unique(ans) => assert_eq!(ans, vec![2, 2, 2, 4]), // 紫紫紫黄
            other => panic!("金标准应唯一解，实际 {other:?}"),
        }
    }

    #[test]
    fn contradictory_records_detected() {
        let s = Settings::default();
        let mut records = golden();
        records.push(rec("RRRR", 1, 0)); // 与金标准矛盾
        assert_eq!(solve(&s, &records), SolveOutcome::Contradiction);
    }

    #[test]
    fn disabling_suspect_restores_solution() {
        let s = Settings::default();
        let mut records = golden();
        records.push(rec("RRRR", 1, 0));
        assert_eq!(find_suspects(&s, &records), vec![6]);
        records[6].enabled = false;
        match solve(&s, &records) {
            SolveOutcome::Unique(ans) => assert_eq!(ans, vec![2, 2, 2, 4]),
            other => panic!("禁用嫌疑记录后应恢复唯一解，实际 {other:?}"),
        }
    }

    #[test]
    fn ambiguous_when_few_records() {
        let s = Settings::default();
        match solve(&s, &[rec("YBYP", 0, 2)]) {
            SolveOutcome::Ambiguous { candidates, recommendation } => {
                assert!(candidates.len() > 1 && candidates.len() < 1296);
                assert_eq!(recommendation.kind, StrategyKind::Entropy);
            }
            other => panic!("应为 Ambiguous，实际 {other:?}"),
        }
    }

    #[test]
    fn permutation_mode_exact_only() {
        let p = Settings { colors: 4, slots: 3, repeats: false };
        // RBP = [0,1,2]；(3,0) 表示三位全对 → 唯一解 [0,1,2]
        match solve(&p, &[rec("RBP", 3, 0)]) {
            SolveOutcome::Unique(ans) => assert_eq!(ans, vec![0, 1, 2]),
            other => panic!("应为唯一解，实际 {other:?}"),
        }
    }

    #[test]
    fn permutation_mode_contradiction_when_impossible() {
        let p = Settings { colors: 4, slots: 3, repeats: false };
        // (0,0)：答案不含 R/B/P → 只剩颜色 3 一种，无法组成 3 个不同宝石
        assert_eq!(solve(&p, &[rec("RBP", 0, 0)]), SolveOutcome::Contradiction);
    }
}
```

- [ ] **Step 2: 运行确认 RED**

Run: `cargo test --lib core::solver`
Expected: panic FAIL（todo!()）。若因 `strategy::recommend` 未就绪而编译失败，先完成 Task 5 的 Step 1–3 再回归。

- [ ] **Step 3: 实现（GREEN）**

```rust
pub fn enumerate_space(settings: &Settings) -> Vec<Vec<u8>> {
    let (colors, slots) = (settings.colors, settings.slots);
    let mut out = Vec::with_capacity(settings.space_size());
    if settings.repeats {
        let mut cur = Vec::with_capacity(slots);
        fn go_repeats(colors: usize, slots: usize, cur: &mut Vec<u8>, out: &mut Vec<Vec<u8>>) {
            if cur.len() == slots { out.push(cur.clone()); return; }
            for c in 0..colors as u8 { cur.push(c); go_repeats(colors, slots, cur, out); cur.pop(); }
        }
        go_repeats(colors, slots, &mut cur, &mut out);
    } else {
        let mut used = vec![false; colors];
        let mut cur = Vec::with_capacity(slots);
        fn go_perms(colors: usize, slots: usize, used: &mut [bool], cur: &mut Vec<u8>, out: &mut Vec<Vec<u8>>) {
            if cur.len() == slots { out.push(cur.clone()); return; }
            for c in 0..colors {
                if !used[c] {
                    used[c] = true; cur.push(c as u8);
                    go_perms(colors, slots, used, cur, out);
                    cur.pop(); used[c] = false;
                }
            }
        }
        go_perms(colors, slots, &mut used, &mut cur, &mut out);
    }
    out
}

pub fn filter_candidates(settings: &Settings, records: &[Record]) -> Vec<Vec<u8>> {
    let active: Vec<&Record> = records.iter().filter(|r| r.enabled).collect();
    enumerate_space(settings)
        .into_iter()
        .filter(|cand| active.iter().all(|r| judge(&r.guess, cand) == (r.exact, r.partial)))
        .collect()
}

pub fn solve(settings: &Settings, records: &[Record]) -> SolveOutcome {
    let candidates = filter_candidates(settings, records);
    match candidates.len() {
        0 => SolveOutcome::Contradiction,
        1 => SolveOutcome::Unique(candidates.into_iter().next().unwrap()),
        _ => {
            let recommendation = recommend(settings, &candidates);
            SolveOutcome::Ambiguous { candidates, recommendation }
        }
    }
}

pub fn find_suspects(settings: &Settings, records: &[Record]) -> Vec<usize> {
    let mut suspects = Vec::new();
    for i in 0..records.len() {
        if !records[i].enabled { continue; }
        let mut probe = records.to_vec();
        probe[i].enabled = false;
        if !matches!(solve(settings, &probe), SolveOutcome::Contradiction) {
            suspects.push(i);
        }
    }
    suspects
}
```

文件头 `use` 补齐：

```rust
use super::judge::judge;
use super::strategy::recommend;
```

- [ ] **Step 4: 运行确认 GREEN（须在 Task 5 之后回归）**

Run: `cargo test --lib`
Expected: 全部 PASS（含金标准 `golden_puzzle_unique_answer`）。

- [ ] **Step 5: Commit**

```bash
git add src/core/solver.rs src/core/mod.rs
git commit -m "feat(core): 全空间枚举/过滤/求解/矛盾嫌疑排查（含金标准测试：紫紫紫黄）"
```

---

### Task 5: core::strategy 熵推荐 + 固定种子采样

**Files:**
- Create: `src/core/strategy.rs`
- Modify: `src/core/mod.rs`（追加 `pub mod strategy;`）

**Interfaces:**
- Consumes: `model::Settings`、`judge::judge`、`solver::enumerate_space`（仅取全空间作为 guess 池；solver 亦调用本模块，Rust 同 crate 模块互调无环问题）
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrategyKind { Answer, CandidatePick, Lookahead, Entropy }

#[derive(Clone, Debug)]
pub struct Recommendation {
    pub guess: Vec<u8>,
    pub kind: StrategyKind,
    pub guaranteed_steps: Option<u32>, // Some(N)：最多还需 N 步（含本次猜测）
    pub worst_bucket: usize,           // 该猜测的最坏反馈桶大小
    pub expected_remaining: f64,       // 该猜测的期望剩余候选数
}

pub fn recommend(settings: &Settings, candidates: &[Vec<u8>]) -> Recommendation;
pub(crate) const FULL_SPACE_LIMIT: usize = 20_000;
pub(crate) const SAMPLE_SIZE: usize = 2048;
pub(crate) fn sample_guesses(space: &[Vec<u8>], n: usize) -> Vec<Vec<u8>>;
pub(crate) fn entropy_pick(guesses: &[Vec<u8>], candidates: &[Vec<u8>]) -> (Vec<u8>, usize, f64);
```

（Task 6 将在本模块追加 `LOOKAHEAD_LIMIT`/`LOOKAHEAD_DEPTH` 与 `lookahead`，并改写 `recommend` 插入前瞻层。）

- [ ] **Step 1: 写测试与 todo!() 桩（RED）**

```rust
//! 分层推荐策略：=1 答案 / =2 二选一 / ≤30 精确前瞻（Task 6）/ 其余熵最大化。

use super::model::Settings;
use super::solver::enumerate_space;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrategyKind { Answer, CandidatePick, Lookahead, Entropy }

#[derive(Clone, Debug)]
pub struct Recommendation {
    pub guess: Vec<u8>,
    pub kind: StrategyKind,
    pub guaranteed_steps: Option<u32>,
    pub worst_bucket: usize,
    pub expected_remaining: f64,
}

pub(crate) const FULL_SPACE_LIMIT: usize = 20_000;
pub(crate) const SAMPLE_SIZE: usize = 2048;

pub fn recommend(settings: &Settings, candidates: &[Vec<u8>]) -> Recommendation { todo!() }

pub(crate) fn sample_guesses(space: &[Vec<u8>], n: usize) -> Vec<Vec<u8>> { todo!() }

pub(crate) fn entropy_pick(guesses: &[Vec<u8>], candidates: &[Vec<u8>]) -> (Vec<u8>, usize, f64) { todo!() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_candidate_is_answer() {
        let s = Settings::default();
        let r = recommend(&s, &[vec![2, 2, 2, 4]]);
        assert_eq!(r.kind, StrategyKind::Answer);
        assert_eq!(r.guess, vec![2, 2, 2, 4]);
        assert_eq!(r.guaranteed_steps, Some(1));
    }

    #[test]
    fn two_candidates_pick_one() {
        let s = Settings::default();
        let cands = vec![vec![2, 2, 2, 4], vec![0, 0, 0, 0]];
        let r = recommend(&s, &cands);
        assert_eq!(r.kind, StrategyKind::CandidatePick);
        assert_eq!(r.guaranteed_steps, Some(2));
        assert!(cands.contains(&r.guess));
    }

    #[test]
    fn entropy_on_full_space_is_deterministic_and_useful() {
        let s = Settings::default();
        let space = enumerate_space(&s);
        let r1 = recommend(&s, &space.clone());
        let r2 = recommend(&s, &space);
        assert_eq!(r1, r2, "同输入必须同输出（固定种子采样）");
        assert_eq!(r1.kind, StrategyKind::Entropy);
        assert_eq!(r1.guess.len(), 4);
        assert!(r1.worst_bucket < 1296 && r1.expected_remaining < 1296.0);
        // 用该推荐对隐藏答案过滤后，候选集必须缩小
        let hidden = vec![2, 2, 2, 4];
        let fb = crate::core::judge::judge(&r1.guess, &hidden);
        let remaining: Vec<_> = space.into_iter()
            .filter(|c| crate::core::judge::judge(&r1.guess, c) == fb).collect();
        assert!(remaining.len() < 1296, "推荐猜测必须带来信息增益");
    }

    #[test]
    fn sample_is_subset_and_deduped() {
        let space: Vec<Vec<u8>> = (0..3000u32).map(|i| vec![i as u8 % 6, (i / 6) as u8 % 6, (i / 36) as u8 % 6, (i / 216) as u8 % 6]).collect();
        let picked = sample_guesses(&space, SAMPLE_SIZE);
        assert_eq!(picked.len(), SAMPLE_SIZE);
        let uniq: std::collections::BTreeSet<_> = picked.iter().collect();
        assert_eq!(uniq.len(), picked.len());
        assert!(picked.iter().all(|g| space.contains(g)));
    }

    #[test]
    fn sample_returns_all_when_space_small() {
        let space = vec![vec![0, 1], vec![1, 2], vec![2, 0]];
        let picked = sample_guesses(&space, SAMPLE_SIZE);
        assert_eq!(picked.len(), 3);
    }
}
```

- [ ] **Step 2: 运行确认 RED**

Run: `cargo test --lib core::strategy`
Expected: 全部 panic FAIL。

- [ ] **Step 3: 实现（GREEN）**

```rust
pub fn recommend(settings: &Settings, candidates: &[Vec<u8>]) -> Recommendation {
    match candidates.len() {
        1 => Recommendation {
            guess: candidates[0].clone(),
            kind: StrategyKind::Answer,
            guaranteed_steps: Some(1),
            worst_bucket: 1,
            expected_remaining: 0.0,
        },
        2 => Recommendation {
            guess: candidates[0].clone(),
            kind: StrategyKind::CandidatePick,
            guaranteed_steps: Some(2),
            worst_bucket: 1,
            expected_remaining: 1.0,
        },
        _ => {
            let space = enumerate_space(settings);
            let guesses = if space.len() <= FULL_SPACE_LIMIT {
                space
            } else {
                sample_guesses(&space, SAMPLE_SIZE)
            };
            let (guess, worst_bucket, expected_remaining) = entropy_pick(&guesses, candidates);
            Recommendation {
                guess,
                kind: StrategyKind::Entropy,
                guaranteed_steps: None,
                worst_bucket,
                expected_remaining,
            }
        }
    }
}

```rust
pub(crate) fn entropy_pick(guesses: &[Vec<u8>], candidates: &[Vec<u8>]) -> (Vec<u8>, usize, f64) {
    use std::collections::HashMap;
    let n = candidates.len() as f64;
    let candidate_set: std::collections::BTreeSet<&[u8]> =
        candidates.iter().map(|c| c.as_slice()).collect();
    let mut best: Option<(f64, i32, f64, Vec<u8>, usize)> = None; // (entropy, in_cand, -expected, guess, worst)
    for g in guesses {
        let mut buckets: HashMap<super::model::Feedback, usize> = HashMap::new();
        for c in candidates { *buckets.entry(judge(g, c)).or_default() += 1; }
        let worst_bucket = buckets.values().copied().max().unwrap_or(0);
        let expected_remaining: f64 =
            buckets.values().map(|&b| b as f64 * (b as f64 / n)).sum();
        let entropy: f64 = buckets.values()
            .map(|&b| { let p = b as f64 / n; -p * p.log2() })
            .sum();
        let in_cand = candidate_set.contains(g.as_slice()) as i32;
        let key = (entropy, in_cand, -expected_remaining);
        if best.as_ref().is_none_or(|b| key > (b.0, b.1, b.2)) {
            best = Some((entropy, in_cand, -expected_remaining, g.clone(), worst_bucket));
        }
    }
    let (_, _, neg_exp, guess, worst) = best.expect("guesses 非空");
    (guess, worst, -neg_exp)
}
```

（头部补 `use super::judge::judge;`）

```rust
pub(crate) fn sample_guesses(space: &[Vec<u8>], n: usize) -> Vec<Vec<u8>> {
    if space.len() <= n { return space.to_vec(); }
    struct SplitMix64(u64);
    impl SplitMix64 {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
    }
    let mut rng = SplitMix64(0x0123_4567_89AB_CDEF);
    let mut idx = std::collections::BTreeSet::new();
    while idx.len() < n { idx.insert((rng.next() as usize) % space.len()); }
    idx.into_iter().map(|i| space[i].clone()).collect()
}
```

注：种子取 `0x0123_4567_89AB_CDEF`（Global Constraints 中的 `0x9E37…` 是 SplitMix64 的黄金增量常数，勿混用；两处都写进代码注释）。

- [ ] **Step 4: 运行确认 GREEN**

Run: `cargo test --lib`
Expected: 全部 PASS（此时 Task 4 若已写好仍处 todo——放行顺序：先 5 后 4，或 4 先写了就一起 GREEN）。

- [ ] **Step 5: Commit**

```bash
git add src/core/strategy.rs src/core/mod.rs
git commit -m "feat(core): 熵最大化推荐 + 固定种子采样 + 分层框架（=1/=2/熵）"
```

---

### Task 6: core::strategy 精确前瞻（保证最少剩余步数）

**Files:**
- Modify: `src/core/strategy.rs`（追加常量、`lookahead`、改写 `recommend` 插入前瞻层）

**Interfaces:**
- Consumes: `judge`、`Recommendation`
- Produces（新增/变更）:

```rust
pub(crate) const LOOKAHEAD_LIMIT: usize = 30;
pub(crate) const LOOKAHEAD_DEPTH: u32 = 3;
pub(crate) fn lookahead(candidates: &[Vec<u8>], budget: u32) -> Option<(u32, Vec<u8>)>;
// recommend：candidates.len() == 2 与 > LOOKAHEAD_LIMIT 之间插入：
//   ≤ LOOKAHEAD_LIMIT → 尝试 lookahead；Some((steps, guess)) → kind=Lookahead, guaranteed_steps=Some(steps)
//                       None → 回退 Entropy（并重算该 guess 的 worst/expected）
```

设计要点（写进代码注释）：**前瞻 guess 池 = 候选集本身**。任一候选猜测对候选集的反馈划分是真严格子划分（被猜中的那个候选独占 `(slots,0)` 桶），故递归必然终止、无需记忆化；规模 ≤30×30×30 次 judge，毫秒级。`lookahead` 返回的步数**包含**本次猜测（1 个候选 → 1 步：直接猜它）。

- [ ] **Step 1: 写测试（RED）**

在 `src/core/strategy.rs` 的 `mod tests` 追加：

```rust
    #[test]
    fn lookahead_two_candidates_is_two_steps() {
        let cands = vec![vec![0, 1], vec![2, 3]];
        assert_eq!(lookahead(&cands, 3), Some((2, vec![0, 1])));
    }

    #[test]
    fn lookahead_budget_zero_fails() {
        let cands = vec![vec![0, 1], vec![2, 3]];
        assert_eq!(lookahead(&cands, 0), None);
    }

    #[test]
    fn recommend_uses_lookahead_for_small_sets() {
        let s = Settings { colors: 3, slots: 2, repeats: true }; // 全空间 9
        let space = enumerate_space(&s);
        let r = recommend(&s, &space);
        assert_eq!(r.kind, StrategyKind::Lookahead);
        let bound = r.guaranteed_steps.expect("前瞻层必须给出保证步数");
        assert!((2..=3).contains(&bound));
        // 性质：对任意隐藏答案，沿每步重算的推荐提交猜测，含终局提交在内的总次数 ≤ 最初保证。
        // （bound 语义 = lookahead 的 N：单候选叶子计 1 次提交，故总提交 = 非终局步数 + 1）
        for hidden in &space {
            let mut cands = space.clone();
            let mut steps = 0u32;
            while cands.len() > 1 {
                let rec = recommend(&s, &cands);
                let fb = crate::core::judge::judge(&rec.guess, hidden);
                cands.retain(|c| crate::core::judge::judge(&rec.guess, c) == fb);
                steps += 1;
            }
            assert_eq!(cands[0], *hidden);
            assert!(steps + 1 <= bound, "hidden={hidden:?}：共 {} 次提交超过保证 {bound}", steps + 1);
        }
    }

    #[test]
    fn small_candidate_set_uses_lookahead_even_in_big_space() {
        // 候选数（而非全空间大小）决定分层：3 候选 ≤ LOOKAHEAD_LIMIT → 前瞻；
        // 尽管全空间 8^6 = 262144 远超 FULL_SPACE_LIMIT（前瞻池 = 候选集，与空间大小无关）。
        let s = Settings { colors: 8, slots: 6, repeats: true };
        let cands = vec![vec![0, 1, 2, 3, 4, 5], vec![5, 4, 3, 2, 1, 0], vec![1, 2, 3, 4, 5, 6]];
        let r = recommend(&s, &cands);
        assert_eq!(r.kind, StrategyKind::Lookahead);
        // 保证步数推演：猜候选之一 c0 → c0 独占全对桶；c1、c2 对 c0 的反馈
        // 同桶（2 候选）→ 该桶还需 2 步 → 总 1+2=3；分桶（各 1）→ 总 1+1=2。故 bound ∈ {2,3}。
        let bound = r.guaranteed_steps.expect("前瞻必须给出保证");
        assert!(bound == 2 || bound == 3, "实际 bound = {bound:?}");
        for hidden in &cands {
            let mut pool = cands.clone();
            let mut steps = 0u32;
            while pool.len() > 1 {
                let rec = recommend(&s, &pool);
                let fb = crate::core::judge::judge(&rec.guess, hidden);
                pool.retain(|c| crate::core::judge::judge(&rec.guess, c) == fb);
                steps += 1;
            }
            assert_eq!(pool[0], *hidden);
            assert!(steps + 1 <= bound);
        }
    }
```

- [ ] **Step 2: 运行确认 RED**

Run: `cargo test --lib core::strategy`
Expected: 新增测试编译失败（`lookahead` 未定义）。

- [ ] **Step 3: 实现 lookahead 并接入 recommend（GREEN）**

```rust
pub(crate) const LOOKAHEAD_LIMIT: usize = 30;
pub(crate) const LOOKAHEAD_DEPTH: u32 = 3;

/// 在候选集内精确搜索"保证最少剩余步数"的猜测。
/// guess 池 = 候选集本身：候选猜测的反馈划分必为严格子划分（被猜中者独占全对桶），递归必终止。
/// 返回 Some((N, guess))：N 包含本次猜测；1 个候选 → N=1。
pub(crate) fn lookahead(candidates: &[Vec<u8>], budget: u32) -> Option<(u32, Vec<u8>)> {
    if candidates.is_empty() { return None; }
    if candidates.len() == 1 { return Some((1, candidates[0].clone())); }
    if budget == 0 { return None; }
    let mut best: Option<(u32, Vec<u8>)> = None;
    for g in candidates {
        // 按反馈分桶
        let mut buckets: Vec<Vec<Vec<u8>>> = Vec::new();
        'each_cand: for c in candidates {
            let fb = judge(g, c);
            for b in &mut buckets {
                if judge(g, &b[0]) == fb { b.push(c.clone()); continue 'each_cand; }
            }
            buckets.push(vec![c.clone()]);
        }
        // 最坏桶递归
        let mut worst: u32 = 0;
        for b in &buckets {
            if b.len() == candidates.len() { continue; } // 不可能发生（严格划分），防御
            match lookahead(b, budget - 1) {
                Some((n, _)) => worst = worst.max(n),
                None => { worst = u32::MAX; break; }
            }
        }
        if worst != u32::MAX {
            let total = 1 + worst;
            if best.as_ref().is_none_or(|&(bn, _)| total < bn) {
                best = Some((total, g.clone()));
            }
        }
    }
    best
}
```

`recommend` 的多候选分支改为：

```rust
        n if n <= LOOKAHEAD_LIMIT => {
            if let Some((steps, guess)) = lookahead(candidates, LOOKAHEAD_DEPTH) {
                let mut buckets: std::collections::HashMap<super::model::Feedback, usize> =
                    std::collections::HashMap::new();
                for c in candidates { *buckets.entry(judge(&guess, c)).or_default() += 1; }
                let worst_bucket = buckets.values().copied().max().unwrap_or(0);
                let count = candidates.len() as f64;
                let expected_remaining: f64 =
                    buckets.values().map(|&b| b as f64 * (b as f64 / count)).sum();
                Recommendation {
                    guess,
                    kind: StrategyKind::Lookahead,
                    guaranteed_steps: Some(steps),
                    worst_bucket,
                    expected_remaining,
                }
            } else {
                // 理论上 ≤30 候选、深度 3 内必有解；防御性回退熵层
                let space = enumerate_space(settings);
                let guesses = if space.len() <= FULL_SPACE_LIMIT { space }
                              else { sample_guesses(&space, SAMPLE_SIZE) };
                let (guess, worst_bucket, expected_remaining) = entropy_pick(&guesses, candidates);
                Recommendation {
                    guess, kind: StrategyKind::Entropy,
                    guaranteed_steps: None, worst_bucket, expected_remaining,
                }
            }
        }
        _ => { /* 原熵分支不动 */ }
```

- [ ] **Step 4: 运行确认 GREEN 并核对保证步数**

Run: `cargo test --lib`
Expected: 全部 PASS。同时人工核对 `lookahead_two_candidates_is_two_steps`、3 候选返回值与推演一致（把推演写进 `small_candidate_set_uses_lookahead_even_in_big_space` 的注释）。

- [ ] **Step 5: Commit**

```bash
git add src/core/strategy.rs
git commit -m "feat(core): 精确前瞻层（≤30 候选保证最少剩余步数）+ 分层接入"
```

---

### Task 7: GUI 外壳（中文字体 / 设置栏 / Tab / 会话状态）

**Files:**
- Rewrite: `src/main.rs`
- Create: `src/gui/mod.rs`

**Interfaces:**
- Consumes: `gemsleuth::core::model::{Settings, Record, MIN_COLORS, MAX_COLORS, MIN_SLOTS, MAX_SLOTS}`
- Produces（后续 GUI 任务依赖）:

```rust
// src/gui/mod.rs
pub mod assets;      // Task 8
pub mod palette;     // Task 8
pub mod records_panel; // Task 9
pub mod solve_panel;   // Task 10
pub mod assistant_panel; // Task 11
// （本任务先只建 mod.rs 本体 + 注释掉未建模块的声明，随任务逐个放开）

pub struct SessionState { pub settings: Settings, pub records: Vec<Record>, pub editor: RecordEditor }
pub struct RecordEditor { pub guess: Vec<Option<u8>>, pub exact: u8, pub partial: u8, pub editing: Option<usize> }
impl RecordEditor { pub fn new(slots: usize) -> Self; pub fn reset(&mut self, slots: usize); }
pub struct GemSleuthApp { pub state: SessionState, pub tab: Tab, pub pending_settings: Option<Settings> }
pub enum Tab { Solve, Assistant }
pub fn install_cjk_fonts(ctx: &egui::Context) -> bool;
pub fn recommendation_card(ui: &mut egui::Ui, rec: &Recommendation, colors: usize); // Task 8 有素材后实现
```

- [ ] **Step 1: 写 gui/mod.rs 与 main.rs**

`src/gui/mod.rs`（本任务版本：模块声明中 assets/palette/records_panel/solve_panel/assistant_panel 五行先注释，Task 8–11 逐个放开；recommendation_card 本任务不写）：

```rust
//! GUI 层：egui 薄壳。核心逻辑一律调 gemsleuth::core，不在此实现。

use eframe::egui;
use gemsleuth::core::model::{Record, Settings, MIN_COLORS, MAX_COLORS, MIN_SLOTS, MAX_SLOTS};

// pub mod assets;        // Task 8
// pub mod palette;       // Task 8
// pub mod records_panel; // Task 9
// pub mod solve_panel;   // Task 10
// pub mod assistant_panel; // Task 11

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab { Solve, Assistant }

pub struct RecordEditor {
    pub guess: Vec<Option<u8>>,
    pub exact: u8,
    pub partial: u8,
    pub editing: Option<usize>,
}

impl RecordEditor {
    pub fn new(slots: usize) -> Self {
        Self { guess: vec![None; slots], exact: 0, partial: 0, editing: None }
    }
    pub fn reset(&mut self, slots: usize) {
        self.guess = vec![None; slots];
        self.exact = 0;
        self.partial = 0;
        self.editing = None;
    }
}

pub struct SessionState {
    pub settings: Settings,
    pub records: Vec<Record>,
    pub editor: RecordEditor,
}

pub struct GemSleuthApp {
    pub state: SessionState,
    pub tab: Tab,
    pub pending_settings: Option<Settings>,
}

impl GemSleuthApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_cjk_fonts(&cc.egui_ctx);
        let settings = Settings::default();
        Self {
            state: SessionState { settings, records: Vec::new(), editor: RecordEditor::new(settings.slots) },
            tab: Tab::Solve,
            pending_settings: None,
        }
    }

    fn try_change_settings(&mut self, s: Settings) {
        if s == self.state.settings || !s.is_valid() { return; }
        if self.state.records.is_empty() {
            let slots = s.slots;
            self.state.settings = s;
            self.state.editor.reset(slots);
        } else {
            self.pending_settings = Some(s);
        }
    }

    fn settings_bar(&mut self, ui: &mut egui::Ui) {
        let current = self.state.settings;
        let mut chosen = current;
        ui.horizontal(|ui| {
            ui.label("颜色数");
            egui::ComboBox::from_id_salt("colors")
                .selected_text(format!("{}", chosen.colors))
                .show_ui(ui, |ui| {
                    for c in MIN_COLORS..=MAX_COLORS {
                        ui.selectable_value(&mut chosen.colors, c, format!("{c}"));
                    }
                });
            ui.label("槽位数");
            egui::ComboBox::from_id_salt("slots")
                .selected_text(format!("{}", chosen.slots))
                .show_ui(ui, |ui| {
                    for s in MIN_SLOTS..=MAX_SLOTS {
                        ui.selectable_value(&mut chosen.slots, s, format!("{s}"));
                    }
                });
            ui.checkbox(&mut chosen.repeats, "允许重复");
            ui.separator();
            if ui.button("重置会话").clicked() {
                self.state.records.clear();
                self.state.editor.reset(self.state.settings.slots);
            }
        });
        if !chosen.is_valid() {
            ui.colored_label(egui::Color32::RED, "不允许重复时，槽位数不得超过颜色数（当前组合不可用）");
        }
        if chosen != current { self.try_change_settings(chosen); }
    }

    fn confirm_modal(&mut self, ctx: &egui::Context) {
        if let Some(new_settings) = self.pending_settings {
            egui::Modal::new(egui::Id::new("reset_confirm")).show(ctx, |ui| {
                ui.label("修改参数将清空当前记录，确定？");
                ui.horizontal(|ui| {
                    if ui.button("确定").clicked() {
                        let slots = new_settings.slots;
                        self.state.settings = new_settings;
                        self.state.records.clear();
                        self.state.editor.reset(slots);
                        self.pending_settings = None;
                        ui.close();
                    }
                    if ui.button("取消").clicked() {
                        self.pending_settings = None;
                        ui.close();
                    }
                });
            });
        }
    }
}

impl eframe::App for GemSleuthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("settings_bar").show(ctx, |ui| self.settings_bar(ui));
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Solve, "整卷求解");
                ui.selectable_value(&mut self.tab, Tab::Assistant, "陪玩助手");
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.tab {
                Tab::Solve => { ui.label("（结果区：Task 10）"); }
                Tab::Assistant => { ui.label("（结果区：Task 11）"); }
            }
        });
        self.confirm_modal(ctx);
    }
}

/// 运行时加载系统中文字体（不分发字体文件）。成功返回 true。
pub fn install_cjk_fonts(ctx: &egui::Context) -> bool {
    const CANDIDATES: [&str; 4] = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "C:\\Windows\\Fonts\\Deng.ttf",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert("cjk".into(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                if let Some(list) = fonts.families.get_mut(&family) {
                    list.push("cjk".into());
                }
            }
            ctx.set_fonts(fonts);
            return true;
        }
    }
    false
}
```

注：若解析到的 egui 无 `egui::Modal`（0.32 起内置），用 `egui::Window::new("确认").collapsible(false)` 实现同一 `pending_settings` 确认逻辑。

`src/main.rs` 重写：

```rust
mod gui;

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("宝石推理求解器 gemsleuth"),
        ..Default::default()
    };
    eframe::run_native(
        "gemsleuth",
        opts,
        Box::new(|cc| Ok(Box::new(gui::GemSleuthApp::new(cc)))),
    )
}
```

- [ ] **Step 2: 构建并手动验证**

Run: `cargo build && cargo run`
Expected: 窗口出现，**中文无乱码**（字体生效）；顶部设置栏两个下拉框 + 复选框 + 重置按钮可用；Tab 可切换；有记录时改参数弹确认框、确定后记录清空（当前无记录功能，可目测设置变更即时生效）。

- [ ] **Step 3: Commit**

```bash
git add src/gui/mod.rs src/main.rs
git commit -m "feat(gui): 外壳（系统中文字体/设置栏/双Tab/会话状态/重置确认）"
```

---

### Task 8: 素材嵌入 + 调色板（占位图从截图裁切）

**Files:**
- Create: `tools/crop_assets.py`、`assets/gems/gem_0.png … gem_7.png`、`assets/icons/exact.png|partial.png|unknown.png`、`src/gui/assets.rs`、`src/gui/palette.rs`
- Modify: `src/gui/mod.rs`（放开 `pub mod assets; pub mod palette;`；`GemSleuthApp::new` 里 `egui_extras::install_image_loaders(&cc.egui_ctx, &[])`；update 里加一行素材自检展示）

**Interfaces:**
- Produces:

```rust
// src/gui/assets.rs
pub const GEMS: [egui::ImageSource; 8] = [ /* include_image! gem_0..7 */ ];
pub const ICON_EXACT: egui::ImageSource = egui::include_image!("../../assets/icons/exact.png");
pub const ICON_PARTIAL: egui::ImageSource = egui::include_image!("../../assets/icons/partial.png");
pub const ICON_UNKNOWN: egui::ImageSource = egui::include_image!("../../assets/icons/unknown.png");
pub fn gem(color: u8) -> egui::ImageSource;

// src/gui/palette.rs
pub const GEM_NAMES: [&str; 8] = ["红", "蓝", "紫", "橙", "黄", "绿", "青", "白"];
pub fn gem_name(color: u8) -> &'static str;
```

- [ ] **Step 1: 写裁图脚本并生成占位素材**

`tools/crop_assets.py`（源图为 2026-09-08 会话截图，路径常量；若该文件不存在则报错提示替换路径）：

```python
"""从游戏截图裁切占位素材（用户后续用高清图同名替换）。坐标基于 1107x719 截图。"""
from PIL import Image, ImageDraw
import sys

SRC = r"C:\Users\einstein\.zcode\cli\image-cache\sess_66d352b6-ab5a-414a-ba91-12b95a3c3af4\image-37a4e185dbba8091bdbe8d315ba1c868.png"
OUT = r"D:\hong_projects\gemsleuth\assets"

GEMS = {  # gem_index: (x0, y0, x1, y1) —— 备选行 6 颗
    0: (386, 127, 434, 175),  # 红
    1: (448, 127, 496, 175),  # 蓝
    2: (510, 127, 558, 175),  # 紫
    3: (572, 127, 620, 175),  # 橙
    4: (634, 127, 682, 175),  # 黄
    5: (696, 127, 744, 175),  # 绿
}
ICONS = {
    "exact":   (471, 270, 497, 296),  # 蓝标
    "partial": (470, 520, 496, 546),  # 金标
    "unknown": (502, 270, 528, 296),  # 问号
}

im = Image.open(SRC).convert("RGB")
for idx, box in GEMS.items():
    im.crop(box).save(rf"{OUT}\gems\gem_{idx}.png")
for name, box in ICONS.items():
    im.crop(box).save(rf"{OUT}\icons\{name}.png")

# 备用色 6青 / 7白：程序化占位（描边圆）
for idx, (fill, outline) in {6: ((41, 182, 246), (20, 60, 90)), 7: ((236, 236, 236), (90, 90, 90))}.items():
    img = Image.new("RGBA", (48, 48), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.ellipse((2, 2, 46, 46), fill=fill, outline=outline, width=3)
    img.save(rf"{OUT}\gems\gem_{idx}.png")
print("assets written")
```

运行：

```bash
mkdir -p assets/gems assets/icons
python tools/crop_assets.py
```

- [ ] **Step 2: 目视核对裁切结果（重要）**

生成 8x 放大校验图并逐张 Read 查看（或用任意看图方式），确认 6 颗宝石完整居中、无偏移；若偏移，调整 `GEMS`/`ICONS` 坐标重跑：

```bash
python - <<'EOF'
from PIL import Image
import glob
for p in glob.glob(r"D:\hong_projects\gemsleuth\assets\gems\*.png") + glob.glob(r"D:\hong_projects\gemsleuth\assets\icons\*.png"):
    im = Image.open(p); im.resize((im.width*8, im.height*8), Image.NEAREST).save(p + ".check.png")
EOF
```

Expected: 11 张素材全部可用（参考：本会话已用坐标 (471,270,497,296)/(470,520,496,546)/(502,270,528,296) 成功裁出三图标并存放于 `C:\Users\einstein\Downloads\`，可对照）。核对完删除 `*.check.png`。

- [ ] **Step 3: 写 assets.rs 与 palette.rs，接入 loader**

`src/gui/assets.rs`:

```rust
//! 编译期嵌入的素材（同名替换 assets/ 下文件后重新编译即生效）。

use eframe::egui;

pub const GEMS: [egui::ImageSource; 8] = [
    egui::include_image!("../../assets/gems/gem_0.png"),
    egui::include_image!("../../assets/gems/gem_1.png"),
    egui::include_image!("../../assets/gems/gem_2.png"),
    egui::include_image!("../../assets/gems/gem_3.png"),
    egui::include_image!("../../assets/gems/gem_4.png"),
    egui::include_image!("../../assets/gems/gem_5.png"),
    egui::include_image!("../../assets/gems/gem_6.png"),
    egui::include_image!("../../assets/gems/gem_7.png"),
];

pub const ICON_EXACT: egui::ImageSource = egui::include_image!("../../assets/icons/exact.png");
pub const ICON_PARTIAL: egui::ImageSource = egui::include_image!("../../assets/icons/partial.png");
pub const ICON_UNKNOWN: egui::ImageSource = egui::include_image!("../../assets/icons/unknown.png");

pub fn gem(color: u8) -> egui::ImageSource {
    GEMS[color as usize % GEMS.len()]
}
```

`src/gui/palette.rs`:

```rust
//! 颜色索引 → 中文名。

pub const GEM_NAMES: [&str; 8] = ["红", "蓝", "紫", "橙", "黄", "绿", "青", "白"];

pub fn gem_name(color: u8) -> &'static str {
    GEM_NAMES[color as usize % GEM_NAMES.len()]
}
```

`src/gui/mod.rs` 修改：放开 `pub mod assets; pub mod palette;`；`GemSleuthApp::new` 中加 `egui_extras::install_image_loaders(&cc.egui_ctx, &[]);`；`update` 的 CentralPanel 顶部临时加一行素材自检（本任务验证用，Task 9 移除）：

```rust
ui.horizontal(|ui| {
    for c in 0..8u8 { ui.add(egui::Image::new(assets::gem(c)).max_size(egui::vec2(36.0, 36.0))); }
    ui.separator();
    ui.add(egui::Image::new(assets::ICON_EXACT).max_size(egui::vec2(28.0, 28.0)));
    ui.add(egui::Image::new(assets::ICON_PARTIAL).max_size(egui::vec2(28.0, 28.0)));
    ui.add(egui::Image::new(assets::ICON_UNKNOWN).max_size(egui::vec2(28.0, 28.0)));
});
```

- [ ] **Step 4: 构建并手动验证**

Run: `cargo run`
Expected: 窗口顶部一行显示 8 颗宝石 + 3 个判定图标，图像清晰无拉伸。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(gui): 素材嵌入（截图裁切占位）+ 调色板命名"
```

---

### Task 9: 记录面板（增 / 删 / 改 / 禁用）

**Files:**
- Create: `src/gui/records_panel.rs`
- Modify: `src/gui/mod.rs`（放开 `pub mod records_panel;`；CentralPanel 里渲染记录区；删除 Task 8 的临时自检行）

**Interfaces:**
- Consumes: `assets`、`palette`、`SessionState`、`RecordEditor`、`gemsleuth::core::model::Record::validate`
- Produces: `pub fn ui_records(ui: &mut egui::Ui, state: &mut SessionState);`

- [ ] **Step 1: 实现 records_panel.rs**

```rust
//! 记录区（两 Tab 共享）：调色板 + 槽位编辑 + 记录列表（改/禁用/删）。

use eframe::egui;
use gemsleuth::core::model::Record;

use super::{assets, SessionState};

pub fn ui_records(ui: &mut egui::Ui, state: &mut SessionState) {
    let slots = state.settings.slots;
    let colors = state.settings.colors;

    // 调色板：点击填入第一个空槽
    ui.horizontal(|ui| {
        ui.label("点击宝石填入：");
        for c in 0..colors {
            let img = egui::Image::new(assets::gem(c as u8)).max_size(egui::vec2(34.0, 34.0));
            if ui.add(egui::Button::image(img)).clicked() {
                if let Some(slot) = state.editor.guess.iter().position(|g| g.is_none()) {
                    state.editor.guess[slot] = Some(c as u8);
                }
            }
        }
    });

    // 槽位 + 计数 + 提交
    ui.horizontal(|ui| {
        for i in 0..slots {
            let cell = |ui: &mut egui::Ui, src: egui::ImageSource| {
                ui.add(egui::Button::image(egui::Image::new(src).max_size(egui::vec2(40.0, 40.0))))
            };
            let resp = match state.editor.guess[i] {
                Some(c) => cell(ui, assets::gem(c)),
                None => cell(ui, assets::ICON_UNKNOWN),
            };
            if resp.clicked() { state.editor.guess[i] = None; }
        }
        ui.separator();
        ui.add(egui::Image::new(assets::ICON_EXACT).max_size(egui::vec2(24.0, 24.0)));
        ui.add(egui::DragValue::new(&mut state.editor.exact).range(0..=slots as u8));
        ui.add(egui::Image::new(assets::ICON_PARTIAL).max_size(egui::vec2(24.0, 24.0)));
        ui.add(egui::DragValue::new(&mut state.editor.partial).range(0..=slots as u8));
        ui.separator();
        let complete = state.editor.guess.iter().all(|g| g.is_some());
        let label = if state.editor.editing.is_some() { "更新该记录" } else { "添加记录" };
        if ui.add_enabled(complete, egui::Button::new(label)).clicked() {
            let guess: Vec<u8> = state.editor.guess.iter().map(|g| g.unwrap()).collect();
            let rec = Record { guess, exact: state.editor.exact, partial: state.editor.partial, enabled: true };
            if rec.validate(&state.settings).is_ok() {
                match state.editor.editing {
                    Some(i) if i < state.records.len() => state.records[i] = rec,
                    _ => state.records.push(rec),
                }
                state.editor.reset(slots);
            }
        }
        if ui.button("清空编辑").clicked() { state.editor.reset(slots); }
    });

    ui.separator();

    // 记录列表：编辑/删除动作先收集、循环外统一执行，规避借用冲突
    enum Action { Edit, Delete }
    let count = state.records.len();
    let mut actions: Vec<(usize, Action)> = Vec::new();
    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
        egui::Grid::new("records_grid").num_columns(1).show(ui, |ui| {
            for i in 0..count {
                ui.horizontal(|ui| {
                    let rec = &mut state.records[i];
                    ui.monospace(format!("{}.", i + 1));
                    for &c in &rec.guess {
                        ui.add(egui::Image::new(assets::gem(c)).max_size(egui::vec2(30.0, 30.0)));
                    }
                    ui.add(egui::Image::new(assets::ICON_EXACT).max_size(egui::vec2(20.0, 20.0)));
                    ui.monospace(rec.exact.to_string());
                    ui.add(egui::Image::new(assets::ICON_PARTIAL).max_size(egui::vec2(20.0, 20.0)));
                    ui.monospace(rec.partial.to_string());
                    let mut enabled = rec.enabled;
                    if ui.checkbox(&mut enabled, "").changed() {
                        rec.enabled = enabled;
                    }
                    if ui.button("✎").clicked() { actions.push((i, Action::Edit)); }
                    if ui.button("🗑").clicked() { actions.push((i, Action::Delete)); }
                });
                ui.end_row();
            }
        });
    });
    for (i, action) in actions {
        match action {
            Action::Delete => {
                state.records.remove(i);
                if state.editor.editing == Some(i) { state.editor.reset(slots); }
            }
            Action::Edit => {
                let rec = state.records[i].clone();
                state.editor.editing = Some(i);
                state.editor.guess = rec.guess.iter().map(|&c| Some(c)).collect();
                state.editor.exact = rec.exact;
                state.editor.partial = rec.partial;
            }
        }
    }
}
```

- [ ] **Step 2: 构建并手动验证**

Run: `cargo run`
Expected: 能完成——点宝石填 4 槽、填蓝/金标数、添加记录；列表出现该记录；✎ 装载编辑、更新回原位；🗑 删除；勾选框可禁用。不完整猜测时"添加记录"置灰。

- [ ] **Step 3: Commit**

```bash
git add src/gui/records_panel.rs src/gui/mod.rs
git commit -m "feat(gui): 记录面板（调色板录入/编辑/禁用/删除）"
```

---

### Task 10: 整卷求解面板

**Files:**
- Create: `src/gui/solve_panel.rs`
- Modify: `src/gui/mod.rs`（放开 `pub mod solve_panel;`；Tab::Solve 分支渲染：先 `records_panel::ui_records`，分隔线，再结果区）

**Interfaces:**
- Consumes: `gemsleuth::core::solver::{solve, find_suspects, SolveOutcome}`、`assets`、`palette`
- Produces: `pub fn ui_solve(ui: &mut egui::Ui, state: &mut SessionState);`

- [ ] **Step 1: 实现 solve_panel.rs**

```rust
//! 整卷求解结果区：唯一答案 / 多候选 / 矛盾（含嫌疑提示）。

use eframe::egui;
use gemsleuth::core::solver::{find_suspects, solve, SolveOutcome};

use super::{assets, palette, SessionState};

pub fn ui_solve(ui: &mut egui::Ui, state: &mut SessionState) {
    ui.heading("推理结果");
    ui.add_space(4.0);
    match solve(&state.settings, &state.records) {
        SolveOutcome::Unique(ans) => {
            ui.label("唯一答案：");
            ui.horizontal(|ui| {
                for &c in &ans {
                    ui.add(egui::Image::new(assets::gem(c)).max_size(egui::vec2(56.0, 56.0)));
                }
            });
            ui.label(ans.iter().map(|&c| palette::gem_name(c)).collect::<Vec<_>>().join(" "));
        }
        SolveOutcome::Ambiguous { candidates, recommendation } => {
            ui.label(format!("候选答案共 {} 个（记录不足或存在多解）：", candidates.len()));
            super::recommendation_card(ui, &recommendation);
            ui.add_space(4.0);
            ui.label("候选列表：");
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for cand in candidates.iter().take(50) {
                    ui.horizontal(|ui| {
                        for &c in cand {
                            ui.add(egui::Image::new(assets::gem(c)).max_size(egui::vec2(26.0, 26.0)));
                        }
                    });
                }
                if candidates.len() > 50 {
                    ui.label(format!("…… 其余 {} 个省略", candidates.len() - 50));
                }
            });
        }
        SolveOutcome::Contradiction => {
            ui.colored_label(egui::Color32::RED, "记录自相矛盾：请检查是否有录错的记录。");
            let suspects = find_suspects(&state.settings, &state.records);
            if !suspects.is_empty() {
                let list = suspects.iter().map(|i| format!("第 {} 条", i + 1)).collect::<Vec<_>>().join("、");
                ui.label(format!("禁用以下记录后可恢复一致（嫌疑最大）：{list}"));
            }
        }
    }
}
```

`src/gui/mod.rs` 补 `recommendation_card`（供两个面板复用）：

```rust
use gemsleuth::core::strategy::Recommendation;

pub fn recommendation_card(ui: &mut egui::Ui, rec: &Recommendation) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("推荐下一猜：");
            for &c in &rec.guess {
                ui.add(egui::Image::new(assets::gem(c)).max_size(egui::vec2(36.0, 36.0)));
            }
            ui.separator();
            match rec.guaranteed_steps {
                Some(1) => ui.label("这就是答案"),
                Some(n) => ui.label(format!("最多还需 {n} 步（已保证）")),
                None => ui.label(format!(
                    "期望剩余 ~{:.0} 个，最坏桶 {}",
                    rec.expected_remaining, rec.worst_bucket
                )),
            };
        });
        match rec.kind {
            gemsleuth::core::strategy::StrategyKind::Lookahead => ui.label("策略：精确前瞻"),
            gemsleuth::core::strategy::StrategyKind::Entropy => ui.label("策略：信息熵最大化"),
            gemsleuth::core::strategy::StrategyKind::CandidatePick => ui.label("策略：候选二选一"),
            gemsleuth::core::strategy::StrategyKind::Answer => ui.label("策略：直接给答案"),
        }
    });
}
```

（注意 `recommendation_card` 引用 `assets`，放 Task 8 之后实现没问题；本任务一并落地。Tab::Solve 分支改为：

```rust
Tab::Solve => {
    records_panel::ui_records(ui, &mut self.state);
    ui.separator();
    solve_panel::ui_solve(ui, &mut self.state);
}
```

）

- [ ] **Step 2: 构建并手动验证（金标准过一遍）**

Run: `cargo run`
录入 6 条金标准记录（橙蓝紫红 1/0、橙蓝紫绿 1/0、紫橙黄蓝 1/1、黄蓝黄紫 0/2、红黄紫蓝 1/1、橙绿黄黄 1/0），点"求解"。
Expected: 显示唯一答案「紫 紫 紫 黄」四颗大宝石 + 文字。再补录一条 `RRRR 1/0` → 红色矛盾提示 + "禁用第 7 条…"。

- [ ] **Step 3: Commit**

```bash
git add src/gui/solve_panel.rs src/gui/mod.rs
git commit -m "feat(gui): 整卷求解面板（唯一解/多候选/矛盾嫌疑提示）"
```

---

### Task 11: 陪玩助手面板

**Files:**
- Create: `src/gui/assistant_panel.rs`
- Modify: `src/gui/mod.rs`（放开 `pub mod assistant_panel;`；Tab::Assistant 分支：`records_panel::ui_records` + 结果区）

**Interfaces:**
- Consumes: `gemsleuth::core::solver::{filter_candidates, recommend}`
- Produces: `pub fn ui_assistant(ui: &mut egui::Ui, state: &mut SessionState);`

- [ ] **Step 1: 实现 assistant_panel.rs**

```rust
//! 陪玩助手结果区：实时剩余候选 + 推荐下一猜 + 候选浏览（无按钮，随记录即时刷新）。

use eframe::egui;
use gemsleuth::core::solver::{filter_candidates, recommend};

use super::{assets, SessionState};

pub fn ui_assistant(ui: &mut egui::Ui, state: &mut SessionState) {
    let candidates = filter_candidates(&state.settings, &state.records);
    match candidates.len() {
        0 => ui.colored_label(egui::Color32::RED, "当前记录自相矛盾（候选为 0），请在上方禁用或修正记录。"),
        1 => {
            ui.label("已锁定唯一答案：");
            ui.horizontal(|ui| {
                for &c in &candidates[0] {
                    ui.add(egui::Image::new(assets::gem(c)).max_size(egui::vec2(48.0, 48.0)));
                }
            });
        }
        n => {
            ui.label(format!("剩余候选：{n} 个"));
            let rec = recommend(&state.settings, &candidates);
            super::recommendation_card(ui, &rec);
            ui.add_space(4.0);
            ui.label("候选列表：");
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                egui::Grid::new("candidates_grid").num_columns(1).show(ui, |ui| {
                    for cand in candidates.iter().take(50) {
                        ui.horizontal(|ui| {
                            for &c in cand {
                                ui.add(egui::Image::new(assets::gem(c)).max_size(egui::vec2(26.0, 26.0)));
                            }
                        });
                        ui.end_row();
                    }
                });
                if n > 50 { ui.label(format!("…… 其余 {} 个省略", n - 50)); }
            });
        }
    }
}
```

Tab::Assistant 分支：

```rust
Tab::Assistant => {
    records_panel::ui_records(ui, &mut self.state);
    ui.separator();
    assistant_panel::ui_assistant(ui, &mut self.state);
}
```

- [ ] **Step 2: 构建并手动验证（陪玩流程）**

Run: `cargo run`，切到"陪玩助手" Tab，逐条添加金标准记录。
Expected: 每加一条，"剩余候选"即时收缩（1296 → … → 1）；候选 >30 时推荐卡显示"期望剩余 ~N 个，最坏桶 M"；≤30 后显示"最多还需 N 步（已保证）"；加满 6 条时显示唯一答案大图。

- [ ] **Step 3: Commit**

```bash
git add src/gui/assistant_panel.rs src/gui/mod.rs
git commit -m "feat(gui): 陪玩助手面板（实时候选/分层推荐/候选浏览）"
```

---

### Task 12: README、发布构建与最终验收

**Files:**
- Create: `README.md`
- Modify: `src/gui/mod.rs`（如 Task 8 临时自检行有残留则移除）

- [ ] **Step 1: 写 README.md**

```markdown
# gemsleuth —— 宝石推理求解器

"宝石推理"谜题（N 种宝石有放回取 K 颗；每条记录附蓝标=位置种类都对的数量、金标=种类对位置错的数量）的桌面伴侣工具。

## 构建
cargo build --release   # 产物 target/release/gemsleuth.exe（单文件，素材与字体运行时/编译期嵌入）

## 使用
- 整卷求解：把谜面全部记录录完点"求解"。
- 陪玩助手：每猜一次录一条反馈，实时看剩余候选与推荐下一猜（≤30 候选时给出"最多还需 N 步"的硬保证）。
- 录错导致矛盾时按提示禁用嫌疑记录排查。

## 参数
颜色数 4..=8（默认 6）、槽位数 3..=6（默认 4）、允许重复（默认开）——默认对齐游戏。

## 素材替换
assets/gems/gem_0..7.png（0红 1蓝 2紫 3橙 4黄 5绿 6青 7白）、assets/icons/exact|partial|unknown.png。
当前为截图裁切占位图；用高清图（≥128px 建议）**同名替换后 `cargo build --release` 重新编译即生效**，无需改代码。

## 测试
cargo test    # 含金标准测试：真实谜题 6 条记录唯一解「紫紫紫黄」
```

- [ ] **Step 2: 全量回归**

Run: `cargo test --lib && cargo build --release`
Expected: 测试全绿；release 构建成功。

- [ ] **Step 3: 手动验收清单（对照规格 §2.1）**

- F1 整卷：金标准 → 紫紫紫黄 ✓
- F2 陪玩：逐条反馈实时收缩 + 推荐卡 ✓
- F3 参数：改颜色/槽位/重复，重置确认弹窗 ✓
- F4 记录：增删改禁 ✓
- F5 矛盾：红字 + 嫌疑记录提示 ✓
- F6 多候选：列表 + 推荐卡 ✓
- F7 中文界面无乱码 ✓
- F8 素材：替换一张 gem png 重编译后界面更新 ✓

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "docs: README 与验收清单；v0.1.0 收尾"
```

---

## 任务依赖与执行顺序

1 → 2 → 3 → **（4 与 5 互为依赖：建议 5 → 4，或 4 先行但须在 5 完成后回归编译/测试）** → 6 → 7 → 8 → 9 → 10 → 11 → 12。

## 风格与纪律

- 每步命令都必须真实执行并看到预期输出（RED/GREEN 都要看到，不许跳）。
- Rust/egui 具体小 API（如 `egui::Modal`、`run_native` 闭包签名、`is_none_or`）随解析版本可能略有出入：按编译器提示做**最小**适配，并在提交信息或代码注释注明"适配 egui/eframe x.y.z"。
- 不引入 Global Constraints 之外的依赖；不改 Global Constraints 的任何常量/映射。
