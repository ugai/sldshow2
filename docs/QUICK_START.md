# クイックスタート

このドキュメントは、新しいAIアシスタントがプロジェクトをすぐに理解するための簡潔なガイドです。

## 5分で理解するsldshow2

### これは何？
Rust + **winit** + **wgpu** で作られた高性能画像スライドショーアプリ。22種類のカスタムシェーダートランジション付き。

### プロジェクト構造
```
src/main.rs          - イベントループ、レンダリング
src/transition.rs    - wgpuパイプライン、シェーダー管理
src/image_loader.rs  - 非同期画像ロード、テクスチャ管理
src/text.rs          - glyphonテキストレンダリング
src/diagnostics.rs   - パフォーマンス診断
src/metadata.rs      - 画像メタデータ
src/watcher.rs       - 設定ファイルホットリロード
assets/shaders/      - 22種類のWGSLシェーダー
```

### 実行方法
```bash
# リリースビルド推奨（パフォーマンステスト時は必須）
cargo run --release -- ./example.sldshow
```

### アーキテクチャ
- **イベントループ**: winitの`EventLoop`
- **状態管理**: `ApplicationState`構造体（Device, Queue, TextureManager等）
- **レンダリング**: `RedrawRequested`で`update()`と`render()`
- **画像ロード**: rayonで並列デコード、メインスレッドでGPUアップロード
- **テキスト**: glyphonによる高品質レンダリング

### キー操作

| キー | 動作 |
| :--- | :--- |
| **→** / **Space** | 次の画像 |
| **←** | 前の画像 |
| **Home** / **End** | 最初/最後の画像 |
| **P** | 一時停止/再開 |
| **F** | フルスクリーン |
| **Esc** / **Q** | 終了 |

### 次に読むべきドキュメント
- 詳細ルール → `CLAUDE.md`
- AI開発ガイド → `docs/AI_DEVELOPMENT_GUIDE.md`
