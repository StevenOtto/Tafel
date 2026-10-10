/**
 * A macOS-style alert for destructive, irreversible actions: bold title,
 * one explanatory line, and two buttons — Cancel is the default
 * (Return), the destructive action reads in system red. Built on the native
 * <dialog>, so focus is trapped, Escape cancels, and it sits in the top layer
 * above whatever opened it.
 */

import React, { useEffect, useRef } from "react";

export interface ConfirmRequest {
  readonly title: string;
  readonly message: string;
  readonly confirmLabel: string;
  readonly onConfirm: () => void;
}

export const ConfirmDialog = ({
  request,
  onCancel,
}: {
  request: ConfirmRequest | null;
  onCancel: () => void;
}): React.JSX.Element | null => {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog || !request || dialog.open) return;
    dialog.showModal();
  }, [request]);

  if (!request) return null;

  return (
    <dialog
      ref={ref}
      className="confirm"
      aria-labelledby="confirm-title"
      aria-describedby="confirm-message"
      onClose={onCancel}
    >
      <h2 id="confirm-title" className="confirm__title">
        {request.title}
      </h2>
      <p id="confirm-message" className="confirm__message">
        {request.message}
      </p>
      <div className="confirm__actions">
        <button
          type="button"
          className="confirm__button confirm__button--default"
          onClick={onCancel}
          autoFocus
        >
          Cancel
        </button>
        <button
          type="button"
          className="confirm__button confirm__button--destructive"
          onClick={request.onConfirm}
        >
          {request.confirmLabel}
        </button>
      </div>
    </dialog>
  );
};
