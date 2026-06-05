// Conditional Tauri API loader
// Uses real Tauri API when running in Tauri, falls back to mock in browser

let isTauri = false;
if (typeof window !== 'undefined' && (window as any).__TAURI__) {
  isTauri = true;
}

export type UnlistenFn = () => void;

// Invoke a Tauri command
export async function invoke<T = any>(cmd: string, args?: Record<string, any>): Promise<T> {
  if (isTauri) {
    const mod = await import('@tauri-apps/api/core');
    return mod.invoke(cmd, args);
  } else {
    const mod = await import('./tauriMock');
    return mod.invoke(cmd, args);
  }
}

// Listen to Tauri events
export async function listen<T = any>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (isTauri) {
    const mod = await import('@tauri-apps/api/event');
    const unlisten = await mod.listen<T>(event, (ev: any) => handler(ev.payload));
    return unlisten;
  } else {
    const mod = await import('./tauriMock');
    return mod.listen(event, (ev: any) => handler(ev.payload));
  }
}

// Emit a Tauri event
export async function emit(event: string, payload?: any): Promise<void> {
  if (isTauri) {
    const mod = await import('@tauri-apps/api/event');
    return mod.emit(event, payload);
  }
  return Promise.resolve();
}

export { isTauri };
