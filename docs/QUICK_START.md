# クイックスタート

このドキュメントは、新しいAIアシスタントがプロジェクトをすぐに理解するための簡潔なガイドです。

## 5分で理解するsldshow2

### これは何？
Bevy 0.15で作られた画像スライドショーアプリ。20種類のカスタムシェーダートランジション付き。

### プロジェクト構造
```
src/main.rs          - メインロジック（システム定義、UI、キーボード入力）
src/transition.rs    - トランジションエフェクト、Material2d、TransitionPlugin
src/image_loader.rs  - 画像のロード・キャッシュ管理
src/slideshow.rs     - 自動進行タイマー
assets/shaders/      - WGSL シェーダー（20種類のエフェクト）
```

### 今すぐ動かす
```powershell
cd D:\git\sldshow2
cargo build
.\target\debug\sldshow2.exe .\example.sldshow
```

### 主要なシステム（実行順）
```rust
.add_systems(Update, (
    keyboard_input_system,        // キーボード/マウス入力
    handle_slideshow_advance,     // 自動進行
    detect_image_change,          // 画像変更を検出してTransitionEventを送る
    trigger_transition,           // TransitionEntityを作成/更新
    update_transition_on_resize,  // ウィンドウリサイズ対応
).chain())
```

### キーシステムの流れ
1. `ImageLoader` が画像をスキャン・ロード
2. `detect_image_change` が画像変更を検出
3. `TransitionEvent` を送信
4. `trigger_transition` が `TransitionEntity` を作成
5. `TransitionPlugin` がシェーダーでブレンドアニメーション

### 最近解決済み（2026-01-26）
1. ✅ **白い四角問題** → 非同期タスクで画像スキャン実装
2. ✅ **テキスト表示問題** → `bevy_embedded_assets` + M PLUS 2フォント明示的ロード

### 重要なBevy 0.15の特性
- UI Textは親子構造必須（親: Node、子: Text）
- `commands.spawn()` は即座に実行されない（ステージ終了後）
- GPUテクスチャアップロードに数フレームかかる

### デバッグツール
- **F12キー**: スクリーンショット撮影
- 自動スクリーンショット: フレーム 1,10,30,60,120,180
- ログレベル: `info!`, `debug!`, `warn!`, `error!`

### 開発ルール
- **debugビルドで開発**（イテレーション速度優先）
- コメントは英語
- システムの実行順序は`.chain()`で明示

### 次に読むべきドキュメント
- 詳細ルール → `docs/AI_DEVELOPMENT_GUIDE.md`
- 白い四角問題 → `docs/white_square_issue_analysis.md`
