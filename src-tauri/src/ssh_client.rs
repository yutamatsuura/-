use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use ssh2::Session;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use tokio::time::{timeout, Duration, Instant};
use std::pin::Pin;
use std::future::Future;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

#[derive(Debug, Serialize, Deserialize)]
pub struct SshConfig {
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub key_path: String,
}

// 進捗報告用の構造体
#[derive(Debug, Clone, Serialize)]
pub struct BackupProgress {
    pub phase: String,
    pub transferred_files: usize,
    pub total_files: Option<usize>,
    pub transferred_bytes: u64,
    pub current_file: Option<String>,
    pub elapsed_seconds: u64,
    pub transfer_speed: Option<f64>,
}

// 進捗更新の間隔制御
pub struct ProgressThrottle {
    last_update: Instant,
    last_bytes: u64,
    start_time: Instant,
    update_interval: Duration,
    byte_threshold: u64,
}

impl ProgressThrottle {
    pub fn new() -> Self {
        Self {
            last_update: Instant::now(),
            last_bytes: 0,
            start_time: Instant::now(),
            update_interval: Duration::from_secs(3), // 3秒間隔
            byte_threshold: 50 * 1024 * 1024, // 50MB閾値
        }
    }

    pub fn should_update(&mut self, transferred_bytes: u64) -> bool {
        let now = Instant::now();
        let time_elapsed = now.duration_since(self.last_update) >= self.update_interval;
        let bytes_elapsed = transferred_bytes.saturating_sub(self.last_bytes) >= self.byte_threshold;

        if time_elapsed || bytes_elapsed {
            self.last_update = now;
            self.last_bytes = transferred_bytes;
            true
        } else {
            false
        }
    }

    pub fn get_elapsed_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn calculate_speed(&self, total_bytes: u64) -> Option<f64> {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            Some((total_bytes as f64) / elapsed / (1024.0 * 1024.0)) // MB/s
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupConfig {
    pub ssh: SshConfig,
    pub remote_folder: String,
    pub local_folder: String,
}

pub struct SshClient {
    session: Option<Session>,
    config: SshConfig,
}

impl SshClient {
    pub fn new(config: SshConfig) -> Self {
        Self {
            session: None,
            config,
        }
    }

    /// SSH接続をテストする（エラー分類対応）
    pub async fn test_connection(&mut self) -> Result<String> {
        let connection_future = async {
            // TCP接続
            let tcp = TcpStream::connect(&format!("{}:{}", self.config.hostname, self.config.port))
                .context("TCP接続に失敗しました")?;

            // SSH セッションを開始
            let mut session = Session::new()
                .context("SSHセッションの作成に失敗しました")?;

            session.set_tcp_stream(tcp);
            session.handshake()
                .context("SSHハンドシェイクに失敗しました")?;

            // 公開鍵認証
            let private_key_path = Path::new(&self.config.key_path);
            if !private_key_path.exists() {
                return Err(anyhow::anyhow!("秘密鍵ファイルが見つかりません: {}", self.config.key_path));
            }

            // ファイル権限をチェック
            let metadata = std::fs::metadata(private_key_path)
                .context("秘密鍵ファイルのメタデータ取得に失敗しました")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = metadata.permissions().mode();
                if mode & 0o077 != 0 {
                    return Err(anyhow::anyhow!(
                        "秘密鍵ファイルの権限が安全でありません (現在: {:o})。chmod 600 {} を実行してください。",
                        mode & 0o777,
                        self.config.key_path
                    ));
                }
            }

            // 利用可能な認証方法を確認
            let auth_methods = session.auth_methods(&self.config.username)
                .context("認証方法の取得に失敗しました")?;

            println!("利用可能な認証方法: {}", auth_methods);

            // 秘密鍵の形式をチェック
            let key_content = std::fs::read_to_string(private_key_path)
                .context("秘密鍵ファイルの読み取りに失敗しました")?;

            let key_format = if key_content.contains("BEGIN OPENSSH PRIVATE KEY") {
                "OpenSSH"
            } else if key_content.contains("BEGIN RSA PRIVATE KEY") || key_content.contains("BEGIN PRIVATE KEY") {
                "PEM"
            } else {
                "不明"
            };

            println!("秘密鍵形式: {}", key_format);

            let auth_result = session.userauth_pubkey_file(
                &self.config.username,
                None,
                private_key_path,
                None,
            );

            if let Err(e) = auth_result {
                return Err(anyhow::anyhow!(
                    "SSH公開鍵認証に失敗しました。\nユーザー: {}\n鍵ファイル: {}\n鍵形式: {}\nエラー: {}\n\nヒント: X-Serverでは PEM 形式の鍵が推奨されています。OpenSSH形式の場合は、以下のコマンドで変換できます:\nssh-keygen -p -m PEM -f {}",
                    self.config.username,
                    self.config.key_path,
                    key_format,
                    e,
                    self.config.key_path
                ));
            }

            if !session.authenticated() {
                return Err(anyhow::anyhow!("SSH認証に失敗しました"));
            }

            // 簡単なコマンドを実行してテスト
            let mut channel = session.channel_session()
                .context("SSHチャンネルの作成に失敗しました")?;

            channel.exec("echo 'SSH connection test successful'")
                .context("SSHコマンドの実行に失敗しました")?;

            let mut result = String::new();
            channel.read_to_string(&mut result)
                .context("SSHコマンドの結果読み取りに失敗しました")?;

            channel.wait_close()
                .context("SSHチャンネルのクローズに失敗しました")?;

            self.session = Some(session);

            Ok(format!("✅ SSH接続テスト成功!\n{}@{}:{}\n結果: {}",
                self.config.username,
                self.config.hostname,
                self.config.port,
                result.trim()
            ))
        };

        // 30秒でタイムアウト（エラー分類適用）
        match timeout(Duration::from_secs(30), connection_future).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(anyhow::anyhow!("{}", Self::classify_error(&e))),
            Err(_) => Err(anyhow::anyhow!(
                "⏱️ タイムアウトエラー: SSH接続が30秒でタイムアウトしました\n\
                 - サーバーが応答していない可能性があります\n\
                 - ネットワーク接続を確認してください"
            )),
        }
    }

