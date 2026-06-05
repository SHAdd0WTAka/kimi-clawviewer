import React, { useState, useRef, useEffect, useCallback } from 'react';
import type { ChatMessage, AIMode } from '../types';

interface ChatPanelProps {
  messages: ChatMessage[];
  onSendMessage: (content: string) => Promise<void>;
  aiMode: AIMode;
  peerId: string;
}

export function ChatPanel({ messages, onSendMessage, aiMode, peerId }: ChatPanelProps) {
  const [input, setInput] = useState('');
  const [isSending, setIsSending] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!input.trim() || isSending) return;

      setIsSending(true);
      try {
        await onSendMessage(input.trim());
        setInput('');
      } catch (e) {
        console.error('Send error:', e);
      } finally {
        setIsSending(false);
      }
    },
    [input, isSending, onSendMessage]
  );

  const formatTime = (timestamp: number) => {
    const d = new Date(timestamp);
    return d.toLocaleTimeString('de-DE', {
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const getAIModeLabel = (): string => {
    switch (aiMode) {
      case 'observer':
        return 'Beobachter';
      case 'shared':
        return 'Geteilt';
      case 'full':
        return 'Voll';
      case 'disabled':
        return 'Deaktiviert';
    }
  };

  const getAIModeColor = (): string => {
    switch (aiMode) {
      case 'observer':
        return '#3498db';
      case 'shared':
        return '#f39c12';
      case 'full':
        return '#e74c3c';
      case 'disabled':
        return '#555';
    }
  };

  return (
    <div className="chat-panel">
      {/* Header */}
      <div className="chat-header">
        <div>
          <h3>Chat & Log</h3>
          <div
            style={{
              fontSize: '11px',
              color: '#888',
              marginTop: '2px',
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
            }}
          >
            <span>Peer: {peerId || '–'}</span>
            <span
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
              }}
            >
              <span
                style={{
                  width: '6px',
                  height: '6px',
                  borderRadius: '50%',
                  background: getAIModeColor(),
                  display: 'inline-block',
                }}
              />
              KI: {getAIModeLabel()}
            </span>
          </div>
        </div>
      </div>

      {/* Messages */}
      <div className="chat-messages">
        {messages.length === 0 && (
          <div
            style={{
              textAlign: 'center',
              color: '#555',
              fontSize: '13px',
              marginTop: '20px',
              fontStyle: 'italic',
            }}
          >
            Noch keine Nachrichten.
            <br />
            Schreibe etwas, um die Unterhaltung zu beginnen.
          </div>
        )}

        {messages.map((msg) => (
          <div key={msg.id} className={`chat-message ${msg.type}`}>
            <div className="message-header">
              <span style={{ fontWeight: 600 }}>
                {msg.type === 'ai-action'
                  ? '\u{1F916} KI-Aktion'
                  : msg.type === 'human-override'
                    ? '\u270B Mensch'
                    : msg.type === 'system'
                      ? '\u26A0 System'
                      : msg.sender}
              </span>
              <span>{formatTime(msg.timestamp)}</span>
            </div>
            <div>{msg.content}</div>
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <form className="chat-input-area" onSubmit={handleSubmit}>
        <input
          ref={inputRef}
          type="text"
          placeholder="Nachricht schreiben..."
          value={input}
          onChange={(e) => setInput(e.target.value)}
          disabled={isSending}
        />
        <button
          type="submit"
          className="primary"
          disabled={!input.trim() || isSending}
          style={{ padding: '8px 14px' }}
        >
          {isSending ? '...' : '\u25B6'}
        </button>
      </form>
    </div>
  );
}
