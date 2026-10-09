import type { PageId } from "../types";

const NAV_ITEMS: Array<{ id: PageId; label: string; icon: IconName }> = [
  { id: "pets", label: "宠物", icon: "paw" },
  { id: "appearance", label: "外观与交互", icon: "sliders" },
  { id: "persona", label: "人格", icon: "spark" },
  { id: "memory", label: "记忆", icon: "brain" },
  { id: "connection", label: "连接与问候", icon: "plug" },
  { id: "system", label: "系统", icon: "gear" },
];

type IconName = "paw" | "sliders" | "spark" | "brain" | "plug" | "gear";

function Icon({ name }: { name: IconName }) {
  const common = {
    width: 18,
    height: 18,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.8,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
  };
  switch (name) {
    case "paw":
      return (
        <svg {...common}>
          <path d="M8.2 11.2c-1.6-1.4-2.4-3.2-1.8-4.6.5-1.2 1.8-1.5 2.9-.7 1 .7 1.7 2 2.1 3.3" />
          <path d="M15.8 11.2c1.6-1.4 2.4-3.2 1.8-4.6-.5-1.2-1.8-1.5-2.9-.7-1 .7-1.7 2-2.1 3.3" />
          <path d="M5.1 15.1c-.8-1.2-.9-2.6-.2-3.4.7-.8 1.9-.7 2.9.1.9.7 1.6 1.8 1.9 2.9" />
          <path d="M18.9 15.1c.8-1.2.9-2.6.2-3.4-.7-.8-1.9-.7-2.9.1-.9.7-1.6 1.8-1.9 2.9" />
          <path d="M12 12.8c2.3 0 4.3 2.1 4.3 4.2 0 1.7-1.3 2.8-3 2.8h-2.6c-1.7 0-3-1.1-3-2.8 0-2.1 2-4.2 4.3-4.2Z" />
        </svg>
      );
    case "sliders":
      return (
        <svg {...common}>
          <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
          <circle cx="16" cy="7" r="2" />
          <circle cx="10" cy="17" r="2" />
        </svg>
      );
    case "spark":
      return (
        <svg {...common}>
          <path d="m12 3 1.2 4.3L17 9l-3.8 1.7L12 15l-1.2-4.3L7 9l3.8-1.7L12 3Z" />
          <path d="m18.5 15 .7 2.3 2.3.7-2.3.7-.7 2.3-.7-2.3-2.3-.7 2.3-.7.7-2.3Z" />
          <path d="m5.5 14 .5 1.7 1.7.5-1.7.5-.5 1.7-.5-1.7-1.7-.5 1.7-.5.5-1.7Z" />
        </svg>
      );
    case "brain":
      return (
        <svg {...common}>
          <path d="M9.3 5.1A3.2 3.2 0 0 0 4 7.2a3 3 0 0 0 .6 1.8A3.3 3.3 0 0 0 6.8 15" />
          <path d="M9.3 5.1V4.8a2.8 2.8 0 0 1 5.4 0v.3" />
          <path d="M14.7 5.1A3.2 3.2 0 0 1 20 7.2a3 3 0 0 1-.6 1.8 3.3 3.3 0 0 1-2.2 6" />
          <path d="M9.3 5.1v13.4a2.5 2.5 0 0 0 5 0V5.1" />
          <path d="M7 11.2a2.5 2.5 0 0 0 2.3 2.4M17 11.2a2.5 2.5 0 0 1-2.3 2.4" />
          <path d="M7.5 17.1A2.4 2.4 0 0 0 9.6 19M16.5 17.1a2.4 2.4 0 0 1-2.1 1.9" />
        </svg>
      );
    case "plug":
      return (
        <svg {...common}>
          <path d="M9 3v5M15 3v5M7 8h10v2a5 5 0 0 1-5 5v0a5 5 0 0 1-5-5V8ZM12 15v6" />
        </svg>
      );
    case "gear":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-1.8 1.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-2.6V20a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1-1.8-1.8.1-.1A1.7 1.7 0 0 0 7 15a1.7 1.7 0 0 0-1.6-1H5v-2.6h.4A1.7 1.7 0 0 0 7 10.4a1.7 1.7 0 0 0-.3-1.9l-.1-.1 1.8-1.8.1.1a1.7 1.7 0 0 0 1.9.3 1.7 1.7 0 0 0 1-1.6V5h2.6v.4a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1 1.8 1.8-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v2.6H20a1.7 1.7 0 0 0-1.6 1Z" />
        </svg>
      );
  }
}

export function Sidebar({
  active,
  version,
  onChange,
}: {
  active: PageId;
  version: string;
  onChange: (page: PageId) => void;
}) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark">P</div>
        <div className="brand-copy">
          <strong>Petsona</strong>
          <span>设置</span>
        </div>
      </div>
      <nav className="nav" aria-label="设置导航">
        {NAV_ITEMS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`nav-item${active === item.id ? " nav-item-active" : ""}`}
            onClick={() => onChange(item.id)}
            title={item.label}
            aria-current={active === item.id ? "page" : undefined}
          >
            <span className="nav-icon">
              <Icon name={item.icon} />
            </span>
            <span className="nav-label">{item.label}</span>
          </button>
        ))}
      </nav>
      <div className="sidebar-footer">
        <span className="sidebar-version">v{version}</span>
      </div>
    </aside>
  );
}
