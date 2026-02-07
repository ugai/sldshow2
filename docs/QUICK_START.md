# クイックスタート

このドキュメントは、新しいAIアシスタントがプロジェクトをすぐに理解するための簡潔なガイドです。

## 5分で理解するsldshow2

### これは何？
Rust + **winit** + **wgpu** で作られた高性能画像スライドショーアプリ。20種類のカスタムシェーダートランジション付き。Bevyから移行し、フレームスパイクを解消しました。

### プロジェクト構造
```
src/main.rs          - メインロジック（イベントループ、レンダリングループ）
src/transition.rs    - WGPUパイプライン、バインドグループ管理
src/image_loader.rs  - 非同期画像ロード、テクスチャ管理（TextureManager）
src/slideshow.rs     - 自動進行タイマー
assets/shaders/      - WGSL シェーダー（20種類のエフェクト）
```

### 今すぐ動かす
```powershell
cd D:\git\sldshow2
# パフォーマンス確認のため release ビルドを推奨
cargo run --release -- .\example.sldshow
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

### キー操作一覧

| キー | 動作 |
| :--- | :--- |
| **→** / **Space** | 次の画像へ |
| **←** | 前の画像へ |
| **P** | スライドショーの 一時停止 / 再開 |
| **F** | フルスクリーン切り替え |
| **Esc** / **Q** | アプリケーション終了 |

### 解決済みの課題（2026-02-08）
1. ✅ **フレームスパイク解消**: BevyのECS/アセットシステムによる200-400msの遅延を、wgpu直接制御により解消。
2. ✅ **非同期ロード**: `image` クレート + `rayon` による並列ロード実装。

### 既知の制限
- **テキスト表示未実装**: ファイル名やデバッグ情報の表示機能は現在ありません（`glyphon` 等の導入が必要）。

### 次に読むべきドキュメント
- 詳細ルール → `CLAUDE.md`
