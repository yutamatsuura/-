# 🚀 Kyosho Backup for X-Server

X-Server専用の高機能バックアップ自動化ツール

## 📋 概要

Kyosho BackupはX-Serverホスティングサービス専用に設計されたデスクトップアプリケーションです。SSH/SFTP接続を使用してサーバー上のWebサイトファイルを安全かつ効率的にローカルマシンにバックアップします。

## ✨ 主要機能

### 🔐 セキュアな接続
- **SSH公開鍵認証**: パスワード不要の安全な接続
- **X-Server最適化**: X-Serverの設定に完全対応（sv8187.xserver.jp:10022）
- **接続テスト機能**: バックアップ前に接続状態を確認

### 📂 インテリジェントなファイル管理
- **🔍 ドメイン自動探索**: サーバー上の利用可能なドメインを自動検出
- **📁 完全なディレクトリバックアップ**: サブフォルダを含む再帰的バックアップ
- **🚫 隠しファイルスキップ**: `.`で始まるファイル/フォルダを自動除外
- **🛡️ 安全制限**: 50階層制限による無限ループ防止

### 🖥️ 直感的なUI
- **ネイティブダイアログ**: OS標準のフォルダ/ファイル選択ダイアログ
- **ワンクリック選択**: 探索されたドメインをクリックで選択
- **リアルタイム進捗**: バックアップ状況をリアルタイムで表示
- **詳細な結果表示**: 転送ファイル数と完了ステータス

## 🏗️ 技術スタック

- **フロントエンド**: React 18 + TypeScript + Vite
- **バックエンド**: Rust + Tauri 2.x
- **通信**: SSH2 + SFTP
- **クロスプラットフォーム**: Windows, macOS, Linux対応

## 🚀 使用方法

### 1. SSH秘密鍵の準備
```bash
# PEM形式推奨（X-Server対応）
ssh-keygen -t rsa -b 4096 -m PEM -f ~/.ssh/xserver_key
```

### 2. アプリケーションの起動
```bash
npm run tauri:dev  # 開発モード
npm run tauri:build  # 本番ビルド
```

### 3. バックアップ手順

1. **SSH秘密鍵を選択**
   - 「参照」ボタンでファイルを選択
   - 対応形式: .key, .pem, .ppk, .openssh

2. **接続テスト**
   - 「🔍 接続テスト」ボタンをクリック
   - X-Serverへの接続状況を確認

3. **ドメイン探索**
   - 「🔍 利用可能なドメインを探索」ボタンをクリック
   - 自動検出されたドメインフォルダから選択

4. **ローカル保存先を選択**
   - 「参照」ボタンでバックアップ先フォルダを選択

5. **バックアップ実行**
   - 「🚀 バックアップ開始」ボタンをクリック
   - 完了まで待機

## 📁 プロジェクト構造

```
バックアップ自動ツール/
├── src-tauri/                  # Rustバックエンド
│   ├── src/
│   │   ├── main.rs            # メインエントリポイント
│   │   ├── ssh_client.rs      # SSH/SFTP接続処理
│   │   └── config_manager.rs  # 設定管理
│   ├── capabilities/
│   │   └── main.json          # Tauriセキュリティ設定
│   └── Cargo.toml             # Rust依存関係
├── src/                        # Reactフロントエンド
│   └── pages/
│       └── MainPage.tsx       # メインUI
├── dist/                      # ビルド出力
└── README.md                  # このファイル
```

## 🔧 開発環境のセットアップ

### 前提条件
- Node.js 18+
- Rust 1.70+
- Tauri CLI

### インストール
```bash
# 依存関係のインストール
npm install

# Tauri CLIのインストール
npm install -g @tauri-apps/cli

# 開発サーバーの起動
npm run tauri:dev
```

## 🛠️ 主要な技術実装

### SSH接続管理 (`ssh_client.rs`)

```rust
// X-Server接続設定
const XSERVER_HOST: &str = "sv8187.xserver.jp";
const XSERVER_PORT: u16 = 10022;
const XSERVER_USER: &str = "funnybooth";

// 主要機能
- test_connection(): SSH接続テスト
- find_domains(): ドメイン自動探索
- backup_folder(): 再帰的ファイル転送
```

### フロントエンド機能 (`MainPage.tsx`)

```typescript
// 主要な状態管理
const [selectedLocalFolder, setSelectedLocalFolder] = useState<string>('');
const [selectedKeyPath, setSelectedKeyPath] = useState<string>('');
const [remoteFolder, setRemoteFolder] = useState<string>('');
const [availableDomains, setAvailableDomains] = useState<string[]>([]);

// 主要機能
- handleDomainSearch(): ドメイン探索
- handleConnectionTest(): SSH接続テスト
- handleBackupStart(): バックアップ実行
```

