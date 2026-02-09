# Documentation Update Draft

## Overview

This document contains the draft updates for README.md and docs/ to reflect the actual codebase architecture.

---

## Critical Issues Found

### 1. Framework Mismatch 🚨
**Current Documentation**: Claims built with "Bevy 0.15"
**Actual Codebase**: Uses winit 0.29 + wgpu 0.19 (NO Bevy dependency)

### 2. Text Rendering Status
**Current Documentation**: Claims "未実装" (not implemented)
**Actual Status**: ✅ Implemented with glyphon 0.5 (src/text.rs exists, 10KB)

### 3. Transition Effect Count
**Current Documentation**:
- README.md: 22 effects ✅ CORRECT
- QUICK_START.md: 20 effects ❌ INCORRECT

**Actual**: 22 effects (mode 0-21) confirmed in assets/shaders/transition.wgsl

### 4. Undocumented Modules
The following modules exist but are not documented:
- `diagnostics.rs` (5.5KB)
- `metadata.rs` (3.4KB)
- `watcher.rs` (5.2KB) - File watching for hot-reload
- `consts.rs` (260 bytes)
- `text.rs` (10KB) - Text rendering with glyphon

---

## Updated README.md

```markdown
# sldshow2

High-performance slideshow image viewer with custom WGSL transitions, built with Rust, winit, and wgpu.

## Features

- **22 different transition effects** with custom WGSL shaders
- **Embedded assets** (shaders) for standalone distribution
- **Async image loading** for non-blocking startup and navigation
- **Frameless window support** for clean presentation
- **TOML configuration** with flexible settings
- **Smart image preloading/caching** (configurable extent)
- **Auto-advance timer** with pause/resume
- **Keyboard/mouse controls** with hold-to-repeat navigation
- **Text rendering** with glyphon for file path display
- **Hot-reload configuration** via file watching

## Quick Start

### 1. Generate Test Images

```bash
cargo run --example generate_test_images
```

This creates 7 CC0 test images in `assets/test_images/`.

### 2. Run with Test Configuration

```bash
# Development build
cargo run -- test.sldshow

# Release build (RECOMMENDED for performance testing)
cargo run --release -- test.sldshow
```

**Note**: Use `--release` for accurate performance evaluation. Debug builds may exhibit frame stuttering due to unoptimized image decoding and GPU operations.

### 3. Test Controls

**Keyboard:**
- `→` / `Space` - Next image (hold to fast-forward)
- `←` - Previous image (hold to rewind)
- `Home` - First image
- `End` - Last image
- `P` - Toggle pause/resume
- `F` - Toggle fullscreen
- `ESC` / `Q` - Quit

**Mouse:**
- Left click - Next image
- Right click - Previous image
- Scroll wheel - Navigate images

## Building

```bash
# Development build (compile-time check)
cargo build

