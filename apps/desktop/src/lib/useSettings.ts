import { useCallback, useEffect, useRef, useState } from "react";
import { applyAction, loadSnapshot } from "./api";
import type { SettingsAction, SettingsSnapshot } from "../types";

const AFTER_ACTION_MS = 120;

function sleep(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

export interface SettingsController {
  snapshot: SettingsSnapshot | null;
  loading: boolean;
  error: string;
  refresh: () => Promise<void>;
  apply: (action: SettingsAction) => Promise<void>;
}

export function useSettings(pollMs = 700): SettingsController {
  const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const mounted = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const next = await loadSnapshot();
      if (!mounted.current) return;
      setSnapshot(next);
      setError(next.faulted ? next.error || "运行时发生故障" : "");
    } catch (reason) {
      if (!mounted.current) return;
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      if (mounted.current) setLoading(false);
    }
  }, []);

  const apply = useCallback(
    async (action: SettingsAction) => {
      await applyAction(action);
      await sleep(AFTER_ACTION_MS);
      await refresh();
    },
    [refresh],
  );

  useEffect(() => {
    mounted.current = true;
    void refresh();
    const timer = window.setInterval(() => {
      if (!document.hidden) void refresh();
    }, pollMs);
    return () => {
      mounted.current = false;
      window.clearInterval(timer);
    };
  }, [pollMs, refresh]);

  return { snapshot, loading, error, refresh, apply };
}
