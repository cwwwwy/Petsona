import { useCallback, useEffect, useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { Button } from "./components/ui";
import { useSettings } from "./lib/useSettings";
import { AppearancePage } from "./pages/AppearancePage";
import { ConnectionPage } from "./pages/ConnectionPage";
import { MemoryPage } from "./pages/MemoryPage";
import { PersonaPage } from "./pages/PersonaPage";
import { PetsPage } from "./pages/PetsPage";
import { SystemPage } from "./pages/SystemPage";
import type { PageId, SettingsAction } from "./types";

const PAGE_IDS: PageId[] = [
  "pets",
  "appearance",
  "persona",
  "memory",
  "connection",
  "system",
];

function storedPage(): PageId {
  const value = window.localStorage.getItem("petsona.settings.page");
  return PAGE_IDS.includes(value as PageId) ? (value as PageId) : "pets";
}

export default function App() {
  const [page, setPage] = useState<PageId>(storedPage);
  const [toast, setToast] = useState("");
  const { snapshot, loading, error, apply, refresh } = useSettings();

  useEffect(() => {
    window.localStorage.setItem("petsona.settings.page", page);
  }, [page]);

  useEffect(() => {
    if (snapshot) {
      document.title = `Petsona 设置 · ${snapshot.petName || "未选择宠物"}`;
    }
  }, [snapshot]);

  const run = useCallback(
    async (action: SettingsAction) => {
      try {
        await apply(action);
      } catch (reason) {
        setToast(reason instanceof Error ? reason.message : String(reason));
      }
    },
    [apply],
  );

  if (loading && !snapshot) {
    return (
      <main className="boot">
        <div className="brand-mark large">P</div>
        <h1>正在打开 Petsona</h1>
        <p>读取本机宠物与设置…</p>
      </main>
    );
  }

  if (!snapshot) {
    return (
      <main className="boot">
        <div className="brand-mark large">P</div>
        <h1>无法读取设置</h1>
        <p>{error || "运行时没有返回设置快照。"}</p>
        <Button variant="primary" onClick={() => void refresh()}>
          重试
        </Button>
      </main>
    );
  }

  // Do not mount page components against the empty startup projection. The
  // runtime publishes arrays/objects only when it becomes ready; without this
  // gate the first render can crash on a temporary null field.
  if (!snapshot.ready && !snapshot.faulted) {
    return (
      <main className="boot">
        <div className="brand-mark large">P</div>
        <h1>正在读取设置</h1>
        <p>等待 Petsona 运行时准备完成…</p>
      </main>
    );
  }

  const common = { snapshot, run };
  const content = (() => {
    switch (page) {
      case "appearance":
        return <AppearancePage {...common} />;
      case "persona":
        return <PersonaPage {...common} />;
      case "memory":
        return <MemoryPage {...common} />;
      case "connection":
        return <ConnectionPage {...common} />;
      case "system":
        return <SystemPage {...common} />;
      default:
        return <PetsPage {...common} />;
    }
  })();

  return (
    <div className="app-shell">
      <Sidebar active={page} version={snapshot.appVersion} onChange={setPage} />
      <main className="content">
        {(snapshot.faulted || error) && (
          <div className="global-notice global-notice-danger">
            <strong>运行时需要处理</strong>
            <span>{snapshot.error || error}</span>
          </div>
        )}
        {content}
      </main>
      {toast && (
        <div className="toast" role="alert">
          <span>{toast}</span>
          <button type="button" onClick={() => setToast("")} aria-label="关闭提示">
            ×
          </button>
        </div>
      )}
    </div>
  );
}