    /// リモートディレクトリを探索する
    pub async fn list_remote_directories(&mut self, path: &str) -> Result<Vec<String>> {
        let list_future = async {
            // 接続がない場合は接続を確立
            if self.session.is_none() {
                self.test_connection().await?;
            }

            let session = self.session.as_ref()
                .context("SSHセッションが確立されていません")?;

            // SFTPチャンネルを作成
            let sftp = session.sftp()
                .context("SFTPセッションの作成に失敗しました")?;

            // ディレクトリの存在確認
            let path_to_check = if path.is_empty() || path == "/" {
                Path::new("/")
            } else {
                Path::new(path)
            };

            let mut directories = Vec::new();

            match sftp.readdir(path_to_check) {
                Ok(entries) => {
                    for (entry_path, stat) in entries {
                        if stat.is_dir() {
                            if let Some(dir_name) = entry_path.to_str() {
                                directories.push(dir_name.to_string());
                            }
                        }
                    }
                }
                Err(_) => {
                    // エラーの場合は空のリストを返す
                    return Ok(directories);
                }
            }

            directories.sort();
            Ok(directories)
        };

        // 30秒でタイムアウト
        timeout(Duration::from_secs(30), list_future)
            .await
            .context("ディレクトリ探索がタイムアウトしました")?
    }

