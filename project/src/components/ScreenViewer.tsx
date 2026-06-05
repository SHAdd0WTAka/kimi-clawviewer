import React, { useRef, useEffect, useCallback, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AIMode } from '../types';

interface ScreenViewerProps {
  aiMode: AIMode;
  peerId: string;
}

interface GhostCursorPosition {
  x: number;
  y: number;
}

export function ScreenViewer({ aiMode, peerId }: ScreenViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [ghostCursor, setGhostCursor] = useState<GhostCursorPosition | null>(null);
  const [frameSize, setFrameSize] = useState({ width: 0, height: 0 });
  const [hasFrame, setHasFrame] = useState(false);

  // Listen for incoming video frames from Tauri
  useEffect(() => {
    const unlisten = listen<{
      data: number[];
      width: number;
      height: number;
    }>('video-frame', (event) => {
      const { data, width, height } = event.payload;
      const canvas = canvasRef.current;
      if (!canvas) return;

      canvas.width = width;
      canvas.height = height;
      setFrameSize({ width, height });
      setHasFrame(true);

      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      const imageData = new ImageData(
        new Uint8ClampedArray(data),
        width,
        height
      );
      ctx.putImageData(imageData, 0, 0);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen for AI ghost cursor position
  useEffect(() => {
    const unlisten = listen<GhostCursorPosition>('ai-cursor-position', (event) => {
      setGhostCursor(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Forward mouse events to the remote peer via Tauri
  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas || aiMode === 'full') return;

      const rect = canvas.getBoundingClientRect();
      const scaleX = canvas.width / rect.width;
      const scaleY = canvas.height / rect.height;

      const x = Math.round((e.clientX - rect.left) * scaleX);
      const y = Math.round((e.clientY - rect.top) * scaleY);

      invoke('send_input_event', {
        event: {
          type: 'mouse_move',
          x,
          y,
        },
      }).catch((err) => console.error('Mouse move error:', err));
    },
    [aiMode]
  );

  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (aiMode === 'full') return;

      invoke('send_input_event', {
        event: {
          type: 'mouse_down',
          button: e.button,
        },
      }).catch((err) => console.error('Mouse down error:', err));
    },
    [aiMode]
  );

  const handleMouseUp = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (aiMode === 'full') return;

      invoke('send_input_event', {
        event: {
          type: 'mouse_up',
          button: e.button,
        },
      }).catch((err) => console.error('Mouse up error:', err));
    },
    [aiMode]
  );

  const handleWheel = useCallback(
    (e: React.WheelEvent<HTMLCanvasElement>) => {
      if (aiMode === 'full') return;

      invoke('send_input_event', {
        event: {
          type: 'wheel',
          deltaX: e.deltaX,
          deltaY: e.deltaY,
        },
      }).catch((err) => console.error('Wheel error:', err));
    },
    [aiMode]
  );

  // Forward keyboard events
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (aiMode === 'full') return;

      e.preventDefault();
      invoke('send_input_event', {
        event: {
          type: 'key_down',
          key: e.key,
          code: e.code,
          modifiers: {
            ctrl: e.ctrlKey,
            shift: e.shiftKey,
            alt: e.altKey,
            meta: e.metaKey,
          },
        },
      }).catch((err) => console.error('Key down error:', err));
    },
    [aiMode]
  );

  const handleKeyUp = useCallback(
    (e: React.KeyboardEvent) => {
      if (aiMode === 'full') return;

      e.preventDefault();
      invoke('send_input_event', {
        event: {
          type: 'key_up',
          key: e.key,
          code: e.code,
        },
      }).catch((err) => console.error('Key up error:', err));
    },
    [aiMode]
  );

  // Focus canvas on click to capture keyboard events
  const handleCanvasClick = useCallback(() => {
    canvasRef.current?.focus();
  }, []);

  // Calculate ghost cursor position relative to displayed canvas
  const getGhostCursorStyle = (): React.CSSProperties | undefined => {
    if (!ghostCursor || !frameSize.width || !frameSize.height) return undefined;

    const canvas = canvasRef.current;
    if (!canvas) return undefined;

    const rect = canvas.getBoundingClientRect();

    return {
      transform: `translate(${(ghostCursor.x / frameSize.width) * rect.width}px, ${(ghostCursor.y / frameSize.height) * rect.height}px)`,
    };
  };

  return (
    <div className="screen-viewer" ref={containerRef}>
      {hasFrame ? (
        <>
          <canvas
            ref={canvasRef}
            onMouseMove={handleMouseMove}
            onMouseDown={handleMouseDown}
            onMouseUp={handleMouseUp}
            onWheel={handleWheel}
            onKeyDown={handleKeyDown}
            onKeyUp={handleKeyUp}
            onClick={handleCanvasClick}
            tabIndex={0}
            style={{
              outline: 'none',
              cursor: aiMode === 'full' ? 'not-allowed' : 'crosshair',
            }}
            title={
              aiMode === 'full'
                ? 'KI steuert – Eingabe gesperrt'
                : 'Klicken um Tastatureingaben zu erfassen'
            }
          />
          {ghostCursor && aiMode !== 'disabled' && (
            <div
              className="ghost-cursor"
              style={getGhostCursorStyle()}
              title="KI Cursor"
            />
          )}
        </>
      ) : (
        <div className="screen-placeholder">
          <div className="icon">&#x1F5A5;</div>
          <p>Warte auf Remote-Bildschirm...</p>
          <p style={{ fontSize: '14px', marginTop: '8px', color: '#444' }}>
            {peerId ? `Verbunden mit: ${peerId}` : 'Nicht verbunden'}
          </p>
        </div>
      )}
    </div>
  );
}
