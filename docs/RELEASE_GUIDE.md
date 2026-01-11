# リリースガイド - Windows版のダウンロード方法

## 🎯 Windows向けインストーラーの入手方法

### ステップ1: GitHubリリースページにアクセス

以下のURLにアクセスしてください：

```
https://github.com/yutamatsuura/kyosho-backup/releases
```

### ステップ2: 最新リリースを確認

- **v0.1.0** または最新のバージョン番号をクリック
- 現在、GitHub Actionsがビルド中です（約10-15分かかります）

### ステップ3: Windows向けインストーラーをダウンロード

ビルドが完了すると、以下のファイルが利用可能になります：

#### オプション1: MSIインストーラー（推奨）
```
Kyosho Backup_0.1.0_x64_en-US.msi
```
- Windows標準のインストーラー
- 推奨される方法
- ダブルクリックでインストール

#### オプション2: NSISインストーラー
```
Kyosho Backup_0.1.0_x64-setup.exe
```
- 代替インストーラー
- より詳細なインストールオプション

---

## 📦 インストール手順（Windows）

### MSIインストーラーの場合

1. **ダウンロードしたMSIファイルをダブルクリック**
2. **セットアップウィザードに従う**
   - 「次へ」をクリック
   - インストール先を選択（デフォルトで問題ありません）
   - 「インストール」をクリック
3. **完了後、スタートメニューから起動**
   - スタートメニュー → Kyosho Backup

### SmartScreen警告が表示された場合

Windows Defenderが警告を表示することがあります（署名がないため）：

1. **「詳細情報」をクリック**
2. **「実行」をクリック**

---

## ⏱️ ビルド状況の確認方法

### GitHub Actionsページで進捗確認

```
https://github.com/yutamatsuura/kyosho-backup/actions
```

1. **Actionsタブをクリック**
2. **"Release Build"ワークフローを選択**
3. **最新の実行結果を確認**
   - 🟢 緑のチェックマーク = 完了
   - 🟡 黄色の点 = 実行中
   - 🔴 赤いバツ = エラー

### ビルド完了までの目安時間

| プラットフォーム | 所要時間 |
|----------------|----------|
| Windows | 8-12分 |
| macOS (x64) | 6-10分 |
| macOS (ARM) | 6-10分 |
| Linux | 5-8分 |

**合計**: 約10-15分

---

## 🔄 リリースのトリガー方法（開発者向け）

### 自動トリガー（タグプッシュ）

```bash
# バージョンタグを作成
git tag -a v0.1.1 -m "Release v0.1.1"

# GitHubにプッシュ
git push origin v0.1.1
```

→ 自動的にGitHub Actionsがトリガーされ、マルチプラットフォームビルドが開始されます。

### 手動トリガー

1. GitHubリポジトリページにアクセス
2. **Actions** タブをクリック
3. **Release Build** ワークフローを選択
4. **Run workflow** ボタンをクリック
5. ブランチを選択（通常は `main`）
6. **Run workflow** を実行

---

## 📁 配布物一覧

### Windows向け

| ファイル名 | サイズ目安 | 用途 |
|-----------|----------|------|
| `Kyosho Backup_0.1.0_x64_en-US.msi` | 5-8MB | MSIインストーラー |
| `Kyosho Backup_0.1.0_x64-setup.exe` | 6-9MB | NSISインストーラー |

### macOS向け

| ファイル名 | サイズ目安 | 用途 |
|-----------|----------|------|
| `Kyosho Backup_0.1.0_x64.dmg` | 4-6MB | Intel Mac用 |
| `Kyosho Backup_0.1.0_aarch64.dmg` | 4-6MB | Apple Silicon Mac用 |

### Linux向け

| ファイル名 | サイズ目安 | 用途 |
|-----------|----------|------|
| `kyosho-backup_0.1.0_amd64.deb` | 5-8MB | Debian/Ubuntu |
| `kyosho-backup_0.1.0_amd64.AppImage` | 15-20MB | ポータブル実行ファイル |

---

## 🚨 トラブルシューティング

### ビルドが失敗した場合

1. **Actions ログを確認**
   - GitHub Actionsページでエラーログを確認
   - 赤いバツマークのジョブをクリック

2. **よくあるエラー原因**
   - Node.js依存関係の問題 → `package.json` を確認
   - Rust依存関係の問題 → `Cargo.toml` を確認
   - 権限エラー → リポジトリ設定を確認

### ダウンロードリンクが見つからない場合

- ビルドが完了しているか確認
- GitHubリリースページを再読み込み
- Actionsページでビルド完了を確認

---

## 📞 サポート

問題が発生した場合は、GitHubのIssuesページで報告してください：

```
https://github.com/yutamatsuura/kyosho-backup/issues
```

---

## 🎊 現在のステータス

✅ **Phase 10 完了**
- ファイル転送最適化実装
- マルチプラットフォームビルド設定完了
- GitHub Actionsワークフロー作成完了
- v0.1.0タグプッシュ完了

🔄 **現在進行中**
- GitHub Actionsビルド実行中（10-15分）

🎯 **次のステップ**
- ビルド完了後、GitHubリリースページからWindows版をダウンロード
