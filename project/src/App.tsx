import { useState, useCallback, useEffect } from 'react';
import { invoke, listen } from './tauriApi';
import { ScreenViewer } from './components/ScreenViewer';
import { ControlBar } from './components/ControlBar';
import { ChatPanel } from './components/ChatPanel';
import { ConnectDialog } from './components/ConnectDialog';
import { AIStatus } from './components/AIStatus';
import { useWebRTC } from './hooks/useWebRTC';
import type { AIMode, ChatMessage } from './types';
import './App.css';

export default function App() {
  const { connectionState, connect, disconnect } = useWebRTC();
  const [connected, setConnected] = useState(false);
  const [aiMode, setAiMode] = useState<AIMode>('disabled');
  const [showChat, setShowChat] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [peerId, setPeerId] = useState('');

  // Listen for connection events from Tauri
  useEffect(() => {
    const unlistenConnected = listen('peer-connected', () => {
      setConnected(true);
    });

    const unlistenDisconnected = listen('peer-disconnected', () => {
      setConnected(false);
      setAiMode('disabled');
    });

    const unlistenMessage = listen<{ message: ChatMessage }>('chat-message', (payload) => {
      setMessages((prev) => [...prev, payload.message]);
    });

    const unlistenAiMode = listen<{ mode: AIMode }>('ai-mode-changed', (payload) => {
      setAiMode(payload.mode);
    });

    return () => {
      unlistenConnected.then((fn) => fn());
      unlistenDisconnected.then((fn) => fn());
      unlistenMessage.then((fn) => fn());
      unlistenAiMode.then((fn) => fn());
    };
  }, []);

  const handleConnect = useCallback(
    async (id: string, password: string, asHost: boolean) => {
      setPeerId(id);
      try {
        if (asHost) {
          await invoke('start_host_session', { password });
        }
        await connect(id, password);
        setConnected(true);
      } catch (e) {
        console.error('Connection failed:', e);
        throw e;
      }
    },
    [connect]
  );

  const handleDisconnect = useCallback(async () => {
    await disconnect();
    setConnected(false);
    setAiMode('disabled');
    setPeerId('');
  }, [disconnect]);

  const handleEmergencyStop = useCallback(async () => {
    try {
      await invoke('emergency_stop');
      setAiMode('disabled');
      setMessages((prev) => [
        ...prev,
        {
          id: `sys-${Date.now()}`,
          type: 'system',
          sender: 'System',
          content: 'NOTAUS aktiviert – Alle Vorgaenge gestoppt.',
          timestamp: Date.now(),
        },
      ]);
    } catch (e) {
      console.error('Emergency stop failed:', e);
    }
  }, []);

  const handleAiModeChange = useCallback(async (mode: AIMode) => {
    try {
      await invoke('set_ai_mode', { mode });
      setAiMode(mode);
    } catch (e) {
      console.error('Failed to set AI mode:', e);
    }
  }, []);

  const handleSendMessage = useCallback(async (content: string) => {
    const msg: ChatMessage = {
      id: `msg-${Date.now()}`,
      type: 'chat',
      sender: 'Du',
      content,
      timestamp: Date.now(),
    };
    setMessages((prev) => [...prev, msg]);
    try {
      await invoke('send_chat_message', { content });
    } catch (e) {
      console.error('Failed to send message:', e);
    }
  }, []);

  return (
    <div className="app-container">
      {!connected && (
        <ConnectDialog
          onConnect={handleConnect}
          connectionState={connectionState}
        />
      )}
      {connected && (
        <>
          <ScreenViewer aiMode={aiMode} peerId={peerId} />
          <AIStatus mode={aiMode} />
          <ControlBar
            aiMode={aiMode}
            onAiModeChange={handleAiModeChange}
            onEmergencyStop={handleEmergencyStop}
            onToggleChat={() => setShowChat(!showChat)}
            onDisconnect={handleDisconnect}
            connectionState={connectionState}
          />
          {showChat && (
            <ChatPanel
              messages={messages}
              onSendMessage={handleSendMessage}
              aiMode={aiMode}
              peerId={peerId}
            />
          )}
        </>
      )}
    </div>
  );
}
