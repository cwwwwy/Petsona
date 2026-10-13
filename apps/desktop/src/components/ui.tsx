import { createContext, useContext, useId, isValidElement, type RefObject, type ReactNode } from "react";
const FieldContext = createContext<{
    labelId: string;
    hintId?: string;
} | null>(null);
export function PageHeader({ eyebrow, title, description, actions, }: {
    eyebrow?: string;
    title: string;
    description?: string;
    actions?: ReactNode;
}) {
    return (<header className="page-header">
      <div>
        {eyebrow && <p className="eyebrow">{eyebrow}</p>}
        <h1>{title}</h1>
        {description && <p className="page-description">{description}</p>}
      </div>
      {actions && <div className="page-actions">{actions}</div>}
    </header>);
}
export function Card({ title, description, children, tone = "default", }: {
    title?: string;
    description?: string;
    children: ReactNode;
    tone?: "default" | "danger" | "accent";
}) {
    return (<section className={`card${tone === "default" ? "" : ` card-${tone}`}`}>
      {(title || description) && (<div className="card-heading">
          {title && <h2>{title}</h2>}
          {description && <p>{description}</p>}
        </div>)}
      {children}
    </section>);
}
export function SettingRow({ label, hint, children, stacked = false, }: {
    label: string;
    hint?: string;
    children: ReactNode;
    stacked?: boolean;
}) {
    const id = useId();
    const toggle = isValidElement(children) && children.type === Switch;
    return (<div role="group" aria-labelledby={`${id}-label`} className={`setting-row${stacked ? " setting-row-stacked" : ""}${toggle ? " setting-row-toggle" : ""}`}>
      <div className="setting-copy">
        <span id={`${id}-label`} className="setting-label">{label}</span>
        {hint && <span id={`${id}-hint`} className="setting-hint">{hint}</span>}
      </div>
      <FieldContext.Provider value={{ labelId: `${id}-label`, hintId: hint ? `${id}-hint` : undefined }}>
        <div className="setting-control">{children}</div>
      </FieldContext.Provider>
    </div>);
}
export function Switch({ checked, onChange, disabled = false, label, }: {
    checked: boolean;
    onChange: (checked: boolean) => void;
    disabled?: boolean;
    label: string;
}) {
    const field = useContext(FieldContext);
    return (<button type="button" role="switch" aria-checked={checked} aria-describedby={field?.hintId} aria-label={label} className={`switch${checked ? " switch-on" : ""}`} disabled={disabled} onClick={() => onChange(!checked)}>
      <span className="switch-thumb"/>
    </button>);
}
type FieldProps = {
    label?: string;
    hint?: string;
    error?: string;
    disabled?: boolean;
};
function Field({ label, hint, error, id, children }: FieldProps & {
    id: string;
    children: ReactNode;
}) {
    const parent = useContext(FieldContext);
    if (parent && !hint && !error)
        return <>{children}</>;
    return <div className="field">
    {label && !parent && <label className="field-label" htmlFor={id}>{label}</label>}
    {children}
    {hint && <p className="field-hint" id={`${id}-hint`}>{hint}</p>}
    {error && <p className="field-error" id={`${id}-error`} role="alert">{error}</p>}
  </div>;
}
function useField(label?: string, hint?: string, error?: string) {
    const parent = useContext(FieldContext);
    const id = useId();
    return { id, "aria-labelledby": label && !parent ? undefined : parent?.labelId,
        "aria-describedby": [parent?.hintId, hint ? `${id}-hint` : "", error ? `${id}-error` : ""].filter(Boolean).join(" ") || undefined,
        "aria-invalid": error ? true : undefined };
}
export function TextField({ label, hint, error, value, onChange, placeholder, type = "text", disabled = false, spellCheck = false }: FieldProps & {
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    type?: "text" | "password" | "url";
    spellCheck?: boolean;
}) {
    const attributes = useField(label, hint, error);
    return <Field {...{ label, hint, error }} id={attributes.id}><input {...attributes} className="control-input" type={type} value={value} onChange={event => onChange(event.target.value)} placeholder={placeholder} disabled={disabled} spellCheck={spellCheck}/></Field>;
}
export function SelectField<T extends string>({ label, hint, error, value, onChange, options, disabled = false }: FieldProps & {
    value: T;
    onChange: (value: T) => void;
    options: Array<{
        value: T;
        label: string;
    }>;
}) {
    const attributes = useField(label, hint, error);
    return <Field {...{ label, hint, error }} id={attributes.id}><select {...attributes} className="control-input" value={value} disabled={disabled} onChange={event => onChange(event.target.value as T)}>{options.map(option => <option key={option.value} value={option.value}>{option.label}</option>)}</select></Field>;
}
export function TextArea({ label, hint, error, value, onChange, rows = 5, placeholder, disabled = false, inputRef }: FieldProps & {
    value: string;
    onChange: (value: string) => void;
    rows?: number;
    placeholder?: string;
    inputRef?: RefObject<HTMLTextAreaElement>;
}) {
    const attributes = useField(label, hint, error);
    return <Field {...{ label, hint, error }} id={attributes.id}><textarea ref={inputRef} {...attributes} className="control-input control-textarea" value={value} rows={rows} placeholder={placeholder} disabled={disabled} spellCheck={false} onChange={event => onChange(event.target.value)}/></Field>;
}
export type SavePhase = "idle" | "pending" | "saving" | "saved" | "error";
export function SaveStatus({ phase, message }: {
    phase: SavePhase;
    message?: string;
}) {
    const labels = { idle: "未修改", pending: "等待保存…", saving: "正在保存…", saved: "已保存", error: "尚未保存，请重试" };
    return <span className={`save-status save-status-${phase}`} role="status" aria-live="polite">{message || labels[phase]}</span>;
}
export function ActionFooter({ children, status }: {
    children: ReactNode;
    status?: ReactNode;
}) {
    return <div className="action-footer"><div className="action-status">{status}</div><div className="button-group">{children}</div></div>;
}
export function HelpText({ children }: {
    children: ReactNode;
}) { return <p className="help-text">{children}</p>; }
export function MemoryEditor({ value, onChange, kind, onKindChange, inputRef, busy, onCancel, onSave, saveLabel = "保存修改" }: {
    value: string;
    onChange: (value: string) => void;
    kind?: string;
    onKindChange?: (kind: string) => void;
    inputRef?: RefObject<HTMLTextAreaElement>;
    busy: boolean;
    onCancel: () => void;
    onSave: () => void;
    saveLabel?: string;
}) {
    return <div className="memory-editor">
    {kind && onKindChange && <SelectField label="记忆类型" value={kind} onChange={onKindChange} options={[{ value: "关于你", label: "关于你" }, { value: "共同经历", label: "共同经历" }, { value: "约定", label: "你们的约定" }]}/>}
    <TextArea inputRef={inputRef} label="这条记忆" value={value} onChange={onChange} rows={3}/>
    <ActionFooter><Button variant="ghost" disabled={busy} onClick={onCancel}>取消</Button><Button variant="primary" disabled={busy || !value.trim()} onClick={onSave}>{busy ? "正在保存…" : saveLabel}</Button></ActionFooter>
  </div>;
}
export function Button({ children, onClick, variant = "secondary", disabled = false, title, }: {
    children: ReactNode;
    onClick?: () => void;
    variant?: "primary" | "secondary" | "danger" | "ghost";
    disabled?: boolean;
    title?: string;
}) {
    return (<button type="button" className={`button button-${variant}`} onClick={onClick} disabled={disabled} title={title}>
      {children}
    </button>);
}
export function Badge({ children, tone = "neutral", }: {
    children: ReactNode;
    tone?: "neutral" | "positive" | "warning" | "accent";
}) {
    return <span className={`badge badge-${tone}`}>{children}</span>;
}
export function EmptyState({ title, description, action, }: {
    title: string;
    description: string;
    action?: ReactNode;
}) {
    return (<div className="empty-state">
      <div className="empty-mark">·</div>
      <strong>{title}</strong>
      <p>{description}</p>
      {action}
    </div>);
}
export function InlineNotice({ children, tone = "info", }: {
    children: ReactNode;
    tone?: "info" | "success" | "warning" | "danger";
}) {
    return <p role={tone === "danger" ? "alert" : "status"} className={`inline-notice inline-notice-${tone}`}>{children}</p>;
}
