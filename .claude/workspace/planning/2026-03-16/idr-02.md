# IDR: cargo fmt 適用

> 2026-03-16

## Summary

CIの `cargo fmt -- --check` 失敗を修正。`cargo fmt` を適用し、ローカルフォーマッタとの差分を解消。

## Changes

### [src/config.rs](file:////Users/thkt/GitHub/chronicler/src/config.rs)

```diff
@@ -70,10 +70,7 @@
             dir: section.dir.unwrap_or(defaults.dir),
             edit: section.edit.unwrap_or(defaults.edit),
             stop: section.stop.unwrap_or(defaults.stop),
-            mode: section
-                .mode
-                .map(|s| Mode::parse(&s))
-                .unwrap_or_default(),
+            mode: section.mode.map(|s| Mode::parse(&s)).unwrap_or_default(),
         }
```

> [!NOTE]
>
> - メソッドチェーンを1行に統合（rustfmt デフォルト）

---

### [src/main.rs](file:////Users/thkt/GitHub/chronicler/src/main.rs)

```diff
@@ -40,10 +40,7 @@
-    let target_relative = file_path
-        .strip_prefix(project_root)
-        .ok()?
-        .to_string_lossy();
+    let target_relative = file_path.strip_prefix(project_root).ok()?.to_string_lossy();
```

> [!NOTE]
>
> - メソッドチェーン1行化、`format!` マクロの引数展開、`assert!` マクロの複数行化 — 全て rustfmt デフォルト規則

---

### [src/scanner.rs](file:////Users/thkt/GitHub/chronicler/src/scanner.rs)

```diff
@@ -6,8 +6,8 @@
-static REF_RE: LazyLock<Regex> = LazyLock::new(|| {
-    Regex::new(r"...").unwrap()
-});
+static REF_RE: LazyLock<Regex> =
+    LazyLock::new(|| Regex::new(r"...").unwrap());
```

> [!NOTE]
>
> - LazyLock 初期化のブロック→式変換、`find_refs_to_file` シグネチャ1行化、vec マクロの複数行展開

---

### git diff --stat

```
 src/config.rs  |  5 +---
 src/main.rs    | 91 +++++++++++++++++++++++++++++++++-------------------------
 src/scanner.rs | 16 +++++------
 3 files changed, 61 insertions(+), 51 deletions(-)
```
