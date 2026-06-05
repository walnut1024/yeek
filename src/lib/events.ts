import { listen, type UnlistenFn } from "@tauri-apps/api/event";

type EventHandler<T = unknown> = (payload: T) => void;

interface EventTransport {
  on<T = unknown>(event: string, handler: EventHandler<T>): Promise<UnlistenFn>;
}

class TauriEventTransport implements EventTransport {
  async on<T = unknown>(event: string, handler: EventHandler<T>): Promise<UnlistenFn> {
    return listen<T>(event, (e) => handler(e.payload));
  }
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

class NoopEventTransport implements EventTransport {
  async on<T = unknown>(_event: string, _handler: EventHandler<T>): Promise<UnlistenFn> {
    void _event;
    void _handler;
    return () => {};
  }
}

const isTauri = !!window.__TAURI_INTERNALS__;

const eventTransport: EventTransport = isTauri
  ? new TauriEventTransport()
  : new NoopEventTransport();

export function getEventTransport(): EventTransport {
  return eventTransport;
}
