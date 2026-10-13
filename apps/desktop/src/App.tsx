import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { BrandMark } from "./components/BrandMark";
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
    const contentRef = useRef<HTMLElement>(null);
    const [page, setPage] = useState<PageId>(storedPage);
    const [toast, setToast] = useState("");
    const { snapshot, loading, error, apply, refresh } = useSettings();
    useLayoutEffect(() => {
        contentRef.current?.scrollTo({ top: 0, behavior: "instant" });
    }, [page, snapshot?.ready]);
    useEffect(() => {
        window.localStorage.setItem("petsona.settings.page", page);
    }, [page]);
    useEffect(() => {
        if (snapshot) {
            document.title = `Petsona 设置 · ${snapshot.petName || "未选择宠物"}`;
        }
    }, [snapshot]);
    const run = useCallback(async (action: SettingsAction) => {
        try {
            return await apply(action);
        }
        catch (reason) {
            setToast(reason instanceof Error ? reason.message : String(reason));
            return null;
        }
    }, [apply]);
    useEffect(() => {
        const onFailure = (event: PromiseRejectionEvent) => {
            setToast(event.reason instanceof Error ? event.reason.message : "操作未完成，请重试。");
            event.preventDefault();
        };
        window.addEventListener("unhandledrejection", onFailure);
        return () => window.removeEventListener("unhandledrejection", onFailure);
    }, []);
    if (loading && !snapshot) {
        return (<main className="boot">
        <BrandMark large/>
        <h1>正在打开 Petsona</h1>
        <p>读取本机宠物与设置…</p>
      </main>);
    }
    if (!snapshot) {
        return (<main className="boot">
        <BrandMark large/>
        <h1>无法读取设置</h1>
        <p>{error || "运行时没有返回设置快照。"}</p>
        <Button variant="primary" onClick={() => void refresh()}>
          重试
        </Button>
      </main>);
    }
    // Do not mount page components against the empty startup projection. The
    // runtime publishes arrays/objects only when it becomes ready; without this
    // gate the first render can crash on a temporary null field.
    if (!snapshot.ready) {
        return (<main className="boot">
        <BrandMark large/>
        <h1>{snapshot.faulted ? "暂时无法打开设置" : "正在读取设置"}</h1>
        <p>{snapshot.faulted ? snapshot.error || "请检查本机数据后重新启动应用。" : "正在准备你的伙伴与设置…"}</p>
      </main>);
    }
    const common = { snapshot, run };
    const pages = { pets: PetsPage, appearance: AppearancePage, persona: PersonaPage, memory: MemoryPage, connection: ConnectionPage, system: SystemPage };
    return (<div className="app-shell">
      <Sidebar active={page} version={snapshot.appVersion} petName={snapshot.petName} personaName={snapshot.persona.name} pet={snapshot.pets.find(pet => pet.id === snapshot.petId)} onChange={setPage}/>
      <main className="content" ref={contentRef}>
        {(snapshot.faulted || error) && (<div className="global-notice global-notice-danger">
            <strong>应用暂时无法继续</strong>
            <span>{snapshot.error || error}</span>
          </div>)}
        {PAGE_IDS.map(id => { const Page = pages[id]; return <div key={id} hidden={page !== id}><Page {...common}/></div>; })}
      </main>
      {toast && (<div className="toast" role="alert">
          <div><strong>操作未完成，请重试。</strong><details><summary>查看原因</summary><span>{toast}</span></details></div>
          <button type="button" onClick={() => setToast("")} aria-label="关闭提示">
            ×
          </button>
        </div>)}
    </div>);
}
