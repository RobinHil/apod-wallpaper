import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { UiState } from "./types";

/**
 * Runs a backend command and adopts the state it returns.
 *
 * `label` is what the blocking overlay says while the command is in flight;
 * it defaults to a generic wording.
 */
export type Run = (
  command: string,
  args?: Record<string, unknown>,
  label?: string,
) => void;

export interface AppState {
  /** Null until the first `get_state` answers. */
  state: UiState | null;
  /** The overlay's label while the UI is blocked, null when it is not. */
  busy: string | null;
  error: string | null;
  /** Reports a problem the panel found on its own, without calling the backend. */
  showError: (message: string) => void;
  dismissError: () => void;
  run: Run;
}

/**
 * Subscribes to a backend event for the lifetime of the component.
 *
 * The handler is read through a ref so that a re-render never tears the
 * subscription down and builds it again: `listen` resolves asynchronously,
 * and a subscription replaced mid-flight can miss what arrives in between.
 */
function useBackendEvent<T>(event: string, handler: (payload: T) => void): void {
  const latest = useRef(handler);
  latest.current = handler;

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    void listen<T>(event, (received) => latest.current(received.payload)).then(
      (stop) => {
        // Unmounted before `listen` resolved: stop the subscription that has
        // just been handed to us, since the cleanup below has already run.
        if (cancelled) stop();
        else unlisten = stop;
      },
    );

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [event]);
}

/**
 * The panel's whole client state: what the backend last reported, whether an
 * action is in flight, and the last error.
 *
 * Every action goes through `run`: the UI is covered by an overlay until the
 * backend finishes (or fails), then the state the command returned is
 * adopted. Any error shows up in the banner -- nothing fails silently.
 */
export function useAppState(): AppState {
  const [state, setState] = useState<UiState | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Read synchronously by the event handlers below and by `run`'s re-entrancy
  // guard, both of which need the value as it is now rather than as it was
  // when the current render started.
  const pending = useRef(false);

  const run = useCallback<Run>((command, args, label) => {
    if (pending.current) return;
    pending.current = true;
    setBusy(label ?? "Applying...");
    setError(null);

    void (async () => {
      try {
        setState(await invoke<UiState>(command, args));
      } catch (failure) {
        setError(String(failure));
        // Resynchronise the display with the real backend state after a
        // failure.
        try {
          setState(await invoke<UiState>("get_state"));
        } catch {
          // Backend unreachable: keep the current display and the visible
          // error.
        }
      } finally {
        pending.current = false;
        setBusy(null);
      }
    })();
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        setState(await invoke<UiState>("get_state"));
      } catch (failure) {
        setError(String(failure));
      }
    })();
  }, []);

  // Updates pushed by the backend (daily check, startup...): do not adopt one
  // while an action is running, so the blocking overlay is not disturbed.
  useBackendEvent<UiState>("state-updated", (pushed) => {
    if (!pending.current) setState(pushed);
  });

  // The action we just asked for is queued behind an update already running --
  // the scheduler's, usually. It can take as long as a download does, so the
  // overlay says what it is waiting on rather than sitting there mute.
  useBackendEvent<void>("update-waiting", () => {
    if (pending.current) setBusy("Waiting for the update in progress...");
  });

  const showError = useCallback((message: string) => setError(message), []);
  const dismissError = useCallback(() => setError(null), []);

  return { state, busy, error, showError, dismissError, run };
}
