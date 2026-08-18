import type { FormEvent } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { UiState } from "../types";
import type { Run } from "../useAppState";
import { useSyncedField } from "../useSyncedField";
import { KeyIcon } from "./Icons";

export function ApiKeyCard({ state, run }: { state: UiState | null; run: Run }) {
  const [key, setKey, keyRef] = useSyncedField(state, (pushed) => pushed.api_key);

  function save(event: FormEvent) {
    event.preventDefault();
    run("set_api_key", { key }, "Saving the key...");
  }

  return (
    <section className="card">
      <h2>NASA API key</h2>
      <p className="hint">
        Without a personal key, DEMO_KEY is used (30 requests/hour, 50/day). A free
        key takes seconds to obtain from api.nasa.gov.
      </p>
      <form id="key-form" onSubmit={save}>
        <input
          ref={keyRef}
          type="text"
          placeholder="DEMO_KEY"
          spellCheck={false}
          autoComplete="off"
          value={key}
          onChange={(event) => setKey(event.target.value)}
        />
        <button type="submit">Save</button>
      </form>
      <button
        className="link-btn"
        type="button"
        onClick={() => void openUrl("https://api.nasa.gov/")}
      >
        <KeyIcon />
        Get a key at api.nasa.gov
      </button>
    </section>
  );
}
