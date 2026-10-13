import type { SettingsAction, SettingsSnapshot } from "../types";

interface TauriInternals {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  metadata?: {
    currentWindow?: { label?: string };
    currentWebview?: { label?: string };
  };
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

export function currentWindowLabel(): string {
  return (
    window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label ??
    window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label ??
    "settings"
  );
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

export function openExternalUrl(url: string): Promise<void> {
  return invoke<void>("open_external_url", { url });
}

export function openChatWindow(): Promise<void> {
  return invoke<void>("open_chat_window");
}

export function setAutostart(enabled: boolean): Promise<void> {
  return invoke<void>("set_autostart", { enabled });
}

export function pickImportZip(): Promise<string | null> {
  return invoke<string | null>("pick_import_zip");
}

export function pickImportFolder(): Promise<string | null> {
  return invoke<string | null>("pick_import_folder");
}

export function pickExportZip(defaultName: string): Promise<string | null> {
  return invoke<string | null>("pick_export_zip", { defaultName });
}

export function pickPersonaImport(): Promise<string | null> {
  return invoke<string | null>("pick_persona_import");
}

export function pickPersonaExport(defaultName: string): Promise<string | null> {
  return invoke<string | null>("pick_persona_export", { defaultName });
}

export function pickPersonaSource(): Promise<string | null> {
  return invoke<string | null>("pick_persona_source");
}

export function pickMemoryImport(): Promise<string | null> {
  return invoke<string | null>("pick_memory_import");
}

export function pickMemoryExport(defaultName: string): Promise<string | null> {
  return invoke<string | null>("pick_memory_export", { defaultName });
}

export function petPreview(
  path: string,
  frameWidth: number,
  frameHeight: number,
): Promise<number[]> {
  return invoke<number[]>("pet_preview", { path, frameWidth, frameHeight });
}
