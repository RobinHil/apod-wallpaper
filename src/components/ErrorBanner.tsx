import { AlertIcon, CloseIcon } from "./Icons";

export function ErrorBanner({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  return (
    <div className="error-banner">
      <AlertIcon />
      <span>{message}</span>
      <button type="button" aria-label="Dismiss" onClick={onDismiss}>
        <CloseIcon />
      </button>
    </div>
  );
}
