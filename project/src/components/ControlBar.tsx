import React, { useCallback, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AIMode, ConnectionState } from '../types';

interface ControlBarProps {
  aiMode: AIMode;
  onAiModeChange: (mode: AIMode) => Promise<void>;
  onEmergencyStop: () => Promise<void>;
  onToggleChat: () => void;
  onDisconnect: () => Promise<void>;
  connectionState: ConnectionState;
}

export function ControlBar({
  aiMode,
  onAiModeChange,
  onEmergencyStop,
  onToggleChat,
  onDisconnect,
  connectionState,
}: ControlBarProps) {
  const [isCapturing, setIsCapturing] = useState(false);
  const [isStopping, setIsStopping] = useState(false);

  const handleToggleCapture = useCallback(async () => {
    try {
      if (isCapturing) {
        await invoke('stop_capture');
        setIsCapturing(false);
      } else {
        await invoke('start_capture');
        setIsCapturing(true);
      }
    } catch (e) {
      console.error('Capture toggle error:', e);
    }
  }, [isCapturing]);

  const handleEmergency = useCallback(async () => {
    setIsStopping(true);
    try {
      await onEmergencyStop();
    } finally {
      setIsStopping(false);
    }
  }, [onEmergencyStop]);

  const handleAiModeSelect = useCallback(
    async (e: React.ChangeEvent<HTMLSelectElement>) => {
      const mode = e.target.value as AIMode;
      await onAiModeChange(mode);
    },
    [onAiModeChange]
  );

  const getConnectionStatusText = () => {
    switch (connectionState) {
      case 'connected':
        return 'Verbunden';
      case 'connecting':
        return 'Verbinden...';
      case 'disconnected':
        return 'Getrennt';
      default:
        return 'Neu';
    }
  };

  return (
    <div className="control-bar">
      {/* Left: Capture & Connection */}
      <div className="control-group">
        <button
          className={isCapturing ? 'secondary' : 'primary'}
          onClick={handleToggleCapture}
          title={isCapturing ? 'Aufnahme stoppen' : 'Aufnahme starten'}
        >
          {isCapturing ? '\u25A0 Stoppen' : '\u25CF Aufnahme'}
        </button>

        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
            marginLeft: '8px',
            padding: '4px 10px',
            background: '#0f0f1a',
            borderRadius: '6px',
            fontSize: '12px',
          }}
        >
          <span
            style={{
              width: '8px',
              height: '8px',
              borderRadius: '50%',
              background:
                connectionState === 'connected' ? '#2ecc71' : '#e94560',
              display: 'inline-block',
            }}
          />
          <span style={{ color: '#aaa' }}>{getConnectionStatusText()}</span>
        </div>
      </div>

      {/* Center: AI Mode */}
      <div className="control-group" style={{ flex: 1, justifyContent: 'center' }}>
        <span className="control-label">KI-Modus</span>
        <select
          value={aiMode}
          onChange={handleAiModeSelect}
          style={{ width: 'auto', minWidth: '140px', marginBottom: 0 }}
          title="KI-Steuerungsmodus auswaehlen"
        >
          <option value="disabled">Deaktiviert</option>
          <option value="observer">Beobachter</option>
          <option value="shared">Geteilt</option>
          <option value="full">Voll</option>
        </select>
      </div>

      {/* Right: Actions */}
      <div className="control-group">
        <button
          className="icon-btn"
          onClick={onToggleChat}
          title="Chat oeffnen"
        >
          &#x1F4AC;
        </button>

        <button
          className="danger"
          onClick={handleEmergency}
          disabled={isStopping}
          style={{
            fontWeight: 700,
            padding: '8px 20px',
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
          }}
          title="NOTAUS – Alle Vorgaenge sofort stoppen"
        >
          <span style={{ fontSize: '16px' }}>&#x26A0;</span>
          {isStopping ? 'Stoppe...' : 'NOTAUS'}
        </button>

        <button
          className="secondary"
          onClick={onDisconnect}
          title="Verbindung trennen"
          style={{ marginLeft: '8px' }}
        >
          Trennen
        </button>
      </div>
    </div>
  );
}
