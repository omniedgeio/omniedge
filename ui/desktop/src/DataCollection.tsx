import { useState, useEffect, useCallback, useRef } from 'react';
import './App.css';
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Robot Data Collection types
interface DataCollectionStatus {
  initialized: boolean;
  recording: boolean;
  robot_id: string;
  current_episode_id: string | null;
  recording_started_at: string | null;
  total_episodes: number;
  storage_used_bytes: number;
}

interface StreamInfo {
  stream_id: string;
  sample_count: number;
  capacity: number;
  utilization_percent: number;
}

interface EpisodeSummary {
  episode_id: string;
  robot_id: string;
  created_at: string;
  duration_secs: number;
  size_bytes: number;
  sample_count: number;
  status: string;
  upload_status: string | null;
}

interface UploadStatus {
  queued: number;
  active: number;
  bytes_uploaded: number;
}

// Helper functions
const formatBytes = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
};

const formatDuration = (seconds: number): string => {
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}m ${secs}s`;
};

function DataCollection() {
  // Refs for auto-resize
  const windowRef = useRef<HTMLDivElement>(null);
  const lastHeightRef = useRef<number>(0);

  // State
  const [dataCollectionStatus, setDataCollectionStatus] = useState<DataCollectionStatus | null>(null);
  const [dataCollectionStreams, setDataCollectionStreams] = useState<StreamInfo[]>([]);
  const [dataCollectionEpisodes, setDataCollectionEpisodes] = useState<EpisodeSummary[]>([]);
  const [dataCollectionLoading, setDataCollectionLoading] = useState(false);
  const [dataCollectionError, setDataCollectionError] = useState<string | null>(null);
  const [uploadStatus, setUploadStatus] = useState<UploadStatus | null>(null);
  const [selectedEpisodeId, setSelectedEpisodeId] = useState<string | null>(null);
  const [episodePage, setEpisodePage] = useState(0);
  const [initRobotId, setInitRobotId] = useState('');
  const [initDataDir, setInitDataDir] = useState('');
  const [showDataCollectionInit, setShowDataCollectionInit] = useState(false);
  
  // Simulation mode for testing without real robot hardware
  const [simulationMode, setSimulationMode] = useState(true);
  const [simulationInitialized, setSimulationInitialized] = useState(false);

  // Auto-resize window to fit content
  const resizeToContent = useCallback(async () => {
    if (windowRef.current) {
      const contentHeight = windowRef.current.offsetHeight;
      
      // Skip if height hasn't changed significantly (within 5px tolerance)
      if (Math.abs(contentHeight - lastHeightRef.current) < 5) {
        return;
      }
      lastHeightRef.current = contentHeight;
      
      try {
        await invoke('resize_data_collection_window', { height: contentHeight });
      } catch (e) {
        console.error('Failed to resize window:', e);
      }
    }
  }, []);

  // Observe content changes and resize window accordingly
  useEffect(() => {
    if (!windowRef.current) return;

    let resizeTimeout: ReturnType<typeof setTimeout>;

    const triggerResize = () => {
      clearTimeout(resizeTimeout);
      resizeTimeout = setTimeout(resizeToContent, 100);
    };

    // ResizeObserver for size changes
    const resizeObserver = new ResizeObserver(triggerResize);
    resizeObserver.observe(windowRef.current);

    // MutationObserver for DOM structure changes
    const mutationObserver = new MutationObserver(triggerResize);
    mutationObserver.observe(windowRef.current, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['class', 'style']
    });

    // Initial resize
    resizeToContent();

    return () => {
      resizeObserver.disconnect();
      mutationObserver.disconnect();
      clearTimeout(resizeTimeout);
    };
  }, [resizeToContent]);

  // Trigger resize on key state changes
  useEffect(() => {
    const timer = setTimeout(resizeToContent, 50);
    return () => clearTimeout(timer);
  }, [simulationMode, simulationInitialized, showDataCollectionInit, dataCollectionStatus, dataCollectionStreams, dataCollectionEpisodes, selectedEpisodeId, dataCollectionError, resizeToContent]);

  // Load status
  const loadDataCollectionStatus = useCallback(async () => {
    try {
      if (simulationMode) {
        const status = await invoke('get_simulation_status') as DataCollectionStatus;
        setDataCollectionStatus(status);
      } else {
        const status = await invoke('get_data_collection_status') as DataCollectionStatus;
        setDataCollectionStatus(status);
      }
    } catch (e: any) {
      console.error('Failed to load data collection status:', e);
    }
  }, [simulationMode]);

  const loadDataCollectionStreams = async () => {
    try {
      if (simulationMode) {
        const streams = await invoke('get_simulation_streams') as StreamInfo[];
        setDataCollectionStreams(streams);
      } else {
        const streams = await invoke('list_data_streams') as StreamInfo[];
        setDataCollectionStreams(streams);
      }
    } catch (e: any) {
      console.error('Failed to load streams:', e);
    }
  };

  const loadDataCollectionEpisodes = async (page = 0) => {
    try {
      if (simulationMode) {
        const episodes = await invoke('get_simulation_episodes', { page, pageSize: 10 }) as any[];
        setDataCollectionEpisodes(episodes.map(e => ({
          episode_id: e.episode_id,
          robot_id: e.robot_id,
          created_at: new Date(e.start_time_ns / 1000000).toISOString(),
          duration_secs: e.duration_seconds,
          size_bytes: e.size_bytes,
          sample_count: e.sample_count,
          status: 'completed',
          upload_status: e.uploaded ? 'uploaded' : null,
        })));
      } else {
        const episodes = await invoke('list_data_episodes', { page, pageSize: 10 }) as EpisodeSummary[];
        setDataCollectionEpisodes(episodes);
      }
      setEpisodePage(page);
    } catch (e: any) {
      console.error('Failed to load episodes:', e);
    }
  };

  const loadUploadStatus = async () => {
    try {
      if (simulationMode) {
        setUploadStatus({ queued: 0, active: 0, bytes_uploaded: 0 });
      } else {
        const status = await invoke('get_data_upload_status') as UploadStatus;
        setUploadStatus(status);
      }
    } catch (e: any) {
      console.error('Failed to load upload status:', e);
    }
  };

  const refreshDataCollection = useCallback(async () => {
    setDataCollectionLoading(true);
    setDataCollectionError(null);
    try {
      await loadDataCollectionStatus();
      await loadDataCollectionStreams();
      await loadDataCollectionEpisodes(episodePage);
      await loadUploadStatus();
    } finally {
      setDataCollectionLoading(false);
    }
  }, [loadDataCollectionStatus, episodePage]);

  // Initialize on mount
  useEffect(() => {
    refreshDataCollection();
    
    // Refresh periodically when recording
    const interval = setInterval(() => {
      if (dataCollectionStatus?.recording) {
        loadDataCollectionStatus();
        loadDataCollectionStreams();
      }
    }, 2000);
    
    return () => clearInterval(interval);
  }, [refreshDataCollection, dataCollectionStatus?.recording, loadDataCollectionStatus]);

  // Handlers
  const handleInitDataCollection = async () => {
    if (!initRobotId.trim()) {
      setDataCollectionError('Robot ID is required');
      return;
    }
    setDataCollectionLoading(true);
    setDataCollectionError(null);
    try {
      if (simulationMode) {
        await invoke('init_simulation_mode', { robotId: initRobotId.trim() });
        setSimulationInitialized(true);
      } else {
        await invoke('init_data_collection', { 
          robotId: initRobotId.trim(), 
          dataDir: initDataDir.trim() || '/var/lib/omniedge/data' 
        });
      }
      setShowDataCollectionInit(false);
      await refreshDataCollection();
    } catch (e: any) {
      setDataCollectionError(`Failed to initialize: ${e.toString()}`);
    } finally {
      setDataCollectionLoading(false);
    }
  };

  const handleStartRecording = async (reason = 'Manual recording') => {
    setDataCollectionLoading(true);
    setDataCollectionError(null);
    try {
      if (simulationMode) {
        await invoke('start_simulation_recording', { reason });
      } else {
        await invoke('start_data_recording', { reason });
      }
      await loadDataCollectionStatus();
      await loadDataCollectionStreams();
    } catch (e: any) {
      setDataCollectionError(`Failed to start recording: ${e.toString()}`);
    } finally {
      setDataCollectionLoading(false);
    }
  };

  const handleStopRecording = async (discard = false) => {
    setDataCollectionLoading(true);
    setDataCollectionError(null);
    try {
      if (simulationMode) {
        await invoke('stop_simulation_recording', { discard });
      } else {
        await invoke('stop_data_recording', { discard });
      }
      await loadDataCollectionStatus();
      await loadDataCollectionEpisodes(0);
    } catch (e: any) {
      setDataCollectionError(`Failed to stop recording: ${e.toString()}`);
    } finally {
      setDataCollectionLoading(false);
    }
  };

  const handleDeleteEpisode = async (episodeId: string) => {
    setDataCollectionLoading(true);
    try {
      if (simulationMode) {
        await invoke('delete_simulation_episode', { episodeId });
      } else {
        await invoke('delete_data_episode', { episodeId });
      }
      await loadDataCollectionEpisodes(episodePage);
      setSelectedEpisodeId(null);
    } catch (e: any) {
      setDataCollectionError(`Failed to delete episode: ${e.toString()}`);
    } finally {
      setDataCollectionLoading(false);
    }
  };

  const handleUploadEpisode = async (episodeId: string) => {
    setDataCollectionLoading(true);
    try {
      if (simulationMode) {
        await invoke('upload_simulation_episode', { episodeId });
      } else {
        await invoke('upload_data_episode', { episodeId });
      }
      await loadDataCollectionEpisodes(episodePage);
      await loadUploadStatus();
    } catch (e: any) {
      setDataCollectionError(`Failed to queue upload: ${e.toString()}`);
    } finally {
      setDataCollectionLoading(false);
    }
  };

  const handleClose = async () => {
    const win = getCurrentWindow();
    await win.close();
  };

  return (
    <div className="data-collection-window" ref={windowRef}>
      {/* Window Header */}
      <div className="dc-window-header" data-tauri-drag-region>
        <div className="dc-header-left">
          <span className="dc-title">Data Collection</span>
          {simulationMode && <span className="sim-badge">SIM</span>}
          {dataCollectionStatus?.recording && <span className="recording-badge">REC</span>}
        </div>
        <button className="dc-close-btn" onClick={handleClose}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>

      {/* Content */}
      <div className="dc-window-content">
        {dataCollectionError && (
          <div className="data-collection-error">
            <span>{dataCollectionError}</span>
            <span className="error-dismiss-small" onClick={() => setDataCollectionError(null)}>Dismiss</span>
          </div>
        )}

        {/* Simulation mode toggle */}
        <div className="simulation-toggle-row">
          <span className="simulation-label">Simulation Mode</span>
          <div
            className={`ios-switch small ${simulationMode ? 'on' : ''}`}
            onClick={() => {
              setSimulationMode(!simulationMode);
              setDataCollectionStatus(null);
              setSimulationInitialized(false);
            }}
            role="switch"
            aria-checked={simulationMode}
          >
            <div className="dot"></div>
          </div>
        </div>

        {/* Not initialized - show init form */}
        {(simulationMode ? !simulationInitialized : !dataCollectionStatus?.initialized) && !showDataCollectionInit && (
          <div className="data-collection-empty">
            <div className="data-collection-empty-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" opacity="0.3">
                <circle cx="12" cy="12" r="10"></circle>
                <circle cx="12" cy="12" r="3"></circle>
              </svg>
            </div>
            <div className="data-collection-empty-text">
              {simulationMode ? 'Simulation Not Initialized' : 'Data Collection Not Initialized'}
            </div>
            <div className="data-collection-empty-hint">
              {simulationMode ? 'Initialize simulation with a robot ID to test the UI' : 'Configure a robot ID to start collecting data'}
            </div>
            <button 
              className="secondary-btn" 
              onClick={() => setShowDataCollectionInit(true)}
            >
              {simulationMode ? 'Start Simulation' : 'Initialize'}
            </button>
          </div>
        )}

        {/* Init form */}
        {showDataCollectionInit && (
          <div className="data-collection-init-form">
            <div className="form-field">
              <label>Robot ID</label>
              <input
                type="text"
                className="form-input"
                value={initRobotId}
                onChange={(e) => setInitRobotId(e.target.value)}
                placeholder="e.g., robot-001"
              />
            </div>
            <div className="form-field">
              <label>Data Directory (optional)</label>
              <input
                type="text"
                className="form-input"
                value={initDataDir}
                onChange={(e) => setInitDataDir(e.target.value)}
                placeholder="/var/lib/omniedge/data"
              />
            </div>
            <div className="form-actions">
              <button 
                className="secondary-btn" 
                onClick={() => setShowDataCollectionInit(false)}
              >
                Cancel
              </button>
              <button 
                className="primary-login-btn" 
                onClick={handleInitDataCollection}
                disabled={dataCollectionLoading}
              >
                {dataCollectionLoading ? 'Initializing...' : 'Initialize'}
              </button>
            </div>
          </div>
        )}

        {/* Initialized - show dashboard */}
        {(simulationMode ? simulationInitialized : dataCollectionStatus?.initialized) && dataCollectionStatus && (
          <>
            {/* Recording Controls */}
            <div className="recording-controls">
              <div className="recording-status-row">
                <div className="recording-indicator">
                  <div className={`recording-dot ${dataCollectionStatus.recording ? 'recording' : ''}`}></div>
                  <span>{dataCollectionStatus.recording ? 'Recording' : 'Idle'}</span>
                </div>
                <div className="recording-info">
                  <span className="robot-id-badge">{dataCollectionStatus.robot_id}</span>
                </div>
              </div>
              
              {dataCollectionStatus.recording ? (
                <div className="recording-active-controls">
                  <div className="recording-stats">
                    <span>Episode: {dataCollectionStatus.current_episode_id?.slice(0, 8)}...</span>
                  </div>
                  <div className="recording-buttons">
                    <button 
                      className="stop-btn" 
                      onClick={() => handleStopRecording(false)}
                      disabled={dataCollectionLoading}
                    >
                      Stop & Save
                    </button>
                    <button 
                      className="discard-btn" 
                      onClick={() => handleStopRecording(true)}
                      disabled={dataCollectionLoading}
                    >
                      Discard
                    </button>
                  </div>
                </div>
              ) : (
                <button 
                  className="record-btn" 
                  onClick={() => handleStartRecording()}
                  disabled={dataCollectionLoading}
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <circle cx="12" cy="12" r="8"></circle>
                  </svg>
                  Start Recording
                </button>
              )}
            </div>

            {/* Stream Status */}
            {dataCollectionStreams.length > 0 && (
              <div className="streams-section">
                <div className="streams-header">
                  <span className="label-tiny">ACTIVE STREAMS</span>
                  <button className="refresh-btn-small" onClick={loadDataCollectionStreams}>
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                      <path d="M23 4v6h-6"></path>
                      <path d="M1 20v-6h6"></path>
                      <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
                    </svg>
                  </button>
                </div>
                <div className="streams-list">
                  {dataCollectionStreams.map(stream => (
                    <div key={stream.stream_id} className="stream-item">
                      <span className="stream-id">{stream.stream_id}</span>
                      <div className="stream-stats">
                        <span className="stream-samples">{stream.sample_count} samples</span>
                        <div className="stream-utilization">
                          <div 
                            className="utilization-bar" 
                            style={{ width: `${Math.min(stream.utilization_percent, 100)}%` }}
                          ></div>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Episodes List */}
            <div className="episodes-section">
              <div className="episodes-header">
                <span className="label-tiny">RECORDED EPISODES ({dataCollectionStatus.total_episodes})</span>
                <button className="refresh-btn-small" onClick={() => loadDataCollectionEpisodes(episodePage)}>
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                    <path d="M23 4v6h-6"></path>
                    <path d="M1 20v-6h6"></path>
                    <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
                  </svg>
                </button>
              </div>
              
              {dataCollectionEpisodes.length === 0 ? (
                <div className="episodes-empty">No episodes recorded yet</div>
              ) : (
                <>
                  <div className="episodes-list">
                    {dataCollectionEpisodes.map(episode => {
                      const isSelected = selectedEpisodeId === episode.episode_id;
                      return (
                        <div 
                          key={episode.episode_id} 
                          className={`episode-item ${isSelected ? 'is-selected' : ''}`}
                          onClick={() => setSelectedEpisodeId(isSelected ? null : episode.episode_id)}
                        >
                          <div className="episode-main">
                            <div className="episode-id">{episode.episode_id.slice(0, 12)}...</div>
                            <div className="episode-meta">
                              <span>{formatDuration(episode.duration_secs)}</span>
                              <span>{formatBytes(episode.size_bytes)}</span>
                              <span>{episode.sample_count} samples</span>
                            </div>
                          </div>
                          <div className="episode-status">
                            <span className={`episode-status-badge ${episode.status}`}>{episode.status}</span>
                            {episode.upload_status && (
                              <span className={`upload-status-badge ${episode.upload_status}`}>{episode.upload_status}</span>
                            )}
                          </div>
                          
                          {isSelected && (
                            <div className="episode-actions">
                              <button 
                                className="episode-action-btn upload"
                                onClick={(e) => { e.stopPropagation(); handleUploadEpisode(episode.episode_id); }}
                                disabled={dataCollectionLoading || episode.upload_status === 'uploading'}
                              >
                                Upload
                              </button>
                              <button 
                                className="episode-action-btn delete"
                                onClick={(e) => { e.stopPropagation(); handleDeleteEpisode(episode.episode_id); }}
                                disabled={dataCollectionLoading}
                              >
                                Delete
                              </button>
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                  
                  {/* Pagination */}
                  <div className="episodes-pagination">
                    <button 
                      className="pagination-btn"
                      onClick={() => loadDataCollectionEpisodes(episodePage - 1)}
                      disabled={episodePage === 0}
                    >
                      Prev
                    </button>
                    <span className="pagination-info">Page {episodePage + 1}</span>
                    <button 
                      className="pagination-btn"
                      onClick={() => loadDataCollectionEpisodes(episodePage + 1)}
                      disabled={dataCollectionEpisodes.length < 10}
                    >
                      Next
                    </button>
                  </div>
                </>
              )}
            </div>

            {/* Upload Status */}
            {uploadStatus && (uploadStatus.queued > 0 || uploadStatus.active > 0) && (
              <div className="upload-status-section">
                <div className="label-tiny">UPLOAD STATUS</div>
                <div className="upload-stats">
                  <span>Queued: {uploadStatus.queued}</span>
                  <span>Active: {uploadStatus.active}</span>
                  <span>Uploaded: {formatBytes(uploadStatus.bytes_uploaded)}</span>
                </div>
              </div>
            )}

            {/* Storage Info */}
            <div className="storage-info">
              <span className="label-tiny">STORAGE</span>
              <span className="storage-value">{formatBytes(dataCollectionStatus.storage_used_bytes)}</span>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export default DataCollection;
