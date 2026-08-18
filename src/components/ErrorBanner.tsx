import { AlertIcon, CloseIcon } from "./Icons";

const ICON = "mt-px size-[15px] shrink-0";

export function ErrorBanner({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  return (
    <div className="mx-[12px] mt-[10px] flex items-start gap-[8px] rounded-[8px] border border-danger bg-danger-soft px-[12px] py-[10px] text-[12px] text-danger">
      <AlertIcon className={ICON} />
      <span className="flex-1 select-text">{message}</span>
      <button
        className="inline-flex items-center justify-center gap-[7px] rounded-[8px]"
        type="button"
        aria-label="Dismiss"
        onClick={onDismiss}
      >
        <CloseIcon className={ICON} />
      </button>
    </div>
  );
}