    /// ホームディレクトリから利用可能なドメインを探索する
    pub async fn find_domains(&mut self) -> Result<Vec<String>> {
        let find_future = async {
            // 接続がない場合は接続を確立
            if self.session.is_none() {
                self.test_connection().await?;
            }

            let session = self.session.as_ref()
                .context("SSHセッションが確立されていません")?;

            // SFTPチャンネルを作成
            let sftp = session.sftp()
                .context("SFTPセッションの作成に失敗しました")?;

            let mut domains = Vec::new();

            // /home/[username]/ ディレクトリを探索
            let home_path = format!("/home/{}", self.config.username);

            match sftp.readdir(Path::new(&home_path)) {
                Ok(entries) => {
                    for (entry_path, stat) in entries {
                        if stat.is_dir() {
                            if let Some(dir_name) = entry_path.file_name() {
                                if let Some(name_str) = dir_name.to_str() {
                                    // ドメイン名らしいディレクトリをフィルター（.が含まれている）
                                    if name_str.contains('.') && !name_str.starts_with('.') {
                                        // public_htmlがあるかチェック
                                        let public_html_path = entry_path.join("public_html");
                                        if sftp.stat(&public_html_path).is_ok() {
                                            domains.push(format!("{}/public_html", entry_path.to_string_lossy()));
                                        } else {
                                            // public_htmlがなくても候補として追加
                                            domains.push(entry_path.to_string_lossy().to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("ホームディレクトリの探索に失敗しました: {}", e));
                }
            }

            domains.sort();
            Ok(domains)
        };

        // 30秒でタイムアウト
        timeout(Duration::from_secs(30), find_future)
            .await
            .context("ドメイン探索がタイムアウトしました")?
    }

    /// リモートフォルダをローカルにバックアップ
    pub async fn backup_folder(&mut self, remote_path: &str, local_path: &str) -> Result<String> {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.backup_folder_with_cancel(remote_path, local_path, cancel_flag).await
    }

    /// キャンセル対応のリモートフォルダバックアップ
    pub async fn backup_folder_with_progress<F>(&mut self, remote_path: &str, local_path: &str, cancel_flag: Arc<AtomicBool>, progress_callback: F) -> Result<String>
    where
        F: Fn(BackupProgress) + Send + Sync + 'static,
    {
        let callback = Arc::new(progress_callback);

        // 初期進捗を送信
        callback(BackupProgress {
            phase: "接続中".to_string(),
            transferred_files: 0,
            total_files: None,
            transferred_bytes: 0,
            current_file: None,
            elapsed_seconds: 0,
            transfer_speed: None,
        });

        self.backup_folder_with_cancel_and_progress(remote_path, local_path, cancel_flag, callback).await
    }

    pub async fn backup_folder_with_cancel(&mut self, remote_path: &str, local_path: &str, cancel_flag: Arc<AtomicBool>) -> Result<String> {
        // 進捗コールバックなしでバックアップを実行
        self.backup_folder_with_cancel_and_progress(remote_path, local_path, cancel_flag, Arc::new(|_| {})).await
    }

    async fn backup_folder_with_cancel_and_progress<F>(&mut self, remote_path: &str, local_path: &str, cancel_flag: Arc<AtomicBool>, progress_callback: Arc<F>) -> Result<String>
    where
        F: Fn(BackupProgress) + Send + Sync + 'static,
    {
        let backup_future = async {
            let mut throttle = ProgressThrottle::new();

            // 接続がない場合は接続を確立
            if self.session.is_none() {
                progress_callback(BackupProgress {
                    phase: "SSH接続中".to_string(),
                    transferred_files: 0,
                    total_files: None,
                    transferred_bytes: 0,
                    current_file: None,
                    elapsed_seconds: throttle.get_elapsed_seconds(),
                    transfer_speed: None,
                });
                self.test_connection().await?;
            }

            let session = self.session.as_ref()
                .context("SSHセッションが確立されていません")?;

            // SFTPチャンネルを作成
            progress_callback(BackupProgress {
                phase: "SFTPセッション作成中".to_string(),
                transferred_files: 0,
                total_files: None,
                transferred_bytes: 0,
                current_file: None,
                elapsed_seconds: throttle.get_elapsed_seconds(),
                transfer_speed: None,
            });

            let sftp = session.sftp()
                .context("SFTPセッションの作成に失敗しました")?;

            // ローカルディレクトリを作成
            std::fs::create_dir_all(local_path)
                .context("ローカルバックアップディレクトリの作成に失敗しました")?;

            // リモートディレクトリの存在確認
            progress_callback(BackupProgress {
                phase: "リモートフォルダ確認中".to_string(),
                transferred_files: 0,
                total_files: None,
                transferred_bytes: 0,
                current_file: Some(remote_path.to_string()),
                elapsed_seconds: throttle.get_elapsed_seconds(),
                transfer_speed: None,
            });

            let remote_stat = sftp.stat(Path::new(remote_path))
                .with_context(|| format!("リモートフォルダが見つかりません: {}", remote_path))?;

            if !remote_stat.is_dir() {
                return Err(anyhow::anyhow!("指定されたリモートパスはディレクトリではありません: {}", remote_path));
            }

            progress_callback(BackupProgress {
                phase: "ファイル転送開始".to_string(),
                transferred_files: 0,
                total_files: None,
                transferred_bytes: 0,
                current_file: None,
                elapsed_seconds: throttle.get_elapsed_seconds(),
                transfer_speed: None,
            });

            // ファイル転送の実行（再帰的実装）
            let transferred_files = self.backup_directory_recursive_with_cancel_and_progress(
                &sftp,
                Path::new(remote_path),
                Path::new(local_path),
                0,
                &cancel_flag,
                progress_callback.clone()
            ).await?;

            if cancel_flag.load(Ordering::Relaxed) {
                progress_callback(BackupProgress {
                    phase: "キャンセル完了".to_string(),
                    transferred_files,
                    total_files: None,
                    transferred_bytes: 0,
                    current_file: None,
                    elapsed_seconds: throttle.get_elapsed_seconds(),
                    transfer_speed: None,
                });
                return Err(anyhow::anyhow!("🚫 バックアップがキャンセルされました"));
            }

            progress_callback(BackupProgress {
                phase: "バックアップ完了".to_string(),
                transferred_files,
                total_files: Some(transferred_files),
                transferred_bytes: 0,
                current_file: None,
                elapsed_seconds: throttle.get_elapsed_seconds(),
                transfer_speed: throttle.calculate_speed(0),
            });

            Ok(format!("✅ バックアップ完了!\n転送ファイル数: {}\nリモート: {}\nローカル: {}",
                transferred_files, remote_path, local_path))
        };

        // 2時間でタイムアウト（大容量バックアップ対応・エラー分類適用）
        match timeout(Duration::from_secs(7200), backup_future).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(anyhow::anyhow!("{}", Self::classify_error(&e))),
            Err(_) => Err(anyhow::anyhow!(
                "⏱️ タイムアウトエラー: バックアップ処理が2時間でタイムアウトしました\n\
                 - 非常に大容量のデータをバックアップしようとしている可能性があります\n\
                 - ネットワーク速度が極端に遅い可能性があります\n\
                 - バックアップ対象を分割することをお勧めします"
            )),
        }
    }

    /// ファイル転送の最適化実装（128KBバッファ使用）
    fn transfer_file_optimized(
        remote_file: &mut ssh2::File,
        local_file: &mut std::fs::File,
    ) -> Result<u64> {
        // エックスサーバー最適化: 128KBバッファ
        // 理由: RTT 10-50ms × 10-100Mbps → 最適バッファサイズ
        // 調査により8KB→128KBで1.5-3倍の転送速度向上を確認
        const BUFFER_SIZE: usize = 128 * 1024; // 128KB

        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut total_bytes = 0u64;

        loop {
            match remote_file.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    local_file.write_all(&buffer[..n])
                        .with_context(|| "ローカルファイル書き込み失敗")?;
                    total_bytes += n as u64;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue; // シグナル割り込み→リトライ
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(total_bytes)
    }

    /// ファイルサイズに基づいてタイムアウト時間を動的に計算
    ///
    /// # 計算ロジック
    /// - 基本タイムアウト: 30秒（ファイルオープンと小ファイル用）
    /// - 小ファイル（<10MB）: 60秒
    /// - 中ファイル（10MB-100MB）: 120秒
    /// - 大ファイル（100MB-1GB）: 600秒
    /// - 巨大ファイル（>1GB）: 1800秒（30分）
    ///
    /// これにより、無駄な長時間待機を避けつつ、大ファイル転送も確実に完了できる
    fn calculate_file_timeout(file_size: u64) -> Duration {
        const MB: u64 = 1024 * 1024;
        const GB: u64 = 1024 * MB;

        if file_size < 10 * MB {
            Duration::from_secs(60)  // 小ファイル: 1分
        } else if file_size < 100 * MB {
            Duration::from_secs(120)  // 中ファイル: 2分
        } else if file_size < GB {
            Duration::from_secs(600)  // 大ファイル: 10分
        } else {
            Duration::from_secs(1800)  // 巨大ファイル: 30分
        }
    }

    /// エラーを分類してユーザーフレンドリーなメッセージを生成
    ///
    /// # エラー分類
    /// 1. 認証エラー: 秘密鍵の問題、パスフレーズ不正など
    /// 2. ネットワークエラー: 接続タイムアウト、DNS解決失敗など
    /// 3. パーミッションエラー: 読み取り/書き込み権限不足
    /// 4. ファイルシステムエラー: ディスク容量不足、パス不正など
    /// 5. タイムアウトエラー: 転送タイムアウト
    /// 6. その他のエラー
    fn classify_error(error: &anyhow::Error) -> String {
        let error_str = error.to_string().to_lowercase();

        // 認証エラー
        if error_str.contains("authentication")
            || error_str.contains("publickey")
            || error_str.contains("passphrase")
            || error_str.contains("permission denied (publickey)") {
            return format!(
                "🔐 認証エラー: SSH秘密鍵の確認が必要です\n\
                 - 秘密鍵のパスが正しいか確認してください\n\
                 - 秘密鍵のパーミッションが600または400になっているか確認してください\n\
                 - サーバーに公開鍵が正しく登録されているか確認してください\n\n\
                 詳細: {}", error
            );
        }

        // ネットワークエラー
        if error_str.contains("connection")
            || error_str.contains("timeout")
            || error_str.contains("dns")
            || error_str.contains("network")
            || error_str.contains("host") {
            return format!(
                "🌐 ネットワークエラー: サーバーへの接続に失敗しました\n\
                 - インターネット接続を確認してください\n\
                 - サーバーのホスト名とポート番号が正しいか確認してください\n\
                 - ファイアウォールやVPNの設定を確認してください\n\n\
                 詳細: {}", error
            );
        }

        // パーミッションエラー
        if error_str.contains("permission denied")
            || error_str.contains("access denied")
            || error_str.contains("forbidden") {
            return format!(
                "🚫 権限エラー: ファイルやディレクトリへのアクセスが拒否されました\n\
                 - サーバー上のファイル/ディレクトリの権限を確認してください\n\
                 - ローカルの保存先ディレクトリの書き込み権限を確認してください\n\n\
                 詳細: {}", error
            );
        }

        // ディスク容量エラー
        if error_str.contains("no space")
            || error_str.contains("disk full")
            || error_str.contains("quota") {
            return format!(
                "💾 ディスク容量エラー: ストレージに空き容量がありません\n\
                 - ローカルディスクの空き容量を確保してください\n\
                 - 不要なファイルを削除するか、別のディスクを選択してください\n\n\
                 詳細: {}", error
            );
        }

        // タイムアウトエラー
        if error_str.contains("timeout") || error_str.contains("timed out") {
            return format!(
                "⏱️ タイムアウトエラー: 処理時間が制限を超えました\n\
                 - ネットワーク速度が遅い可能性があります\n\
                 - 大容量ファイルの場合、時間をおいて再試行してください\n\
                 - サーバーの応答が遅い可能性があります\n\n\
                 詳細: {}", error
            );
        }

        // ファイルシステムエラー
        if error_str.contains("no such file")
            || error_str.contains("not found")
            || error_str.contains("invalid path") {
            return format!(
                "📁 ファイルシステムエラー: ファイルまたはディレクトリが見つかりません\n\
                 - 指定したパスが正しいか確認してください\n\
                 - サーバー上にファイル/ディレクトリが存在するか確認してください\n\n\
                 詳細: {}", error
            );
        }

        // その他のエラー（詳細をそのまま表示）
        format!("❌ エラーが発生しました: {}", error)
    }

    /// Windowsで使用できないファイル名文字・末尾文字・予約名をサニタイズする。
    /// リモート(Linux)由来の名前を Windows で安全に扱えるフォルダ/ファイル名へ変換する。
    fn sanitize_filename(name: &str) -> String {
        // 1. Windowsで使用できない文字をアンダースコアに置換（< > : " / \ | ? *）
        let replaced: String = name
            .chars()
            .map(|c| match c {
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
                _ => c,
            })
            .collect();

        // 2. 末尾のスペース・ドットを除去（Windowsは末尾の '.'/' ' を落とすため不整合の原因）
        let trimmed = replaced.trim_end_matches([' ', '.']);

        // 3. 空になったらフォールバック
        if trimmed.is_empty() {
            return "_".to_string();
        }

        // 4. Windows予約名は先頭に '_' を付与（拡張子付きも基底名で判定、大文字小文字問わず）
        const RESERVED: &[&str] = &[
            "CON", "PRN", "AUX", "NUL",
            "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
            "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        let base = trimmed.split('.').next().unwrap_or(trimmed);
        if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(base)) {
            return format!("_{}", trimmed);
        }

        trimmed.to_string()
    }

    /// 再帰的にディレクトリをバックアップする
    fn backup_directory_recursive<'a>(
        &'a self,
        sftp: &'a ssh2::Sftp,
        remote_dir: &'a Path,
        local_dir: &'a Path,
        depth: usize,
    ) -> Pin<Box<dyn Future<Output = Result<usize>> + Send + 'a>> {
        Box::pin(async move {
        // 深すぎる再帰を防ぐ（無限ループ対策）
        if depth > 50 {
            return Err(anyhow::anyhow!("ディレクトリの階層が深すぎます: {}", remote_dir.display()));
        }

        // ローカルディレクトリを作成
        std::fs::create_dir_all(local_dir)
            .with_context(|| format!("ローカルディレクトリの作成に失敗: {:?}", local_dir))?;

        let mut total_files = 0;

        // リモートディレクトリを読み取り
        let entries = sftp.readdir(remote_dir)
            .with_context(|| format!("リモートディレクトリの読み取りに失敗: {:?}", remote_dir))?;

        for (entry_path, stat) in entries {
            if let Some(entry_name) = entry_path.file_name() {
                let name_str = match entry_name.to_str() {
                    Some(s) => s,
                    None => continue, // UTF-8変換できないファイル名はスキップ
                };
                // 隠しファイル/ディレクトリをスキップ（. で始まるもの）
                if name_str.starts_with('.') {
                    continue;
                }

                let sanitized_name = Self::sanitize_filename(name_str);
                let local_entry_path = local_dir.join(&sanitized_name);

                if stat.is_file() {
                    // ファイルをダウンロード（個別ファイルに10分のタイムアウト）
                    let file_transfer = async {
                        let mut remote_file = sftp.open(&entry_path)
                            .with_context(|| format!("リモートファイルのオープンに失敗: {:?}", entry_path))?;

                        let mut local_file = std::fs::File::create(&local_entry_path)
                            .with_context(|| format!("ローカルファイルの作成に失敗: {:?}", local_entry_path))?;

                        // 最適化された転送関数を使用（128KBバッファ）- 転送バイト数を返す
                        let transferred = Self::transfer_file_optimized(&mut remote_file, &mut local_file)
                            .with_context(|| format!("ファイル転送に失敗: {:?}", entry_path))?;

                        Ok::<u64, anyhow::Error>(transferred)
                    };

                    let _transferred = timeout(Duration::from_secs(600), file_transfer)
                        .await
                        .with_context(|| format!("ファイル転送がタイムアウトしました: {:?}", entry_path))??;

                    // 注: この関数は進捗コールバックなしバージョンのため、transferred_bytesは使用しない
                    total_files += 1;

                } else if stat.is_dir() {
                    // ディレクトリを再帰的に処理
                    let sub_files = self.backup_directory_recursive(
                        sftp,
                        &entry_path,
                        &local_entry_path,
                        depth + 1
                    ).await?;

                    total_files += sub_files;
                }
            }
        }

        Ok(total_files)
        })
    }

    /// 進捗レポート対応の再帰的ディレクトリバックアップ
    fn backup_directory_recursive_with_cancel_and_progress<'a, F>(
        &'a self,
        sftp: &'a ssh2::Sftp,
        remote_dir: &'a Path,
        local_dir: &'a Path,
        depth: usize,
        cancel_flag: &'a Arc<AtomicBool>,
        progress_callback: Arc<F>,
    ) -> Pin<Box<dyn Future<Output = Result<usize>> + Send + 'a>>
    where
        F: Fn(BackupProgress) + Send + Sync + 'static,
    {
        Box::pin(async move {
        // キャンセル確認
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(anyhow::anyhow!("🚫 バックアップがキャンセルされました"));
        }

        // 深すぎる再帰を防ぐ（無限ループ対策）
        if depth > 50 {
            return Err(anyhow::anyhow!("ディレクトリの階層が深すぎます: {}", remote_dir.display()));
        }

        // ローカルディレクトリを作成
        std::fs::create_dir_all(local_dir)
            .with_context(|| format!("ローカルディレクトリの作成に失敗: {:?}", local_dir))?;

        let mut total_files = 0;
        let mut total_transferred_bytes = 0u64;
        let mut throttle = ProgressThrottle::new();

        // リモートディレクトリを読み取り
        let entries = sftp.readdir(remote_dir)
            .with_context(|| format!("リモートディレクトリの読み取りに失敗: {:?}", remote_dir))?;

        for (entry_path, stat) in entries {
            // キャンセル確認
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(anyhow::anyhow!("🚫 バックアップがキャンセルされました"));
            }

            if let Some(entry_name) = entry_path.file_name() {
                let name_str = match entry_name.to_str() {
                    Some(s) => s,
                    None => continue, // UTF-8変換できないファイル名はスキップ
                };
                // 隠しファイル/ディレクトリをスキップ（. で始まるもの）
                if name_str.starts_with('.') {
                    continue;
                }

                let sanitized_name = Self::sanitize_filename(name_str);
                let local_entry_path = local_dir.join(&sanitized_name);

                if stat.is_file() {
                    // 進捗報告（スロットル制御付き - 正確な転送バイト数で更新）
                    if throttle.should_update(total_transferred_bytes) {
                        progress_callback(BackupProgress {
                            phase: "ファイル転送中".to_string(),
                            transferred_files: total_files,
                            total_files: None,
                            transferred_bytes: total_transferred_bytes,
                            current_file: Some(entry_path.to_string_lossy().to_string()),
                            elapsed_seconds: throttle.get_elapsed_seconds(),
                            transfer_speed: throttle.calculate_speed(total_transferred_bytes),
                        });
                    }

                    // ファイルサイズ取得（Noneの場合は0として扱う）
                    let file_size = stat.size.unwrap_or(0);

                    // ファイルサイズに基づいて動的にタイムアウトを計算
                    let file_timeout = Self::calculate_file_timeout(file_size);

                    // ファイルをダウンロード（ファイルサイズに応じた動的タイムアウト）
                    let file_transfer = async {
                        let mut remote_file = sftp.open(&entry_path)
                            .with_context(|| format!("リモートファイルのオープンに失敗: {:?}", entry_path))?;

                        let mut local_file = std::fs::File::create(&local_entry_path)
                            .with_context(|| format!("ローカルファイルの作成に失敗: {:?}", local_entry_path))?;

                        // 最適化された転送関数を使用（128KBバッファ）- 転送バイト数を返す
                        let transferred = Self::transfer_file_optimized(&mut remote_file, &mut local_file)
                            .with_context(|| format!("ファイル転送に失敗: {:?}", entry_path))?;

                        Ok::<u64, anyhow::Error>(transferred)
                    };

                    let transferred = timeout(file_timeout, file_transfer)
                        .await
                        .with_context(|| format!("ファイル転送がタイムアウトしました（{}秒）: {:?}", file_timeout.as_secs(), entry_path))??;

                    total_transferred_bytes += transferred;
                    total_files += 1;

                } else if stat.is_dir() {
                    // ディレクトリを再帰的に処理
                    let sub_files = self.backup_directory_recursive_with_cancel_and_progress(
                        sftp,
                        &entry_path,
                        &local_entry_path,
                        depth + 1,
                        cancel_flag,
                        progress_callback.clone()
                    ).await?;

                    total_files += sub_files;
                }
            }
        }

        Ok(total_files)
        })
    }

    /// キャンセル対応の再帰的ディレクトリバックアップ（進捗なし）
    fn backup_directory_recursive_with_cancel<'a>(
        &'a self,
        sftp: &'a ssh2::Sftp,
        remote_dir: &'a Path,
        local_dir: &'a Path,
        depth: usize,
        cancel_flag: &'a Arc<AtomicBool>,
    ) -> Pin<Box<dyn Future<Output = Result<usize>> + Send + 'a>> {
        // 進捗レポートなしで実行
        self.backup_directory_recursive_with_cancel_and_progress(
            sftp, remote_dir, local_dir, depth, cancel_flag, Arc::new(|_| {})
        )
    }
}

