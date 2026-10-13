import { useEffect, useId, useRef, useState } from "react";

type Action = { label: string; hint?: string; disabled?: boolean; onClick: () => void };

export function ActionMenu({ label = "更多操作", primary = false, disabled = false, actions }: {
  label?: string; primary?: boolean; disabled?: boolean; actions: Action[];
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const id = useId();
  useEffect(() => {
    if (!open) return;
    root.current?.querySelector<HTMLElement>(".popover-menu button:not(:disabled)")?.focus();
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setOpen(false);
        trigger.current?.focus();
      }
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);
  return (
    <div className="import-control" ref={root} onBlur={(event) => {
      if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget as Node)) setOpen(false);
    }}>
      <button ref={trigger} type="button" className={`button button-${primary ? "primary" : "secondary"}`}
        disabled={disabled} aria-label={label} aria-expanded={open} aria-controls={open ? id : undefined}
        onClick={() => setOpen((value) => !value)}>{label === "更多操作" ? "⋯" : label}</button>
      {open && <div className="popover-menu" id={id}>
        {actions.map((action) => <button key={action.label} type="button" disabled={action.disabled}
          onClick={() => { setOpen(false); trigger.current?.focus(); action.onClick(); }}>
          <strong>{action.label}</strong>{action.hint && <span>{action.hint}</span>}
        </button>)}
      </div>}
    </div>
  );
}
