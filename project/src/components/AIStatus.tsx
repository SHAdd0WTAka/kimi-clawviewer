import { useEffect, useState } from 'react';
import { listen } from '../tauriApi';
import type { AIMode } from '../types';

interface AIStatusProps {
  mode: AIMode;
}

interface AIActivityEvent {
  isActive: boolean;
  confidence?: number;
  currentAction?: string;
}

export function AIStatus({ mode }: AIStatusProps) {
  const [activity, setActivity] = useState<AIActivityEvent>({
    isActive: false,
    confidence: 0,
    currentAction: undefined,
  });

  useEffect(() => {
    const unlisten = listen<AIActivityEvent>('ai-activity', (payload) => {
      setActivity(payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const getLabel = (): string => {
    switch (mode) {
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

  const getIcon = (): string => {
    switch (mode) {
      case 'observer':
        return '\u{1F441}'; // Eye
      case 'shared':
        return '\u{1F91D}'; // Handshake
      case 'full':
        return '\u{1F916}'; // Robot
      case 'disabled':
        return '\u{1F6AB}'; // Prohibited
    }
  };

  return (
    <div className={`ai-status ${mode}`}>
      <span className="ai-pulse-dot" />
      <span>
        {getIcon()} {getLabel()}
      </span>
      {activity.isActive && mode !== 'disabled' && (
        <span
          style={{
            marginLeft: '8px',
            opacity: 0.8,
            fontWeight: 400,
          }}
        >
          {activity.currentAction
            ? `– ${activity.currentAction}`
            : '– Aktiv'}
          {typeof activity.confidence === 'number' && (
            <span
              style={{
                marginLeft: '6px',
                fontSize: '10px',
                opacity: 0.7,
              }}
            >
              ({Math.round(activity.confidence * 100)}%)
            </span>
          )}
        </span>
      )}
    </div>
  );
}
