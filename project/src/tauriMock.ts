// Mock für Tauri API im Browser (ohne Desktop-App)
// Ersetzt @tauri-apps/api/core und @tauri-apps/api/event

export type MockAIMode = 'disabled' | 'suggest' | 'autonomous';

interface MockSession {
  peerId: string;
  password: string;
  mode: MockAIMode;
  connected: boolean;
}

let mockSession: MockSession | null = null;
let eventListeners: Map<string, Array<(payload: any) => void>> = new Map();

// Simulierte Passwort-Generierung (wie in cv-security)
function generateMockPassword(): string {
  const words = ['ace','act','add','age','ago','aid','air','all','and','any','ape','apt','are','arm','art','ash','ask','ate','awe','axe','bad','bag','ban','bar','bat','bay','bed','bet','big','bit','bow','box','boy','bug','bus','but','buy','bye','cab','can','cap','car','cat','cop','cow','cry','cup','cut','dad','day','did','die','dig','dim','dip','dog','dot','dry','dub','due','dug','ear','eat','egg','ego','elf','elk','elm','end','era','eve','eye','fan','far','fat','fax','fee','few','fit','fix','flu','fly','fog','foo','for','fox','fry','fun','gag','gap','gas','gem','get','gig','god','got','gum','gun','guy','gym'];
  const word = words[Math.floor(Math.random() * words.length)];
  const num = Math.floor(Math.random() * 1000).toString().padStart(3, '0');
  return word + num;
}

// Simulierte Peer-ID-Generierung
function generateMockPeerId(): string {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let id = 'claw-';
  for (let i = 0; i < 8; i++) {
    id += chars[Math.floor(Math.random() * chars.length)];
  }
  return id;
}

// Event-System
function emitEvent(eventName: string, payload: any) {
  const listeners = eventListeners.get(eventName) || [];
  listeners.forEach(cb => cb(payload));
}

// Mock invoke
export async function invoke<T>(cmd: string, args?: Record<string, any>): Promise<T> {
  console.log(`[TAURI MOCK] invoke(${cmd})`, args);

  switch (cmd) {
    case 'generate_session_password':
      return generateMockPassword() as unknown as T;

    case 'start_host_session':
      mockSession = {
        peerId: generateMockPeerId(),
        password: args?.password || generateMockPassword(),
        mode: 'disabled',
        connected: false,
      };
      // Simuliere Verbindung nach kurzer Verzögerung
      setTimeout(() => {
        if (mockSession) {
          mockSession.connected = true;
          emitEvent('peer-connected', {});
          emitEvent('connection-state-changed', { state: 'connected' });
        }
      }, 1500);
      return undefined as unknown as T;

    case 'connect_to_peer':
      mockSession = {
        peerId: args?.peerId || 'unknown',
        password: args?.password || '',
        mode: 'disabled',
        connected: false,
      };
      setTimeout(() => {
        if (mockSession) {
          mockSession.connected = true;
          emitEvent('peer-connected', {});
          emitEvent('connection-state-changed', { state: 'connected' });
        }
      }, 1500);
      return undefined as unknown as T;

    case 'disconnect':
      if (mockSession) {
        mockSession.connected = false;
        emitEvent('peer-disconnected', {});
        emitEvent('connection-state-changed', { state: 'disconnected' });
        mockSession = null;
      }
      return undefined as unknown as T;

    case 'emergency_stop':
      emitEvent('chat-message', {
        message: {
          id: `sys-${Date.now()}`,
          type: 'system',
          sender: 'System',
          content: 'NOTAUS aktiviert – Alle Vorgänge gestoppt.',
          timestamp: Date.now(),
        }
      });
      return undefined as unknown as T;

    case 'set_ai_mode':
      if (mockSession) {
        mockSession.mode = args?.mode || 'disabled';
      }
      emitEvent('ai-mode-changed', { mode: args?.mode || 'disabled' });
      return undefined as unknown as T;

    case 'send_chat_message':
      // Echo die Nachricht als "KI-Antwort"
      setTimeout(() => {
        emitEvent('chat-message', {
          message: {
            id: `ai-${Date.now()}`,
            type: 'chat',
            sender: 'KI-Assistent',
            content: `Echo: ${args?.content || ''}`,
            timestamp: Date.now(),
          }
        });
      }, 500);
      return undefined as unknown as T;

    case 'start_capture':
    case 'stop_capture':
      console.log(`[TAURI MOCK] ${cmd} - no-op in web mode`);
      return undefined as unknown as T;

    case 'send_input_event':
      console.log(`[TAURI MOCK] Input event:`, args);
      return undefined as unknown as T;

    default:
      console.warn(`[TAURI MOCK] Unbekannter Command: ${cmd}`);
      return undefined as unknown as T;
  }
}

// Mock listen
export async function listen<T>(eventName: string, handler: (event: { payload: T }) => void): Promise<() => void> {
  console.log(`[TAURI MOCK] listen(${eventName})`);

  if (!eventListeners.has(eventName)) {
    eventListeners.set(eventName, []);
  }

  const wrappedHandler = (payload: T) => handler({ payload });
  eventListeners.get(eventName)!.push(wrappedHandler);

  // Return unlisten function
  return () => {
    const listeners = eventListeners.get(eventName) || [];
    const idx = listeners.indexOf(wrappedHandler);
    if (idx !== -1) listeners.splice(idx, 1);
  };
}

// Mock für UnlistenFn type
export type UnlistenFn = () => void;
