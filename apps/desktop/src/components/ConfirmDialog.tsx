import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { Button } from "./ui";

/** Native dialog supplies inert background, focus containment and restoration. */
export function Dialog({ title, children, onClose }: {
  title: string;
  children: ReactNode;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const element = dialog.current!;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    element.showModal();
    const initial = element.querySelector<HTMLElement>("[data-initial-focus]")
      ?? element.querySelector<HTMLElement>("button:not(:disabled), input, textarea, select");
    if (initial?.classList.contains("initial-focus")) initial.querySelector<HTMLElement>("button")?.focus();
    else initial?.focus();
    return () => {
      element.close();
      if (previous?.isConnected) previous.focus();
    };
  }, []);
  return (
    <dialog ref={dialog} className="modal" aria-labelledby={titleId}
      onCancel={(event) => { event.preventDefault(); close.current(); }}>
      <h2 id={titleId}>{title}</h2>
      {children}
    </dialog>
  );
}

type Confirmation = { title: string; message: string; confirmLabel?: string };

export function useConfirm() {
  const [request, setRequest] = useState<Confirmation | null>(null);
  const resolve = useRef<((value: boolean) => void) | null>(null);
  useEffect(() => () => { resolve.current?.(false); }, []);
  const finish = (value: boolean) => {
    const complete = resolve.current;
    resolve.current = null;
    setRequest(null);
    complete?.(value);
  };
  const confirm = (next: Confirmation) => new Promise<boolean>((complete) => {
    resolve.current?.(false);
    resolve.current = complete;
    setRequest(next);
  });
  const confirmation = request && (
    <Dialog title={request.title} onClose={() => finish(false)}>
      <p>{request.message}</p>
      <div className="modal-actions">
        <span data-initial-focus tabIndex={-1} className="initial-focus">
          <Button variant="ghost" onClick={() => finish(false)}>取消</Button>
        </span>
        <Button variant="danger" onClick={() => finish(true)}>{request.confirmLabel ?? "确认"}</Button>
      </div>
    </Dialog>
  );
  return { confirm, confirmation };
}
