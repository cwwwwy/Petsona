import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  Card,
  EmptyState,
  InlineNotice,
  PageHeader,
} from "../components/ui";
import { ActionMenu } from "../components/ActionMenu";
import { Dialog, useConfirm } from "../components/ConfirmDialog";
import { PetThumb } from "../components/PetThumb";
import { pickExportZip, pickImportFolder, pickImportZip } from "../lib/api";
import type { ImportConflict, PageProps } from "../types";

type Busy =
  | "zip"
  | "folder"
  | "scan"
  | "export"
  | "delete"
  | `codex:${string}`
  | null;

export function PetsPage({ snapshot, run }: PageProps) {
  const [selectedId, setSelectedId] = useState(snapshot.petId);
  const { confirm, confirmation } = useConfirm();
  const [busy, setBusy] = useState<Busy>(null);
  const conflict = snapshot.importConflict as ImportConflict | null;

  useEffect(() => {
    setSelectedId(snapshot.petId);
  }, [snapshot.petId]);

  const selected = snapshot.pets.find((pet) => pet.id === selectedId) ?? null;

  const scan = async () => {
    setBusy("scan");
    try {
      await run({ type: "scanCodexPets" });
    } finally {
      setBusy(null);
    }
  };

  const importFromDialog = async (kind: "zip" | "folder") => {
    setBusy(kind);
    try {
      const path = kind === "zip" ? await pickImportZip() : await pickImportFolder();
      if (path) {
        await run({ type: "importPet", path, overwrite: false });
      }
    } finally {
      setBusy(null);
    }
  };

  const importCodex = async (path: string) => {
    setBusy(`codex:${path}`);
    try {
      await run({ type: "importPet", path, overwrite: false });
    } finally {
      setBusy(null);
    }
  };

  const exportSelected = async () => {
    if (!selected) return;
    setBusy("export");
    try {
      const path = await pickExportZip(`${selected.id}.zip`);
      if (path) {
        await run({ type: "exportPet", id: selected.id, path });
      }
    } finally {
      setBusy(null);
    }
  };

  const deleteSelected = async () => {
    if (!selected) return;
    if (!await confirm({ title: "删除宠物", message: `确定删除宠物「${selected.name}」吗？宠物文件会从本地宠物库移除。人格与记忆会保留，重新导入同 ID 可继续使用。`, confirmLabel: "删除" })) {
      return;
    }
    setBusy("delete");
    try {
      await run({ type: "deletePet", id: selected.id });
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="宠物"
        title="宠物库"
        description="本地宠物库。单击选择、双击切换；可拖入 ZIP / 宠物文件夹，或用导入菜单（含 Codex 扫描）。"
        actions={
          <ActionMenu label={busy === "zip" || busy === "folder" ? "正在导入…" : "导入"} primary disabled={busy !== null}
            actions={[
              { label: "选择 ZIP 文件", hint: "导入压缩包中的宠物", onClick: () => void importFromDialog("zip") },
              { label: "选择宠物文件夹", hint: "导入包含 pet.json 的目录", onClick: () => void importFromDialog("folder") },
              { label: "扫描 Codex 宠物", hint: "复制到本地库，保留原文件", onClick: () => void scan() },
            ]} />
        }
      />

      <Card
        title="本地宠物库"
        description={`${snapshot.pets.length} 个宠物包。单击选择，双击切换；选择后可导入/导出/删除。`}
      >
        {snapshot.pets.length === 0 ? (
          <EmptyState
            title="宠物库为空"
            description="Petsona 不会内置宠物。导入或从 Codex 扫描后，宠物会显示在这里。"
          />
        ) : (
          <div className="pet-list">
            {snapshot.pets.map((pet) => {
              const active = pet.id === snapshot.petId;
              const selectedRow = pet.id === selectedId;
              return (
                <button
                  key={pet.id}
                  type="button"
                  className={`pet-row${selectedRow ? " pet-row-selected" : ""}`}
                  onClick={() => setSelectedId(pet.id)}
                  onDoubleClick={() => void run({ type: "selectPet", id: pet.id })}
                >
                  <PetThumb
                    path={pet.spritesheet}
                    frameWidth={pet.cellWidth}
                    frameHeight={pet.cellHeight}
                    fallback={pet.name}
                  />
                  <div className="pet-row-copy">
                    <strong>{pet.name}</strong>
                    <span className="mono">{pet.id}</span>
                  </div>
                  <div className="pet-row-tags">
                    {pet.v2 ? <Badge tone="accent">V2</Badge> : <Badge>V1</Badge>}
                    {active && <Badge tone="positive">当前</Badge>}
                  </div>
                </button>
              );
            })}
          </div>
        )}
        <div className="card-footer pet-actions">
          <span className="muted">
            {selected ? `已选择：${selected.name}` : "选择一只宠物以查看操作"}
          </span>
          <div className="button-group">
            <Button
              variant="ghost"
              disabled={!selected || busy !== null}
              onClick={() => void exportSelected()}
            >
              {busy === "export" ? "导出中…" : "导出"}
            </Button>
            <Button
              variant="danger"
              disabled={!selected || busy !== null}
              onClick={() => void deleteSelected()}
            >
              {busy === "delete" ? "删除中…" : "删除"}
            </Button>
            <Button
              variant="primary"
              disabled={!selected || selected.id === snapshot.petId || busy !== null}
              onClick={() => selected && void run({ type: "selectPet", id: selected.id })}
            >
              设为当前宠物
            </Button>
          </div>
        </div>
      </Card>

      {snapshot.codexPets.length > 0 && (
        <Card
          title="Codex 候选"
          description={`找到 ${snapshot.codexPets.length} 个候选；双击导入，不会修改 Codex 原始文件。`}
        >
          <div className="pet-list compact">
            {snapshot.codexPets.map((pet) => (
              <div
                className="pet-row pet-row-static"
                key={pet.id}
                onDoubleClick={() => void importCodex(pet.path)}
                title="双击导入"
              >
                <PetThumb
                  path={pet.spritesheet}
                  frameWidth={pet.cellWidth}
                  frameHeight={pet.cellHeight}
                  fallback={pet.name}
                />
                <div className="pet-row-copy">
                  <strong>{pet.name}</strong>
                  <span className="truncate" title={pet.path}>
                    {pet.path}
                  </span>
                </div>
                <Button
                  variant="secondary"
                  disabled={busy !== null}
                  onClick={() => void importCodex(pet.path)}
                >
                  {busy === `codex:${pet.path}` ? "导入中…" : "导入"}
                </Button>
              </div>
            ))}
          </div>
        </Card>
      )}

      {snapshot.status && (
        <p className="status-line">
          <Badge tone={snapshot.status.includes("失败") ? "warning" : "neutral"}>状态</Badge>
          {snapshot.status}
        </p>
      )}

      <InlineNotice>
        导入宠物会复制到 Petsona 的本地宠物库；删除只影响 Petsona 的副本，不会删除 Codex 原始宠物。
      </InlineNotice>

      {conflict && (
        <Dialog title="发现同 ID 宠物" onClose={() => void run({ type: "clearImportConflict" })}>
          <p>
            本地已有宠物「{conflict.name || conflict.id}」。覆盖会替换 Petsona 宠物库中的副本，
            不会影响 Codex 原始文件。
          </p>
          <div className="modal-actions">
            <Button
              variant="ghost"
              onClick={() => void run({ type: "clearImportConflict" })}
            >
              取消
            </Button>
            <Button
              variant="danger"
              onClick={() => {
                void run({
                  type: "importPet",
                  path: conflict.path,
                  overwrite: true,
                });
              }}
            >
              覆盖导入
            </Button>
          </div>
        </Dialog>
      )}
      {confirmation}
    </div>
  );
}
