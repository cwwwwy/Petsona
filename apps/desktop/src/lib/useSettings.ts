import { useCallback, useEffect, useRef, useState } from "react";
import { applyAction, loadSnapshot } from "./api";
import type { SettingsAction, SettingsSnapshot } from "../types";
export interface SettingsController {
    snapshot: SettingsSnapshot | null;
    loading: boolean;
    error: string;
    refresh: () => Promise<void>;
    apply: (action: SettingsAction) => Promise<SettingsSnapshot>;
}
export function useSettings(pollMs = 700): SettingsController {
    const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState("");
    const mounted = useRef(true);
    const latestRevision = useRef(-1);
    const accept = useCallback((next: SettingsSnapshot) => {
        if (!mounted.current || next.revision < latestRevision.current)
            return;
        latestRevision.current = next.revision;
        setSnapshot(next);
        setError(next.faulted ? next.error || "应用暂时无法继续" : "");
    }, []);
    const refresh = useCallback(async () => {
        try {
            const next = await loadSnapshot();
            if (!mounted.current)
                return;
            accept(next);
        }
        catch (reason) {
            if (!mounted.current)
                return;
            setError(reason instanceof Error ? reason.message : String(reason));
        }
        finally {
            if (mounted.current)
                setLoading(false);
        }
    }, [accept]);
    const apply = useCallback(async (action: SettingsAction) => {
        await applyAction(action);
        const next = await loadSnapshot();
        accept(next);
        return next;
    }, [accept]);
    useEffect(() => {
        mounted.current = true;
        void refresh();
        const timer = window.setInterval(() => {
            if (!document.hidden)
                void refresh();
        }, pollMs);
        return () => {
            mounted.current = false;
            window.clearInterval(timer);
        };
    }, [pollMs, refresh]);
    return { snapshot, loading, error, refresh, apply };
}
