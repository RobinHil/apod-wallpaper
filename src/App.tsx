import { useAppState } from "./useAppState";
import { ApiKeyCard } from "./components/ApiKeyCard";
import { CurrentImageCard } from "./components/CurrentImageCard";
import { ErrorBanner } from "./components/ErrorBanner";
import { ModeCard } from "./components/ModeCard";
import { Overlay } from "./components/Overlay";
import { PanelFooter } from "./components/PanelFooter";
import { TopBar } from "./components/TopBar";

/**
 * The settings panel.
 *
 * The backend owns the application state; this composes the cards that show
 * it and hands each of them the one way to change it, `run`.
 */
export function App() {
  const { state, busy, error, showError, dismissError, run } = useAppState();

  return (
    <>
      <TopBar state={state} />

      {error !== null && <ErrorBanner message={error} onDismiss={dismissError} />}

      <main className="flex flex-1 flex-col gap-[12px] overflow-y-auto p-[12px]">
        <CurrentImageCard current={state?.current ?? null} loading={state === null} />
        <ModeCard state={state} run={run} onError={showError} />
        <ApiKeyCard state={state} run={run} />
        <PanelFooter state={state} />
      </main>

      {busy !== null && <Overlay label={busy} />}
    </>
  );
}