# Release build (optimized, recommended for distribution)
cargo build --release
```

The executable is standalone and includes all required assets (shaders) embedded at compile time. You can run the binary from any location.

## Configuration

See `example.sldshow` for all configuration options.

Default config location: `~/.sldshow`

### Key Settings

**Window:**

- `width`, `height` - Window dimensions
- `fullscreen` - Fullscreen mode
- `decorations` - Show/hide titlebar

**Viewer:**

- `image_paths` - Directories or files to display
- `timer` - Seconds per image (0 = paused)
- `shuffle` - Random order
- `cache_extent` - Number of images to preload

**Transition:**

- `time` - Transition duration in seconds
- `random` - Use random effects
- `mode` - Specific effect (0-21) if not random

**Style:**

- `bg_color` - Background color [R, G, B, A]
- `show_image_path` - Display current file path

## Transition Effects

22 different effects (mode 0-21):

- 0-1: Crossfade variations
- 2-9: Roll (from various directions)
- 10-11: Sliding door (open/close)
- 12-15: Blind effects
- 16-17: Box (expand/contract)
- 18-21: Advanced effects (random squares, angular wipe, etc.)

## Project Structure

```txt
sldshow2/
├── src/
│   ├── main.rs              # Entry point, event loop, state management
│   ├── config.rs            # TOML configuration parsing
│   ├── image_loader.rs      # Async image loading & texture cache
│   ├── transition.rs        # wgpu render pipeline & shader uniforms
│   ├── slideshow.rs         # Auto-advance timer logic
│   ├── text.rs              # Text rendering with glyphon
│   ├── diagnostics.rs       # Performance diagnostics
│   ├── metadata.rs          # Image metadata extraction
│   ├── watcher.rs           # File watching for hot-reload
│   ├── consts.rs            # Application constants
│   └── error.rs             # Custom error types
├── assets/
│   ├── shaders/
│   │   └── transition.wgsl  # 22 transition effects (embedded at compile time)
│   └── test_images/         # Generated test images
├── docs/
│   ├── AI_DEVELOPMENT_GUIDE.md  # AI collaboration guidelines
│   └── QUICK_START.md           # Quick start guide
├── examples/
│   └── generate_test_images.rs # Test image generator
├── test.sldshow             # Test configuration
└── example.sldshow          # Example configuration
```

## Development

### Code Statistics

- ~1,200 lines of Rust
- 11 core modules
- 22 WGSL transition effects

### Architecture

**Direct wgpu Control Architecture:**
- **Event-driven**: Uses `winit` event loop with `RedrawRequested` events
- **State Management**: `ApplicationState` struct holds all app state
  - `wgpu::Device`, `wgpu::Queue` for GPU operations
  - `TextureManager` for async image loading and caching
  - `TransitionPipeline` for render pipeline and bind groups
- **Async Loading**: `rayon` thread pool for non-blocking image decoding
- **Compile-time asset embedding** for standalone distribution
- **Hot-reload**: `notify` crate watches config file for changes

### Key Components

**ApplicationState** (main.rs):
- Central state management
- Event handling and input processing
- Update and render loop coordination

**TransitionPipeline** (transition.rs):
- wgpu render pipeline setup
- Bind group management
- Shader uniform updates

**TextureManager** (image_loader.rs):
- Background thread image decoding
- GPU texture upload throttling
- Rolling texture cache with configurable extent
- Automatic image resizing to fit window

**TextRenderer** (text.rs):
- glyphon-based text rendering
- File path display with custom styling

## Troubleshooting

**No images displayed:**

- Check that `image_paths` in config points to valid directories
- Ensure images are in supported formats (PNG, JPG, GIF, WebP, BMP, TGA, TIFF, ICO, HDR)
- Check console output for error messages

**Transitions not working:**

- Shader compilation errors will be logged to console
- Shaders are embedded in the executable; rebuild if issues persist

**Text not displaying:**

- Check `show_image_path` setting in config
- Verify glyphon initialization in logs

**Performance issues:**

- Reduce `cache_extent` if using many large images
- Lower `transition.time` for faster transitions
- Use `fullscreen = false` and smaller window size
- **Use release builds** (`cargo run --release`) for accurate performance

## License

MIT

## Credits

Based on the original [sldshow](https://github.com/ugai/sldshow) by ugai.

Transition effects adapted from [GL Transitions](https://gl-transitions.com/) (MIT License).

Test images are programmatically generated (CC0/Public Domain).
```

---

## Updated docs/QUICK_START.md

```markdown
# クイックスタート

このドキュメントは、新しいAIアシスタントがプロジェクトをすぐに理解するための簡潔なガイドです。

## 5分で理解するsldshow2

### これは何？
Rust + **winit** + **wgpu** で作られた高性能画像スライドショーアプリ。**22種類**のカスタムシェーダートランジション付き。Bevyから移行し、フレームスパイクを解消しました。

### プロジェクト構造
```
src/main.rs          - メインロジック（イベントループ、レンダリングループ）
src/transition.rs    - WGPUパイプライン、バインドグループ管理
src/image_loader.rs  - 非同期画像ロード、テクスチャ管理（TextureManager）
src/slideshow.rs     - 自動進行タイマー
src/text.rs          - glyphonベースのテキストレンダリング
src/diagnostics.rs   - パフォーマンス診断
src/metadata.rs      - 画像メタデータ抽出
src/watcher.rs       - 設定ファイルのホットリロード
assets/shaders/      - WGSL シェーダー（22種類のエフェクト）
```

### 今すぐ動かす
```bash
cd /home/user/sldshow2
# パフォーマンス確認のため release ビルドを推奨
cargo run --release -- ./example.sldshow
```

### アーキテクチャの要点
1. **イベントループ**: `winit` の `EventLoop` が主導権を持つ。
2. **状態管理**: `ApplicationState` 構造体に全てを集約（Window, Device, Queue, Subsystems）。
3. **レンダリング**:
   - `RedrawRequested` で `state.update()` と `state.render()` を呼び出す。
   - `TransitionPipeline` がシェーダーとユニフォームを管理。
4. **リソース管理**:
   - `TextureManager` が別スレッド（`rayon`）で画像をデコード。
   - メインスレッドでGPUへアップロード（`queue.write_texture`）。
   - VRAM使用量を抑えるため、テクスチャはウィンドウサイズに合わせてリサイズされる。
5. **テキストレンダリング**:
   - `glyphon` クレートを使用して高品質なテキスト表示を実装。
   - ファイルパス表示機能が利用可能。

### キー操作一覧

| キー | 動作 |
| :--- | :--- |
| **→** / **Space** | 次の画像へ |
| **←** | 前の画像へ |
| **Home** | 最初の画像へ |
| **End** | 最後の画像へ |
| **P** | スライドショーの 一時停止 / 再開 |
| **F** | フルスクリーン切り替え |
| **Esc** / **Q** | アプリケーション終了 |

### 解決済みの課題（2026-02-08）
1. ✅ **フレームスパイク解消**: BevyのECS/アセットシステムによる200-400msの遅延を、wgpu直接制御により解消。
2. ✅ **非同期ロード**: `image` クレート + `rayon` による並列ロード実装。
3. ✅ **テキスト表示実装**: `glyphon` によるテキストレンダリング機能を実装。

### 実装済み機能
- ✅ 22種類のトランジションエフェクト
- ✅ ファイルパス表示（`show_image_path` 設定）
- ✅ 設定ファイルのホットリロード
- ✅ パフォーマンス診断機能
- ✅ 画像メタデータ抽出

### 次に読むべきドキュメント
- 詳細ルール → `CLAUDE.md`
- AI開発ガイド → `docs/AI_DEVELOPMENT_GUIDE.md`
```

