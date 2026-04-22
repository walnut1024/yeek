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

class SseEventTransport implements EventTransport {
  private es: EventSource;
  private listeners: Map<string, Set<EventHandler>> = new Map();

  constructor(url = "http://localhost:17321/api/events") {
    this.es = new EventSource(url);
    this.es.onmessage = (msg) => {
      try {
        const { event, payload } = JSON.parse(msg.data);
        this.listeners.get(event)?.forEach((h) => h(payload));
      } catch {
        /* ignore malformed messages */
      }
    };
  }

  async on<T = unknown>(event: string, handler: EventHandler<T>): Promise<UnlistenFn> {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set());
    this.listeners.get(event)!.add(handler as EventHandler);
    return () => {
      this.listeners.get(event)?.delete(handler as EventHandler);
    };
  }
}

const isTauri = !!(window as any).__TAURI_INTERNALS__;

let eventTransport: EventTransport = isTauri
  ? new TauriEventTransport()
  : new SseEventTransport();

export function getEventTransport(): EventTransport {
  return eventTransport;
}