## 🔐 セキュリティ機能

### Tauri権限システム
- ダイアログアクセス: `dialog:default`, `dialog:allow-open`
- ファイルシステムアクセス: 制限付きローカルファイル操作
- ネットワークアクセス: SSH接続のみ許可

### SSH セキュリティ
- 公開鍵認証のみサポート
- ファイル権限チェック（600推奨）
- 接続タイムアウト（30秒）
- 転送タイムアウト（5分）

## 📊 パフォーマンス

### 転送性能
- **同期転送**: ファイル単位でのシーケンシャル処理
- **進捗表示**: リアルタイム転送状況
- **エラー処理**: 部分的失敗でも継続実行

### メモリ効率
- **ストリーミング転送**: 大容量ファイル対応
- **リソース管理**: 自動的なSSH接続クリーンアップ

## 🐛 トラブルシューティング

### よくある問題と解決方法

#### SSH接続エラー
```
❌ SSH公開鍵認証に失敗しました
```
**解決方法:**
1. 秘密鍵のファイル権限を確認: `chmod 600 ~/.ssh/your_key`
2. PEM形式に変換: `ssh-keygen -p -m PEM -f ~/.ssh/your_key`
3. X-Serverに公開鍵が登録されているか確認

#### フォルダが見つからないエラー
```
❌ リモートフォルダが見つかりません: /example-domain.com
```
**解決方法:**
1. 「ドメイン探索」機能を使用
2. 正しいパス形式を確認: `/home/funnybooth/domain.com/public_html/`

#### ダイアログが開かない
```
❌ dialog.open not allowed
```
**解決方法:**
- アプリケーションを再起動
- 開発モードで権限設定を確認

## 📦 リリース・配布

### GitHub Actionsによる自動ビルド

このプロジェクトはGitHub Actionsで自動的にマルチプラットフォームビルドを実行します。

#### リリース方法

1. **バージョンタグを作成してプッシュ**
```bash
git tag v0.1.0
git push origin v0.1.0
```

2. **GitHub Actionsが自動的にビルド**
   - Windows (MSI, NSIS)
   - macOS (DMG, App)
   - Linux (DEB, AppImage)

3. **GitHubリリースページから配布物をダウンロード**
   - `https://github.com/your-username/kyosho-backup/releases`

#### 手動ビルド（ローカル）

```bash
# Windows向け (Windowsマシンで実行)
npm run tauri build

# macOS向け (macOSマシンで実行)
npm run tauri build

# Linux向け (Linuxマシンで実行)
npm run tauri build
```

### 配布物の種類

| OS | ファイル | 説明 |
|---|---|---|
| Windows | `.msi` | Windows標準インストーラー |
| Windows | `-setup.exe` | NSIS形式インストーラー |
| macOS | `.dmg` | macOS標準ディスクイメージ |
| macOS | `.app` | アプリケーションバンドル |
| Linux | `.deb` | Debian/Ubuntu向けパッケージ |
| Linux | `.AppImage` | ポータブル実行ファイル |

## 📝 更新履歴

### v0.1.0 (2025-01-10)
- ✅ X-Server専用バックアップ機能
- ✅ ネイティブファイル/フォルダ選択ダイアログ
- ✅ ドメイン自動探索機能
- ✅ 再帰的ディレクトリバックアップ
- ✅ リアルタイム進捗表示
- ✅ 包括的エラーハンドリング
- ✅ **Phase 10: ファイル転送最適化**
  - 128KBバッファ最適化（1.5-3倍高速化）
  - 動的タイムアウト計算
  - エラーメッセージ分類・改善
  - 進捗報告の精密化

## 🤝 貢献

このプロジェクトへの貢献を歓迎します。

1. このリポジトリをフォーク
2. 機能ブランチを作成 (`git checkout -b feature/amazing-feature`)
3. 変更をコミット (`git commit -m 'Add amazing feature'`)
4. ブランチにプッシュ (`git push origin feature/amazing-feature`)
5. プルリクエストを作成

## 📄 ライセンス

このプロジェクトはMITライセンスの下で公開されています。

## 👨‍💻 開発者

**Yuta Matsuura**

## 🙏 謝辞

- **Tauri**: クロスプラットフォーム開発フレームワーク
- **X-Server**: 安定したホスティングサービス
- **Rust SSH2**: 信頼性の高いSSH実装