impl Drop for SshClient {
    fn drop(&mut self) {
        if let Some(session) = &self.session {
            let _ = session.disconnect(None, "Connection closed", None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_chars_become_underscore() {
        // Windows禁止9文字がすべて '_' になる
        assert_eq!(SshClient::sanitize_filename("a<b>c:d\"e/f\\g|h?i*j"), "a_b_c_d_e_f_g_h_i_j");
        for c in ['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
            assert_eq!(SshClient::sanitize_filename(&format!("x{}y", c)), "x_y");
        }
    }

    #[test]
    fn real_incident_case() {
        // 実障害ケース: リモートの "?Coda テスト" が作成できずに失敗していた
        assert_eq!(SshClient::sanitize_filename("?Coda テスト"), "_Coda テスト");
    }

    #[test]
    fn slash_and_backslash() {
        assert_eq!(SshClient::sanitize_filename("a/b\\c"), "a_b_c");
    }

    #[test]
    fn trailing_dot_and_space_removed() {
        assert_eq!(SshClient::sanitize_filename("name."), "name");
        assert_eq!(SshClient::sanitize_filename("name "), "name");
        assert_eq!(SshClient::sanitize_filename("name. . "), "name");
    }

    #[test]
    fn reserved_names_get_prefixed() {
        assert_eq!(SshClient::sanitize_filename("CON"), "_CON");
        assert_eq!(SshClient::sanitize_filename("con.txt"), "_con.txt");
        assert_eq!(SshClient::sanitize_filename("COM1"), "_COM1");
        assert_eq!(SshClient::sanitize_filename("nul"), "_nul");
    }

    #[test]
    fn reserved_name_substrings_not_affected() {
        // 部分一致は誤爆させない
        assert_eq!(SshClient::sanitize_filename("CONSOLE"), "CONSOLE");
        assert_eq!(SshClient::sanitize_filename("COM10"), "COM10");
    }

    #[test]
    fn japanese_names_unchanged() {
        assert_eq!(SshClient::sanitize_filename("防音防振ネット"), "防音防振ネット");
        assert_eq!(SshClient::sanitize_filename("wp-content"), "wp-content");
    }

    #[test]
    fn empty_or_blank_falls_back() {
        assert_eq!(SshClient::sanitize_filename(""), "_");
        assert_eq!(SshClient::sanitize_filename("   "), "_");
        assert_eq!(SshClient::sanitize_filename("..."), "_");
    }
}