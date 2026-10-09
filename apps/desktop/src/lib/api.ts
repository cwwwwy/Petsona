import type { SettingsAction, SettingsSnapshot } from "../types";

interface TauriInternals {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: TauriInternals;
  }
}

function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const internals = window.__TAURI_INTERNALS__;
  if (!internals) {
    return Promise.reject(
      new Error("未检测到 Petsona 桌面运行时；请在应用窗口中打开设置页。"),
    );
  }
  return internals.invoke<T>(command, args);
}

export function loadSnapshot(): Promise<SettingsSnapshot> {
  return invoke<SettingsSnapshot>("settings_snapshot");
}

export function applyAction(action: SettingsAction): Promise<void> {
  return invoke<void>("settings_action", { action });
}

export function openPath(path: string): Promise<void> {
  return invoke<void>("open_data_path", { path });
}
