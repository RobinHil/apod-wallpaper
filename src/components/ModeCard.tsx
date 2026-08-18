import {
  useEffect,
  useMemo,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import type { UiState } from "../types";
import type { Run } from "../useAppState";
import { useSyncedField } from "../useSyncedField";
import {
  CalendarIcon,
  CalendarPickIcon,
  RefreshIcon,
  ShuffleIcon,
} from "./Icons";

/** Upper bound of the date picker: today, in local time. */
function today(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

export function ModeCard({
  state,
  run,
  onError,
}: {
  state: UiState | null;
  run: Run;
  onError: (message: string) => void;
}) {
  // Bumped on every "Specific date" click. It does two things the mode alone
  // cannot: keep the picker visible before any date has been applied (the mode
  // only switches once one has been), and move the focus into it again on a
  // second click, when the picker is already open.
  const [pickerRequests, setPickerRequests] = useState(0);
  const [date, setDate, dateRef] = useSyncedField(
    state,
    (pushed) => pushed.specific_date || null,
  );

  const maximumDate = useMemo(today, []);
  const pickerVisible = state?.mode === "specific" || pickerRequests > 0;

  // Keyed on the click count, so a "specific" mode arriving from the backend
  // never steals the focus -- only a click does.
  useEffect(() => {
    if (pickerRequests > 0) dateRef.current?.focus();
  }, [pickerRequests, dateRef]);

  function applyDate(event: FormEvent) {
    event.preventDefault();
    if (!date) {
      onError("Pick a date first.");
      return;
    }
    run("set_specific_date", { date }, `Loading the APOD for ${date}...`);
  }

  return (
    <section className="card">
      <h2>Mode</h2>
      <div className="segmented" role="group" aria-label="Image selection mode">
        <SegmentedButton
          active={state?.mode === "daily"}
          onClick={() => {
            setPickerRequests(0);
            run("set_mode", { mode: "daily" }, "Switching to picture of the day...");
          }}
        >
          <CalendarIcon />
          Picture of the day
        </SegmentedButton>
        <SegmentedButton
          active={state?.mode === "random"}
          onClick={() => {
            setPickerRequests(0);
            run("set_mode", { mode: "random" }, "Drawing a random image...");
          }}
        >
          <ShuffleIcon />
          Random
        </SegmentedButton>
        <SegmentedButton
          active={state?.mode === "specific"}
          // No backend call here: reveal the picker, the mode only switches
          // once a date has been chosen and applied.
          onClick={() => setPickerRequests((clicks) => clicks + 1)}
        >
          <CalendarPickIcon />
          Specific date
        </SegmentedButton>
      </div>
      <p className="hint">
        The wallpaper is updated once per day, at the local day change. "Refresh now"
        applies a new image straight away.
      </p>

      {pickerVisible && (
        <div>
          <form id="date-form" onSubmit={applyDate}>
            <input
              ref={dateRef}
              type="date"
              min="1995-06-16"
              max={maximumDate}
              aria-label="APOD date"
              value={date}
              onChange={(event) => setDate(event.target.value)}
            />
            <button type="submit">Apply</button>
          </form>
          <p className="hint">
            Available dates: from 16 June 1995 (the first published APOD) to today. A
            few rare days have no publication.
          </p>
        </div>
      )}

      <h2>Screen fit</h2>
      <div className="segmented" role="group" aria-label="Image fit">
        <SegmentedButton
          active={state?.fit_mode === "blur_fill"}
          onClick={() =>
            run("set_fit_mode", { fit: "blur_fill" }, "Recomposing the wallpaper...")
          }
        >
          Blurred fill
        </SegmentedButton>
        <SegmentedButton
          active={state?.fit_mode === "crop_fill"}
          onClick={() =>
            run("set_fit_mode", { fit: "crop_fill" }, "Recomposing the wallpaper...")
          }
        >
          Crop
        </SegmentedButton>
      </div>

      <button
        className="primary"
        type="button"
        onClick={() => run("refresh_now", undefined, "Checking for the latest image...")}
      >
        <RefreshIcon />
        Refresh now
      </button>
    </section>
  );
}

/** One choice in a segmented control, announced as a toggle to assistive tech. */
function SegmentedButton({
  active,
  onClick,
  children,
}: {
  active: boolean | undefined;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={active ? "active" : undefined}
      aria-pressed={active === true}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
