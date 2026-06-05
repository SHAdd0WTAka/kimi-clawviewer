import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ConnectionState } from '../types';

export function useWebRTC() {
  const [connectionState, setConnectionState] = useState<ConnectionState>('new');
  const unlistenRef = useRef<UnlistenFn | null>(null);

  // Listen for connection state changes from Tauri
  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<{ state: ConnectionState }>(
        'connection-state-changed',
        (event) => {
          setConnectionState(event.payload.state);
        }
      );
      unlistenRef.current = unlisten;
    };

    setupListener();

    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
      }
    };
  }, []);

  /**
   * Connect to a remote peer
   */
  const connect = useCallback(async (peerId: string, password: string) => {
    setConnectionState('connecting');
    try {
      await invoke('connect_to_peer', { peerId, password });
      setConnectionState('connected');
    } catch (e) {
      setConnectionState('disconnected');
      throw e;
    }
  }, []);

  /**
   * Disconnect from the current peer
   */
  const disconnect = useCallback(async () => {
    try {
      await invoke('disconnect');
    } catch (e) {
      console.error('Disconnect error:', e);
    } finally {
      setConnectionState('new');
    }
  }, []);

  /**
   * Generate a new session password for hosting
   */
  const generatePassword = useCallback(async (): Promise<string> => {
    return await invoke<string>('generate_session_password');
  }, []);

  return {
    connectionState,
    connect,
    disconnect,
    generatePassword,
  };
}
