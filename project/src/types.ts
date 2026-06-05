/**
 * Shared types for ClawViewer frontend
 */

export type AIMode = 'observer' | 'shared' | 'full' | 'disabled';

export type ConnectionState = 'new' | 'connecting' | 'connected' | 'disconnected';

export type MessageType = 'chat' | 'system' | 'ai-action' | 'human-override';

export interface ChatMessage {
  id: string;
  type: MessageType;
  sender: string;
  content: string;
  timestamp: number;
}

export interface AIActivity {
  mode: AIMode;
  isActive: boolean;
  confidence?: number;
  currentAction?: string;
}

export interface ConnectionConfig {
  peerId: string;
  password: string;
  isHost: boolean;
}

export interface SessionInfo {
  sessionId: string;
  peerId: string;
  connectedAt: number;
  aiMode: AIMode;
}
