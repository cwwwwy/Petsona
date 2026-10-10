import { createContext, useContext, useId, type ReactNode } from "react";

const FieldContext = createContext<{ labelId: string; hintId?: string } | null>(null);

export function PageHeader({
  eyebrow,
  title,
  description,
  actions,
}: {
  eyebrow?: string;
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <header className="page-header">
      <div>
        {eyebrow && <p className="eyebrow">{eyebrow}</p>}
        <h1>{title}</h1>
        {description && <p className="page-description">{description}</p>}
      </div>
      {actions && <div className="page-actions">{actions}</div>}
    </header>
  );
}

export function Card({
  title,
  description,
  children,
  tone = "default",
}: {
  title?: string;
  description?: string;
  children: ReactNode;
  tone?: "default" | "danger" | "accent";
}) {
  return (
    <section className={`card${tone === "default" ? "" : ` card-${tone}`}`}>
      {(title || description) && (
        <div className="card-heading">
          {title && <h2>{title}</h2>}
          {description && <p>{description}</p>}
        </div>
      )}
      {children}
    </section>
  );
}

export function SettingRow({
  label,
  hint,
  children,
  stacked = false,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
  stacked?: boolean;
}) {
  const id = useId();
  return (
    <div role="group" aria-labelledby={`${id}-label`} className={`setting-row${stacked ? " setting-row-stacked" : ""}`}>
      <div className="setting-copy">
        <span id={`${id}-label`} className="setting-label">{label}</span>
        {hint && <span id={`${id}-hint`} className="setting-hint">{hint}</span>}
      </div>
      <FieldContext.Provider value={{ labelId: `${id}-label`, hintId: hint ? `${id}-hint` : undefined }}>
        <div className="setting-control">{children}</div>
      </FieldContext.Provider>
    </div>
  );
}

export function Switch({
  checked,
  onChange,
  disabled = false,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={`switch${checked ? " switch-on" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span className="switch-thumb" />
    </button>
  );
}

export function TextField({
  label,
  value,
  onChange,
  placeholder,
  type = "text",
  disabled = false,
  spellCheck = false,
}: {
  label?: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: "text" | "password" | "url";
  disabled?: boolean;
  spellCheck?: boolean;
}) {
  const field = useContext(FieldContext);
  return (
    <input
      aria-label={label}
      aria-labelledby={label ? undefined : field?.labelId}
      aria-describedby={field?.hintId}
      className="control-input"
      type={type}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      placeholder={placeholder}
      disabled={disabled}
      spellCheck={spellCheck}
    />
  );
}

export function SelectField<T extends string>({
  label,
  value,
  onChange,
  options,
  disabled = false,
}: {
  label?: string;
  value: T;
  onChange: (value: T) => void;
  options: Array<{ value: T; label: string }>;
  disabled?: boolean;
}) {
  const field = useContext(FieldContext);
  return (
    <select
      aria-label={label}
      aria-labelledby={label ? undefined : field?.labelId}
      aria-describedby={field?.hintId}
      className="control-input"
      value={value}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value as T)}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

export function TextArea({
  label,
  value,
  onChange,
  rows = 5,
  placeholder,
  disabled = false,
}: {
  label?: string;
  value: string;
  onChange: (value: string) => void;
  rows?: number;
  placeholder?: string;
  disabled?: boolean;
}) {
  const field = useContext(FieldContext);
  return (
    <textarea
      aria-label={label}
      aria-labelledby={label ? undefined : field?.labelId}
      aria-describedby={field?.hintId}
      className="control-input control-textarea"
      value={value}
      rows={rows}
      placeholder={placeholder}
      disabled={disabled}
      spellCheck={false}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}

export function Button({
  children,
  onClick,
  variant = "secondary",
  disabled = false,
  title,
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "primary" | "secondary" | "danger" | "ghost";
  disabled?: boolean;
  title?: string;
}) {
  return (
    <button
      type="button"
      className={`button button-${variant}`}
      onClick={onClick}
      disabled={disabled}
      title={title}
    >
      {children}
    </button>
  );
}

export function Badge({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "neutral" | "positive" | "warning" | "accent";
}) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function EmptyState({
  title,
  description,
  action,
}: {
  title: string;
  description: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty-state">
      <div className="empty-mark">·</div>
      <strong>{title}</strong>
      <p>{description}</p>
      {action}
    </div>
  );
}

export function InlineNotice({
  children,
  tone = "info",
}: {
  children: ReactNode;
  tone?: "info" | "warning" | "danger";
}) {
  return <p className={`inline-notice inline-notice-${tone}`}>{children}</p>;
}
