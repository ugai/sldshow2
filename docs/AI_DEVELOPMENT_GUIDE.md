# AI開発ガイド

このドキュメントは、AIアシスタント（Claude、Gemini等）と協働してsldshow2を開発する際のルールとガイドラインをまとめたものです。

## プロジェクト概要

**sldshow2** - Rust + winit + wgpu ベースの画像スライドショーアプリケーション

### 主要機能
- 20種類のカスタムWGSLトランジションエフェクト
- TOML設定ファイルによる柔軟なカスタマイズ
- キーボード/マウスによる操作
- ハイパフォーマンス（フレームスパイクの排除）

### 技術スタック
- **言語**: Rust
- **ウィンドウ管理**: winit
- **グラフィックス**: wgpu (WebGPU API)
- **画像処理**: image, rayon (並列処理)
- **設定形式**: TOML

## ビルドとテスト

### ビルドルール

- **動作検証は基本的に `release` ビルドで行う**
  - `debug` ビルドでは `image` クレートのデコード処理や `wgpu` 最適化不足により、パフォーマンスが極端に低下し、フレームスパイクの原因を誤認する可能性があります。
  - コマンド: `cargo run --release`
  - 実行ファイル: `target/release/sldshow2.exe`

- **コンパイルチェックのみ `debug` ビルドを使用**
  - コマンド: `cargo check`

### テストコマンド例

```powershell
# Development run (check logic only, likely slow)
cargo run -- .\example.sldshow

# Release run (check performance/visuals)
cargo run --release -- .\example.sldshow
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

### パフォーマンス
- **非同期ロード**: メインスレッドをブロックしないよう、画像処理はバックグラウンドで行う。
- **スロットリング**: GPUへのテクスチャアップロードはフレームごとに制限し、スタッターを防ぐ（1フレームあたり1枚など）。

## 現在の課題（2026-02-08）

### 未実装機能
- **テキスト表示**: ファイル名やデバッグ情報のレンダリング機能がありません。`glyphon` 等のライブラリ導入が必要です。

### 解決済み
- ✅ **フレームスパイク**: Bevy ECSのオーバーヘッドを排除し、wgpu直接制御によりスムーズなトランジションを実現。
- ✅ **非同期ロード**: `TextureManager` 実装により、4K画像のロード時でもアニメーションがカクつかない。

## ファイル構成

```
D:\git\sldshow2\
├── src/
│   ├── main.rs              # アプリケーションエントリ、イベントループ
│   ├── config.rs            # 設定ファイル読み込み
│   ├── image_loader.rs      # TextureManager（画像ロード・キャッシュ）
│   ├── slideshow.rs         # スライドショータイマー・ロジック
│   ├── transition.rs        # WGPUパイプライン、シェーダー管理
│   └── error.rs             # エラー型定義
├── assets/
│   └── shaders/
│       └── transition.wgsl  # トランジションシェーダー
├── docs/
│   ├── AI_DEVELOPMENT_GUIDE.md          # このファイル
│   └── QUICK_START.md                   # 簡易ガイド
└── example.sldshow          # サンプル設定ファイル
```

## デバッグワークフロー

1. **問題の特定**
   - `RUST_LOG=debug` 環境変数を設定して実行し、詳細ログを確認。
2. **修正**
   - `cargo check` でコンパイルエラーを確認。
   - `cargo clippy` で静的解析を実行。
3. **確認**
   - `cargo run --release` で動作確認。