---

## Updated docs/AI_DEVELOPMENT_GUIDE.md

```markdown
# AI開発ガイド

このドキュメントは、AIアシスタント（Claude、Gemini等）と協働してsldshow2を開発する際のルールとガイドラインをまとめたものです。

## プロジェクト概要

**sldshow2** - Rust + winit + wgpu ベースの画像スライドショーアプリケーション

### 主要機能
- 22種類のカスタムWGSLトランジションエフェクト
- TOML設定ファイルによる柔軟なカスタマイズ
- キーボード/マウスによる操作
- ハイパフォーマンス（フレームスパイクの排除）
- glyphonベースのテキストレンダリング
- 設定ファイルのホットリロード機能

### 技術スタック
- **言語**: Rust
- **ウィンドウ管理**: winit 0.29
- **グラフィックス**: wgpu 0.19 (WebGPU API)
- **テキストレンダリング**: glyphon 0.5
- **画像処理**: image 0.25, rayon 1.10 (並列処理)
- **設定形式**: TOML
- **ファイル監視**: notify 7.0

## ビルドとテスト

### ビルドルール

- **動作検証は基本的に `release` ビルドで行う**
  - `debug` ビルドでは `image` クレートのデコード処理や `wgpu` 最適化不足により、パフォーマンスが極端に低下し、フレームスパイクの原因を誤認する可能性があります。
  - コマンド: `cargo run --release`
  - 実行ファイル: `target/release/sldshow2`

- **コンパイルチェックのみ `debug` ビルドを使用**
  - コマンド: `cargo check`

### テストコマンド例

```bash
# Development run (check logic only, likely slow)
cargo run -- ./example.sldshow

# Release run (check performance/visuals) - RECOMMENDED
cargo run --release -- ./example.sldshow

# With logging
RUST_LOG=debug cargo run --release -- ./example.sldshow
```

## コーディング規約

### コメントとドキュメント
- 関数のドキュメント（`///`）およびコード内コメント（`//`）は**英語**で記述
- 日本語コメントは避ける（このガイドを除く）

### ログメッセージ
- `log` クレートを使用 (`info!`, `warn!`, `error!`, `debug!`)
- `println!` の使用は避ける

## アーキテクチャ原則

### Winit + Wgpu 設計
1. **状態の集約**: `ApplicationState` 構造体に `Device`, `Queue`, `TextureManager`, `TransitionPipeline` などを保持。
2. **イベント駆動**: `winit` の `RedrawRequested` イベントで `update()` と `render()` を呼び出す。
3. **リソース管理**:
   - `TextureManager` は `rayon` スレッドプールで画像をデコード。
   - メインスレッドで `Queue::write_texture` を使用してGPUにアップロード。
   - VRAM管理のため、画像はウィンドウサイズに合わせてリサイズする。
4. **テキストレンダリング**:
   - `glyphon` による高品質テキスト表示。
   - `TextRenderer` が描画を管理。

### パフォーマンス
- **非同期ロード**: メインスレッドをブロックしないよう、画像処理はバックグラウンドで行う。
- **スロットリング**: GPUへのテクスチャアップロードはフレームごとに制限し、スタッターを防ぐ（1フレームあたり1枚など）。

## モジュール構成

### コアモジュール

**main.rs** (37KB)
- アプリケーションエントリポイント
- `ApplicationState` 構造体
- イベントループと入力処理
- update/render ループ調整

