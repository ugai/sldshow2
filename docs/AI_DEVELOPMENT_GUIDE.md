# AI開発ガイド

このドキュメントは、AIアシスタント（Claude、Gemini等）と協働してsldshow2を開発する際のルールとガイドラインをまとめたものです。

## プロジェクト概要

**sldshow2** - Bevy 0.15ベースの画像スライドショーアプリケーション

### 主要機能
- 20種類のカスタムWGSLトランジションエフェクト
- TOML設定ファイルによる柔軟なカスタマイズ
- キーボード/マウスによる操作
- フルスクリーン対応
- EXIF情報読み取り（回転補正）

### 技術スタック
- **言語**: Rust
- **ゲームエンジン**: Bevy 0.15
- **シェーダー**: WGSL (WebGPU Shading Language)
- **設定形式**: TOML

## ビルドとテスト

### ビルドルール

- **普段の動作検証はdebugビルドで行う**
  - イテレーションを速く回すため
  - コマンド: `cargo build`
  - 実行ファイル: `target/debug/sldshow2.exe`

- **リリース前の最終確認のみreleaseビルドを使用**
  - コマンド: `cargo build --release`
  - 実行ファイル: `target/release/sldshow2.exe`

### テストコマンド例

```powershell
# Debug build and run
cd D:\git\sldshow2 && cargo build && .\target\debug\sldshow2.exe .\example.sldshow

# Release build and run
cd D:\git\sldshow2 && cargo build --release && .\target\release\sldshow2.exe .\example.sldshow
```

## コーディング規約

### コメントとドキュメント

- 関数のドキュメントコメント（`///`）は英語で記述
- コード内の説明コメント（`//`）も英語で記述
- 日本語コメントは避ける（このガイドを除く）

### ログメッセージ

- ログレベルの使い分け:
  - `info!`: 通常の動作ログ（画像ロード、遷移開始など）
  - `debug!`: デバッグ情報（スキップされた処理など）
  - `warn!`: 警告（画像が見つからないなど）
  - `error!`: エラー（致命的な問題）

## アーキテクチャ原則

### Bevy ECS設計

- システムの実行順序は`.chain()`で明示的に制御
- 複雑な状態管理はResourceを使用
- エンティティとコンポーネントの責務を明確に分離

### パフォーマンス

- 不要なクローンを避ける
- システムのクエリは必要最小限に
- リソースの変更検知（`Changed<T>`）を活用

## 現在の課題

### 解決済み
- ✅ キーボードホールドによる高速画像送り（1秒遅延 + 60ms間隔）
- ✅ フルスクリーントグル（Fキー）
- ✅ ランダムトランジションモードの範囲修正（0-19）

### 最近解決済み（2026-01-26）

1. **白い四角問題（HIGH）** ✅
   - 原因: `setup`関数内の画像スキャンがメインスレッドをブロック
   - 解決策: 非同期タスク（`AsyncComputeTaskPool`）を使用した画像スキャン
   - 実装: `start_image_scan` + `poll_image_scan` システム
   - 結果: 2フレーム遅延後にバックグラウンドスレッドでスキャン実行、メインスレッドは一切ブロックされない

2. **テキスト表示が機能しない（HIGH）** ✅
   - 原因: フォントロード方法の問題
   - 解決策: `bevy_embedded_assets`プラグイン + M PLUS 2フォント明示的ロード
   - 実装: `server.load("fonts/MPLUS2-VariableFont_wght.ttf")`
   - スタイル: 黒背景（0.5透明度）+ 白テキスト（20px）



## ファイル構成

```
D:\git\sldshow2\
├── src/
│   ├── main.rs              # メインアプリケーションロジック
│   ├── config.rs            # TOML設定ファイル処理
│   ├── image_loader.rs      # 画像ロード・キャッシュ管理
│   ├── slideshow.rs         # スライドショータイマー
│   ├── transition.rs        # トランジションエフェクト
│   └── exif.rs              # EXIF情報読み取り
├── assets/
│   └── shaders/
│       └── transition.wgsl  # トランジションシェーダー
├── docs/
│   ├── AI_DEVELOPMENT_GUIDE.md          # このファイル
│   └── white_square_issue_analysis.md   # 白い四角問題の分析
└── example.sldshow          # サンプル設定ファイル
```

## デバッグ機能

### スクリーンショット

- **F12キー**: 手動スクリーンショット撮影
- 自動スクリーンショット: フレーム 1, 10, 30, 60, 120, 180で自動撮影
- 保存先: `debug_screenshots/` フォルダ

### スクリーンショットの無効化

デバッグが完了したら、自動スクリーンショットを無効化:

```rust
// src/main.rs の DebugScreenshotState::default()
Self {
    capture_frames: vec![],  // 空にする
    frame_count: 0,
    enabled: false,  // falseにする
}
```

## 開発ワークフロー

1. **問題の特定**
   - ログを確認
   - 必要に応じてスクリーンショットを撮影（F12キー）

2. **分析**
   - システムの実行順序を確認
   - Resourceの状態変化を追跡

3. **修正**
   - Debug buildで動作確認
   - ログで挙動を検証

4. **ドキュメント更新**
   - 重要な問題は`docs/`に記録
   - このガイドも必要に応じて更新

## 注意事項

### Bevy 0.15の特性

- UI Textは親子構造が必須（親: Node、子: Text）
- Spriteも Material2dもGPUアップロード待ちが発生する
- `commands.spawn()`は即座に実行されず、ステージ終了後に適用される

### Windows固有の問題

- ファイルパスは絶対パスを使用（相対パスは動作不安定）
- ファイルウォッチャーは無効化済み（Windowsのパス問題回避）
