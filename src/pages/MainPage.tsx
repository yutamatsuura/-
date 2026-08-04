import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-shell';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { BackupResult, BackupProgress } from '../types/tauri';
import { useAuth } from '../hooks/useAuth';
import PinAuthModal from '../components/PinAuthModal';
import {
  Rocket,
  Server,
  Key,
  Folder,
  HardDrive,
  Play,
  Search,
  Download,
  Clock,
  FileText,
  Settings,
  BarChart3,
  Shield,
  Home,
  Globe,
  Volume,
  Lock,
  Unlock,
  Square
} from 'lucide-react';

const MainPage: React.FC = () => {
  const { isAuthenticated, isPinEnabled, isLoading, authenticate } = useAuth();
  const [appVersion, setAppVersion] = useState<string>('');
  const [selectedLocalFolder, setSelectedLocalFolder] = useState<string>('');
  const [selectedKeyPath, setSelectedKeyPath] = useState<string>('');
  const [remoteFolder, setRemoteFolder] = useState<string>('');
  const [isBackupRunning, setIsBackupRunning] = useState<boolean>(false);
  const [backupResult, setBackupResult] = useState<string>('');
  const [progressMessage, setProgressMessage] = useState<string>('');
  const [connectionStatus, setConnectionStatus] = useState<string>('');
  const [availableDomains, setAvailableDomains] = useState<string[]>([]);
  const [isDomainSearching, setIsDomainSearching] = useState<boolean>(false);
  const [showPinAuth, setShowPinAuth] = useState<boolean>(false);
  const [progressPercent, setProgressPercent] = useState<number>(0);
  const [transferredFiles, setTransferredFiles] = useState<number>(0);
  const [totalFiles, setTotalFiles] = useState<number | null>(null);
  const [currentPhase, setCurrentPhase] = useState<string>('');
  const [currentFile, setCurrentFile] = useState<string>('');
  const [transferSpeed, setTransferSpeed] = useState<number | null>(null);
  const [elapsedTime, setElapsedTime] = useState<number>(0);

  // アプリの実バージョンを取得してバッジに表示（真実の源: tauri.conf.json）
  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion(''));
  }, []);

  // PIN認証の状態を監視し、必要に応じて認証モーダルを表示
  useEffect(() => {
    if (!isLoading && isPinEnabled && !isAuthenticated) {
      setShowPinAuth(true);
    } else {
      setShowPinAuth(false);
    }
  }, [isLoading, isPinEnabled, isAuthenticated]);

  // バックアップ進捗イベントリスナーを設定
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<BackupProgress>('backup-progress', (event) => {
          const progress = event.payload;

          setCurrentPhase(progress.phase);
          setTransferredFiles(progress.transferred_files);
          setCurrentFile(progress.current_file || '');
          setTransferSpeed(progress.transfer_speed || null);
          setElapsedTime(progress.elapsed_seconds);

          if (progress.total_files) {
            setTotalFiles(progress.total_files);
            const percent = (progress.transferred_files / progress.total_files) * 100;
            setProgressPercent(Math.min(percent, 100));
          } else {
            // 総ファイル数が不明の場合は、転送ファイル数に基づいて仮の進捗を表示
            const baseProgress = Math.min(progress.transferred_files * 2, 100);
            setProgressPercent(baseProgress);
          }
        });
      } catch (error) {
        console.error('進捗イベントリスナーの設定に失敗しました:', error);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const handlePinAuthSuccess = () => {
    setShowPinAuth(false);
  };

  const handleFolderSelection = async () => {
    try {
      // フォルダ選択ダイアログを開く
      const selected = await openDialog({
        directory: true,
        multiple: false,
      });

      if (selected) {
        setSelectedLocalFolder(selected);
      }
    } catch (error) {
      console.error('フォルダ選択エラー:', error);
      // ダイアログが開けない場合は何もしない（ユーザーに手動入力してもらう）
    }
  };

  const handleKeyFileSelection = async () => {
    try {
      // ファイル選択ダイアログを開く
      const selected = await openDialog({
        multiple: false,
        filters: [
          {
            name: 'SSH Private Keys',
            extensions: ['key', 'pem', 'ppk', 'openssh']
          },
          {
            name: 'All Files',
            extensions: ['*']
          }
        ]
      });

      if (selected) {
        setSelectedKeyPath(selected);
      }
    } catch (error) {
      console.error('ファイル選択エラー:', error);
      // ダイアログが開けない場合は何もしない（ユーザーに手動入力してもらう）
    }
  };

  const handleConnectionTest = async () => {
    if (!selectedKeyPath) {
      alert('SSH秘密鍵ファイルを選択してください');
      return;
    }

    setConnectionStatus('X-Server接続テスト中...');

    try {
      const result = await invoke<string>('test_xserver_connection', {
        keyPath: selectedKeyPath
      });
      setConnectionStatus(result);
    } catch (error) {
      setConnectionStatus('❌ ' + String(error));
    }
  };

  const handleDomainSearch = async () => {
    if (!selectedKeyPath) {
      alert('SSH秘密鍵ファイルを選択してください');
      return;
    }

    setIsDomainSearching(true);
    setAvailableDomains([]);

    try {
      const domains = await invoke<string[]>('find_xserver_domains', {
        keyPath: selectedKeyPath
      });
      setAvailableDomains(domains);

      if (domains.length === 0) {
        alert('利用可能なドメインが見つかりませんでした');
      }
    } catch (error) {
      alert('ドメイン探索に失敗しました: ' + String(error));
    } finally {
      setIsDomainSearching(false);
    }
  };

  const handleBackupStart = async () => {
    if (!selectedLocalFolder) {
      alert('バックアップ先フォルダを選択してください');
      return;
    }

    if (!selectedKeyPath) {
      alert('SSH秘密鍵ファイルを選択してください');
      return;
    }

    if (!remoteFolder) {
      alert('リモートフォルダパスを入力してください');
      return;
    }

    setIsBackupRunning(true);
    setBackupResult('');
    setProgressMessage('バックアップを準備中...');
    setProgressPercent(0);
    setTransferredFiles(0);
    setTotalFiles(null);
    setCurrentPhase('');
    setCurrentFile('');
    setTransferSpeed(null);
    setElapsedTime(0);

    try {
      setProgressMessage('X-Server SSH接続中...');
      setProgressPercent(10);

      // 仮の進捗更新（実際のイベントが来るまでの暫定）
      setProgressMessage('ファイル探索中...');
      setProgressPercent(20);

      // 少し待ってから転送開始の表示
      setTimeout(() => {
        setProgressMessage('ファイル転送中...');
        setProgressPercent(30);
      }, 500);

      const result = await invoke<BackupResult>('backup_xserver_folder', {
        keyPath: selectedKeyPath,
        remoteFolder: remoteFolder,
        localFolder: selectedLocalFolder,
      });

      // 時間を分:秒形式でフォーマット
      const minutes = Math.floor(result.elapsed_seconds / 60);
      const seconds = result.elapsed_seconds % 60;
      const timeFormat = minutes > 0 ? `${minutes}分${seconds}秒` : `${seconds}秒`;

      setProgressPercent(100);
      setTransferredFiles(result.transferred_files);
      setBackupResult(`${result.message}
📊 転送ファイル数: ${result.transferred_files.toLocaleString()}個
⏱️ 実行時間: ${timeFormat}`);
      setProgressMessage('バックアップ完了');

    } catch (error) {
      const errorMessage = String(error);
      if (errorMessage.includes('🚫 バックアップがキャンセルされました')) {
        setBackupResult('🚫 バックアップがキャンセルされました');
        setProgressMessage('バックアップ中止');
        setProgressPercent(0);
      } else {
        setBackupResult(`❌ エラー: ${errorMessage}`);
        setProgressMessage('バックアップ失敗');
        setProgressPercent(0);
      }
    } finally {
      setIsBackupRunning(false);
    }
  };

  const handleBackupCancel = async () => {
    try {
      await invoke('cancel_backup');
      setProgressMessage('キャンセル中...');
    } catch (error) {
      console.error('バックアップキャンセルエラー:', error);
    }
  };

  // ローディング画面
  if (isLoading) {
    return (
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        height: '100vh',
        backgroundColor: '#f5f5f5'
      }}>
        <div style={{ textAlign: 'center' }}>
          <div style={{ fontSize: '3rem', marginBottom: '1rem' }}>🔄</div>
          <p style={{ fontSize: '1.2rem', color: '#666' }}>アプリケーション起動中...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <header className="modern-header">
        <div className="header-content">
          <div className="header-icon">
            <Rocket className="w-12 h-12 text-white" />
          </div>
          <div className="header-text">
            <h1 className="header-title">Kyosho Backup</h1>
            <p className="header-subtitle">for X-Server</p>
            <p className="header-description">プロフェッショナルなサーバーバックアップソリューション</p>
          </div>
          <div className="header-nav">
            <div className="nav-links">
              <a href="/settings" className="nav-link">
                <Settings className="w-4 h-4" />
                設定
              </a>
              <a href="/history" className="nav-link">
                <BarChart3 className="w-4 h-4" />
                履歴
              </a>
            </div>
            <div className="pin-status">
              {isPinEnabled ? (
                <div className="pin-indicator enabled">
                  <Lock className="w-4 h-4" />
                  PIN有効
                </div>
              ) : (
                <div className="pin-indicator disabled">
                  <Unlock className="w-4 h-4" />
                  PIN無効
                </div>
              )}
            </div>
          </div>
        </div>
        <div className="header-badge">
          <span className="version-badge">{appVersion ? `v${appVersion}` : ''}</span>
        </div>
      </header>

      <main className="settings-content">
        <section className="modern-card">
          <div className="card-header">
            <Server className="w-6 h-6 text-blue-600" />
            <h2 className="card-title">接続先サーバー</h2>
          </div>
          <div className="server-info-content">
            <div className="info-item">
              <span className="info-label">ホスト</span>
              <span className="info-value">sv8187.xserver.jp:10022</span>
            </div>
            <div className="info-item">
              <span className="info-label">ユーザー</span>
              <span className="info-value">funnybooth</span>
            </div>
            <div className="info-item">
              <span className="info-label">サービス</span>
              <span className="info-value">X-Server SFTP/SSH</span>
            </div>
          </div>
        </section>

        <section className="modern-card">
          <div className="card-header">
            <Key className="w-6 h-6 text-green-600" />
            <h2 className="card-title">SSH秘密鍵の選択</h2>
          </div>
          <div className="input-group">
            <div className="file-input-group">
              <input
                type="text"
                value={selectedKeyPath}
                onChange={(e) => setSelectedKeyPath(e.target.value)}
                placeholder="SSH秘密鍵ファイル（/Users/username/Downloads/private.key）"
                className="folder-input"
              />
              <button
                onClick={handleKeyFileSelection}
                className="select-button"
                disabled={isBackupRunning}
              >
                参照
              </button>
            </div>
            {selectedKeyPath && (
              <button
                onClick={handleConnectionTest}
                className="test-button"
                disabled={isBackupRunning}
                style={{ marginTop: '0.5rem' }}
              >
<Search className="w-4 h-4" />
                接続テスト
              </button>
            )}
          </div>
          {connectionStatus && (
            <div style={{
              marginTop: '1rem',
              padding: '0.75rem',
              background: connectionStatus.includes('✅') ? '#e8f5e8' : '#ffebee',
              borderRadius: '0.5rem',
              fontSize: '0.9rem'
            }}>
              {connectionStatus}
            </div>
          )}
        </section>

        <section className="modern-card">
          <div className="card-header">
            <Folder className="w-6 h-6 text-orange-600" />
            <h2 className="card-title">バックアップ対象フォルダ</h2>
          </div>

          {/* プルダウンによるバックアップ対象選択 - 常に表示 */}
          <div style={{ marginBottom: '1rem' }}>
            <p style={{ margin: '0 0 0.5rem 0', fontWeight: 'bold' }}>バックアップ対象を選択:</p>
            <select
              value={remoteFolder}
              onChange={(e) => setRemoteFolder(e.target.value)}
              className="styled-dropdown"
              disabled={isBackupRunning}
            >
              <option value="">-- バックアップ対象を選択してください --</option>
              <option value="/home/funnybooth/kyosho-eco.net/">共ショウecoネット</option>
              <option value="/home/funnybooth/kyosho.nagoya/">共ショウnet</option>
              <option value="/home/funnybooth/bouon-boushin.net/public_html/">防音防振ネット！</option>

              {/* 探索されたドメインも選択肢に追加 */}
              {availableDomains.length > 0 && availableDomains.map((domain, index) => {
                // 固定オプションと重複しない場合のみ追加
                const fixedPaths = [
                  '/home/funnybooth/kyosho-eco.net/',
                  '/home/funnybooth/kyosho.nagoya/',
                  '/home/funnybooth/bouon-boushin.net/public_html/'
                ];
                if (!fixedPaths.includes(domain)) {
                  return (
                    <option key={`discovered-${index}`} value={domain}>
                      {domain} (探索で発見)
                    </option>
                  );
                }
                return null;
              })}
            </select>
          </div>

          {selectedKeyPath && (
            <div style={{ marginBottom: '1rem' }}>
              <button
                onClick={handleDomainSearch}
                disabled={isDomainSearching || isBackupRunning}
                className="test-button"
                style={{ marginBottom: '0.5rem' }}
              >
{isDomainSearching ? (
                  <>
                    <Search className="w-4 h-4 animate-spin" />
                    ドメイン探索中...
                  </>
                ) : (
                  <>
                    <Search className="w-4 h-4" />
                    利用可能なドメインを探索
                  </>
                )}
              </button>
            </div>
          )}

          <div className="input-group" style={{ opacity: 0.7 }}>
            <label style={{ fontSize: '0.9rem', color: '#666' }}>
              選択されたパス（参考）:
            </label>
            <input
              type="text"
              value={remoteFolder}
              readOnly
              placeholder="上のプルダウンから選択してください"
              className="folder-input"
            />
          </div>
          <small style={{ color: '#666', fontSize: '0.8rem' }}>
            X-Server上のバックアップしたいディレクトリパスを指定してください
          </small>
        </section>

        <section className="modern-card">
          <div className="card-header">
            <HardDrive className="w-6 h-6 text-purple-600" />
            <h2 className="card-title">ローカル保存先の選択</h2>
          </div>
          <div className="input-group">
            <div className="file-input-group">
              <input
                type="text"
                value={selectedLocalFolder}
                onChange={(e) => setSelectedLocalFolder(e.target.value)}
                placeholder="バックアップファイルを保存するローカルフォルダ（/Users/username/Desktop/backup）"
                className="folder-input"
              />
              <button
                onClick={handleFolderSelection}
                className="select-button"
                disabled={isBackupRunning}
              >
                参照
              </button>
            </div>
          </div>
        </section>

        <section className="modern-card backup-main">
          <div className="card-header">
            <Play className="w-6 h-6 text-red-600" />
            <h2 className="card-title">バックアップ実行</h2>
          </div>

          <div style={{ marginBottom: '1rem', padding: '1rem', background: '#f8f9fa', borderRadius: '0.5rem' }}>
            <p><strong>リモート:</strong> funnybooth@sv8187.xserver.jp</p>
            <p><strong>対象フォルダ:</strong> {remoteFolder || '（未指定）'}</p>
            <p><strong>保存先:</strong> {selectedLocalFolder || '（未選択）'}</p>
          </div>

          <div className="backup-actions">
            <button
              onClick={handleBackupStart}
              disabled={isBackupRunning || !selectedLocalFolder || !selectedKeyPath || !remoteFolder}
              className={`backup-button ${isBackupRunning ? 'running' : ''}`}
            >
              {isBackupRunning ? (
                <>
                  <Download className="w-5 h-5 animate-pulse" />
                  バックアップ実行中...
                </>
              ) : (
                <>
                  <Rocket className="w-5 h-5" />
                  バックアップ開始
                </>
              )}
            </button>

            {isBackupRunning && (
              <button
                onClick={handleBackupCancel}
                className="cancel-button"
              >
                <Square className="w-4 h-4" />
                停止
              </button>
            )}
          </div>

          {isBackupRunning && (
            <div className="progress-section">
              <div className="progress-bar">
                <div
                  className="progress-fill flowing"
                  style={{
                    width: `${progressPercent}%`,
                    transition: 'width 0.5s ease-in-out'
                  }}
                ></div>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '0.5rem' }}>
                <div>
                  <p className="progress-text" style={{ margin: 0, fontSize: '1rem', fontWeight: '600' }}>
                    {currentPhase || progressMessage}
                  </p>
                  {currentFile && (
                    <p style={{ margin: '0.25rem 0 0 0', fontSize: '0.8rem', color: '#666', fontFamily: 'monospace' }}>
                      {currentFile.length > 60 ? `...${currentFile.slice(-60)}` : currentFile}
                    </p>
                  )}
                </div>
                <div style={{ textAlign: 'right', fontSize: '0.9rem', color: '#666' }}>
                  <div>
                    {transferredFiles > 0 && (
                      <span>転送済み: {transferredFiles.toLocaleString()}ファイル</span>
                    )}
                    {totalFiles && (
                      <span> / {totalFiles.toLocaleString()}ファイル</span>
                    )}
                    <span style={{ marginLeft: '1rem', fontWeight: 'bold' }}>{progressPercent.toFixed(0)}%</span>
                  </div>
                  <div style={{ marginTop: '0.25rem', fontSize: '0.8rem' }}>
                    {elapsedTime > 0 && (
                      <span>経過時間: {Math.floor(elapsedTime / 60)}分{elapsedTime % 60}秒</span>
                    )}
                    {transferSpeed && transferSpeed > 0 && (
                      <span style={{ marginLeft: '1rem' }}>{transferSpeed.toFixed(1)} MB/s</span>
                    )}
                  </div>
                </div>
              </div>
            </div>
          )}

          {backupResult && (
            <div style={{
              marginTop: '1rem',
              padding: '1rem',
              background: backupResult.includes('❌') ? '#ffebee' : '#e8f5e8',
              borderRadius: '0.5rem',
              whiteSpace: 'pre-wrap'
            }}>
              {backupResult}
            </div>
          )}
        </section>
      </main>

      <footer className="footer">
        <p>© 2025 Kyosho Backup - Cross-platform Server Backup Tool</p>
      </footer>

      {/* PIN認証モーダル */}
      <PinAuthModal
        isOpen={showPinAuth}
        mode="verify"
        onSuccess={handlePinAuthSuccess}
        onCancel={() => {}} // キャンセル不可（認証必須）
        title="🔐 アプリケーション認証"
      />
    </div>
  );
};

export default MainPage;