**transition.rs** (7.3KB)
- wgpu レンダーパイプライン設定
- バインドグループ管理
- シェーダーユニフォームの更新

**image_loader.rs** (10KB)
- `TextureManager` 実装
- バックグラウンドスレッドでの画像デコード
- GPUテクスチャアップロードのスロットリング
- 設定可能なローリングキャッシュ
- 自動画像リサイズ

**text.rs** (10KB)
- glyphonベースのテキストレンダリング
- ファイルパス表示
- カスタムスタイリング

**slideshow.rs** (1.3KB)
- 自動進行タイマーロジック
- 一時停止/再開機能

**config.rs** (8.7KB)
- TOML設定ファイルのパース
- デフォルト値管理
- バリデーション

**diagnostics.rs** (5.5KB)
- パフォーマンス診断
- フレームタイム測定
- デバッグ情報出力

**metadata.rs** (3.4KB)
- 画像メタデータ抽出
- ファイル情報管理

**watcher.rs** (5.2KB)
- 設定ファイル監視
- ホットリロード機能

**consts.rs** (260B)
- アプリケーション定数

**error.rs** (1.3KB)
- カスタムエラー型定義

## 現在の状態（2026-02-09）

### 実装済み機能
- ✅ **22種類のトランジションエフェクト**: 完全実装（mode 0-21）
- ✅ **テキストレンダリング**: glyphonによる高品質表示
- ✅ **ホットリロード**: 設定ファイルの自動再読み込み
- ✅ **パフォーマンス診断**: フレームタイム測定とログ出力
- ✅ **非同期ロード**: スムーズな画像読み込み

### 解決済み
- ✅ **フレームスパイク**: wgpu直接制御によりスムーズなトランジションを実現。
- ✅ **非同期ロード**: `TextureManager` 実装により、4K画像のロード時でもアニメーションがカクつかない。

## ファイル構成

```
/home/user/sldshow2/
├── src/
│   ├── main.rs              # アプリケーションエントリ、イベントループ
│   ├── config.rs            # 設定ファイル読み込み
│   ├── image_loader.rs      # TextureManager（画像ロード・キャッシュ）
│   ├── slideshow.rs         # スライドショータイマー・ロジック
│   ├── transition.rs        # WGPUパイプライン、シェーダー管理
│   ├── text.rs              # テキストレンダリング（glyphon）
│   ├── diagnostics.rs       # パフォーマンス診断
│   ├── metadata.rs          # 画像メタデータ
│   ├── watcher.rs           # ファイル監視・ホットリロード
│   ├── consts.rs            # 定数定義
│   └── error.rs             # エラー型定義
├── assets/
│   └── shaders/
│       └── transition.wgsl  # トランジションシェーダー（22種類）
├── docs/
│   ├── AI_DEVELOPMENT_GUIDE.md          # このファイル
│   └── QUICK_START.md                   # 簡易ガイド
├── CLAUDE.md                # Claude Code用開発ガイド
└── example.sldshow          # サンプル設定ファイル
```

## デバッグワークフロー

1. **問題の特定**
   - `RUST_LOG=debug` 環境変数を設定して実行し、詳細ログを確認。
2. **修正**
   - `cargo check` でコンパイルエラーを確認。
   - `cargo clippy` で静的解析を実行。
3. **確認**
   - `cargo run --release` で動作確認（パフォーマンステストは必ずreleaseビルドで）。
```

---

## Summary of Changes

### README.md
- ❌ Removed all Bevy references
- ✅ Updated to reflect winit + wgpu architecture
- ✅ Added missing modules to project structure
- ✅ Updated architecture description
- ✅ Added text rendering feature
- ✅ Removed Bevy-specific troubleshooting items
- ✅ Verified transition effect count (22 is correct)

### docs/QUICK_START.md
- ✅ Fixed transition effect count (20 → 22)
- ✅ Removed "テキスト表示未実装" claim
- ✅ Added text rendering to implemented features
- ✅ Added all missing modules to structure
- ✅ Updated feature status to reflect implementation

### docs/AI_DEVELOPMENT_GUIDE.md
- ❌ Removed all Bevy references
- ✅ Updated to reflect winit + wgpu stack
- ✅ Fixed transition effect count (20 → 22)
- ✅ Removed text rendering from "未実装機能"
- ✅ Added all modules with descriptions
- ✅ Updated technology stack versions
- ✅ Corrected implementation status

---

## Next Steps

1. Review this draft
2. Apply changes to actual files
3. Test that all information is accurate
4. Commit and push changes
5. Create GitHub issue documenting the updates

---

**Created**: 2026-02-09
**Session**: https://claude.ai/code/session_01QMqe96p1e7BGQmioCexu2a
