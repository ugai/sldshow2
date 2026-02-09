# AI開発ガイド

このドキュメントは、AIアシスタントと協働開発する際のルールとガイドラインです。

## プロジェクト概要

**sldshow2** - Rust + winit + wgpu ベースの画像スライドショーアプリ

### 技術スタック
- **言語**: Rust
- **ウィンドウ管理**: winit 0.29
- **グラフィックス**: wgpu 0.19
- **テキスト**: glyphon 0.5
- **画像処理**: image 0.25, rayon 1.10
- **設定**: TOML
- **ファイル監視**: notify 7.0

## ビルドとテスト

### ビルドルール
- **パフォーマンステストは必ず`release`ビルド**
  - debugビルドはimage/wgpuが最適化されず、フレームスパイクを誤認する
  - コマンド: `cargo run --release`
- **コンパイルチェックのみdebug**: `cargo check`

### テストコマンド
```bash
# 開発実行（ロジック確認のみ、遅い）
cargo run -- ./example.sldshow

# リリース実行（パフォーマンス確認）- 推奨
cargo run --release -- ./example.sldshow

# ログ付き
RUST_LOG=debug cargo run --release -- ./example.sldshow
```

## コーディング規約

- コメント・ドキュメント: **英語**で記述
- ログ: `log`クレート使用（`println!`禁止）

## アーキテクチャ

### 設計原則
1. **状態集約**: `ApplicationState`に全て集約
2. **イベント駆動**: winitの`RedrawRequested`で`update()`/`render()`
3. **リソース管理**: rayonで画像デコード→メインスレッドでGPUアップロード
4. **テキスト**: glyphonで管理

### パフォーマンス
- 非同期ロード: バックグラウンドで画像処理
- スロットリング: フレーム毎にテクスチャアップロード制限

## モジュール構成

### コアモジュール
- **main.rs** - エントリポイント、ApplicationState、イベントループ
- **transition.rs** - wgpuパイプライン、バインドグループ、シェーダー
- **image_loader.rs** - TextureManager、非同期ロード、キャッシュ
- **text.rs** - glyphonテキストレンダリング
- **slideshow.rs** - タイマーロジック
- **config.rs** - TOML設定パース
- **diagnostics.rs** - パフォーマンス診断
- **metadata.rs** - 画像メタデータ
- **watcher.rs** - 設定ファイル監視
- **consts.rs** - 定数定義
- **error.rs** - エラー型

## デバッグワークフロー

1. **問題特定**: `RUST_LOG=debug`で詳細ログ確認
2. **修正**: `cargo check`と`cargo clippy`
3. **確認**: `cargo run --release`で動作確認
