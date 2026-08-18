import { useEffect, useRef, useState } from "react";
import type { UiState } from "./types";

/**
 * A text field the user types in and the backend also owns.
 *
 * The backend pushes a whole new state on every command and on its own
 * schedule, and each push would otherwise overwrite whatever is half-typed.
 * The value is therefore held locally and only re-seeded from a push while
 * the field does not have the focus.
 *
 * `pick` returning null means this push carries nothing worth seeding with,
 * which is how the date field keeps what it shows when the backend has no
 * date to offer.
 */
export function useSyncedField(
  state: UiState | null,
  pick: (state: UiState) => string | null,
) {
  const ref = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState("");

  const picker = useRef(pick);
  picker.current = pick;

  // Deliberately keyed on the state object rather than on the picked value:
  // the backend sends a fresh object on every push, and re-seeding on each of
  // them is what the field is meant to do.
  useEffect(() => {
    if (state === null || document.activeElement === ref.current) return;
    const pushed = picker.current(state);
    if (pushed !== null) setValue(pushed);
  }, [state]);

  return [value, setValue, ref] as const;
}
