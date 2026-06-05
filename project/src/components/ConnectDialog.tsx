import React, { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ConnectionState } from '../types';

interface ConnectDialogProps {
  onConnect: (peerId: string, password: string, asHost: boolean) => Promise<void>;
  connectionState: ConnectionState;
}

type ConnectionMode = 'host' | 'client';

export function ConnectDialog({ onConnect, connectionState }: ConnectDialogProps) {
  const [mode, setMode] = useState<ConnectionMode>('client');
  const [peerId, setPeerId] = useState('');
  const [password, setPassword] = useState('');
  const [generatedPassword, setGeneratedPassword] = useState('');
  const [error, setError] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Generate a session password when switching to host mode
  useEffect(() => {
    if (mode === 'host') {
      invoke<string>('generate_session_password')
        .then((pw) => {
          setGeneratedPassword(pw);
          setPassword(pw);
        })
        .catch((e) => setError(`Fehler beim Generieren: ${e}`));
    } else {
      setGeneratedPassword('');
      setPassword('');
    }
    setError('');
  }, [mode]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError('');

      if (!peerId.trim()) {
        setError('Bitte eine Peer-ID eingeben.');
        return;
      }

      if (mode === 'client' && !password.trim()) {
        setError('Bitte ein Passwort eingeben.');
        return;
      }

      setIsSubmitting(true);
      try {
        await onConnect(peerId.trim(), password, mode === 'host');
      } catch (e) {
        setError(`Verbindung fehlgeschlagen: ${e}`);
      } finally {
        setIsSubmitting(false);
      }
    },
    [mode, peerId, password, onConnect]
  );

  const handleCopyPassword = useCallback(() => {
    if (generatedPassword) {
      navigator.clipboard.writeText(generatedPassword);
    }
  }, [generatedPassword]);

  const getStatusText = () => {
    switch (connectionState) {
      case 'connecting':
        return 'Verbindung wird hergestellt...';
      case 'connected':
        return 'Verbunden!';
      case 'disconnected':
        return 'Verbindung getrennt.';
      default:
        return 'Bereit zum Verbinden.';
    }
  };

  const getStatusDotClass = () => {
    switch (connectionState) {
      case 'connecting':
        return 'connecting';
      case 'connected':
        return 'connected';
      case 'disconnected':
        return 'error';
      default:
        return '';
    }
  };

  return (
    <div className="connect-dialog">
      <div className="connect-dialog-content">
        <h2>ClawViewer</h2>
        <p className="subtitle">KI-gestuetzter Remote Desktop</p>

        <div className="connection-mode-tabs">
          <button
            className={mode === 'client' ? 'active' : ''}
            onClick={() => setMode('client')}
            type="button"
          >
            Verbinden
          </button>
          <button
            className={mode === 'host' ? 'active' : ''}
            onClick={() => setMode('host')}
            type="button"
          >
            Hosten
          </button>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Peer-ID</label>
            <input
              type="text"
              placeholder="z.B. claw-desktop-01"
              value={peerId}
              onChange={(e) => setPeerId(e.target.value)}
              disabled={isSubmitting}
              autoFocus
            />
          </div>

          {mode === 'host' ? (
            <div className="form-group">
              <label>Session-Passwort</label>
              <div className="password-display">
                <code>{generatedPassword || '...'}</code>
                <button
                  type="button"
                  className="secondary"
                  onClick={handleCopyPassword}
                  disabled={!generatedPassword}
                >
                  Kopieren
                </button>
              </div>
              <p style={{ fontSize: '12px', color: '#888', marginTop: '6px' }}>
                Teile diese Peer-ID und das Passwort mit deinem Partner.
              </p>
            </div>
          ) : (
            <div className="form-group">
              <label>Passwort</label>
              <input
                type="password"
                placeholder="Session-Passwort eingeben"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                disabled={isSubmitting}
              />
            </div>
          )}

          {error && (
            <p
              style={{
                color: '#e94560',
                fontSize: '13px',
                marginBottom: '12px',
                padding: '8px',
                background: 'rgba(233, 69, 96, 0.1)',
                borderRadius: '6px',
              }}
            >
              {error}
            </p>
          )}

          <button
            type="submit"
            className="primary"
            disabled={isSubmitting}
            style={{ width: '100%', padding: '12px' }}
          >
            {isSubmitting
              ? 'Verbinden...'
              : mode === 'host'
                ? 'Session starten'
                : 'Verbinden'}
          </button>
        </form>

        <div className="status-indicator">
          <span className={`status-dot ${getStatusDotClass()}`} />
          <span>{getStatusText()}</span>
        </div>
      </div>
    </div>
  );
}
