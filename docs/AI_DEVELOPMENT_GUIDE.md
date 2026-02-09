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
