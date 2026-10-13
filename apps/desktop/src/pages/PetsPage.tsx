import { useEffect, useState } from "react";
import { Badge, Button, Card, EmptyState, InlineNotice, HelpText, PageHeader, } from "../components/ui";
import { ActionMenu } from "../components/ActionMenu";
import { Dialog, useConfirm } from "../components/ConfirmDialog";
import { PetThumb } from "../components/PetThumb";
import { pickExportZip, pickImportFolder, pickImportZip } from "../lib/api";
import type { ImportConflict, PageProps } from "../types";
type Busy = "zip" | "folder" | "scan" | "export" | "delete" | `codex:${string}` | null;
export function PetsPage({ snapshot, run }: PageProps) {
    const [selectedId, setSelectedId] = useState(snapshot.petId);
    const { confirm, confirmation } = useConfirm();
    const [busy, setBusy] = useState<Busy>(null);
    const [notice, setNotice] = useState<{
        message: string;
        tone: "info" | "success" | "danger";
    } | null>(null);
    const perform: typeof run = async (action) => { const next = await run(action); if (!next) {
        setNotice({ message: "操作未确认，请重试。", tone: "danger" });
        return null;
    } if (next.status.includes("失败"))
        setNotice({ message: next.status, tone: "danger" });
    else if (!next.importConflict)
        setNotice({ message: action.type === "scanCodexPets" ? `找到 ${next.codexPets.length} 只可导入的宠物。` : "操作已完成。", tone: "success" }); return next; };
    const conflict = snapshot.importConflict as ImportConflict | null;
    useEffect(() => {
        setSelectedId(snapshot.petId);
    }, [snapshot.petId]);
    const selected = snapshot.pets.find((pet) => pet.id === selectedId) ?? null;
    const scan = async () => {
        setBusy("scan");
        try {
            await perform({ type: "scanCodexPets" });
        }
        finally {
            setBusy(null);
        }
    };
    const importFromDialog = async (kind: "zip" | "folder") => {
        setBusy(kind);
        try {
            const path = kind === "zip" ? await pickImportZip() : await pickImportFolder();
            if (path) {
                await perform({ type: "importPet", path, overwrite: false });
            }
        }
        finally {
            setBusy(null);
        }
    };
    const importCodex = async (path: string) => {
        setBusy(`codex:${path}`);
        try {
            await perform({ type: "importPet", path, overwrite: false });
        }
        finally {
            setBusy(null);
        }
    };
    const exportSelected = async () => {
        if (!selected)
            return;
        setBusy("export");
        try {
            const path = await pickExportZip(`${selected.id}.zip`);
            if (path) {
                await perform({ type: "exportPet", id: selected.id, path });
            }
        }
        finally {
            setBusy(null);
        }
    };
    const deleteSelected = async () => {
        if (!selected)
            return;
        if (!await confirm({ title: "删除宠物", message: `确定删除宠物「${selected.name}」吗？宠物文件会从本地宠物库移除。人格与记忆会保留，重新导入这只宠物可继续使用。`, confirmLabel: "删除" })) {
            return;
        }
        setBusy("delete");
        try {
            await perform({ type: "deletePet", id: selected.id });
        }
        finally {
            setBusy(null);
        }
    };
    return (<div className="page">
      <PageHeader eyebrow="宠物" title="宠物库" description="单击选择、双击切换。可以拖入宠物压缩包或文件夹，也可以从导入菜单添加。" actions={<ActionMenu label={busy === "zip" || busy === "folder" ? "正在导入…" : "导入"} primary disabled={busy !== null} actions={[
                { label: "选择宠物压缩包", hint: "导入压缩包中的宠物", onClick: () => void importFromDialog("zip") },
                { label: "选择宠物文件夹", hint: "导入包含 pet.json 的目录", onClick: () => void importFromDialog("folder") },
                { label: "扫描 Codex 宠物", hint: "复制到本地库，保留原文件", onClick: () => void scan() },
            ]}/>}/>

      <Card title="本地宠物库" description={`${snapshot.pets.length} 只伙伴`}>
        {snapshot.pets.length === 0 ? (<EmptyState title="宠物库为空" description="Petsona 不会内置宠物。导入或从 Codex 扫描后，宠物会显示在这里。"/>) : (<div className="pet-list">
            {snapshot.pets.map((pet) => {
                const active = pet.id === snapshot.petId;
                const selectedRow = pet.id === selectedId;
                return (<button key={pet.id} type="button" className={`pet-row${selectedRow ? " pet-row-selected" : ""}`} onClick={() => setSelectedId(pet.id)} onDoubleClick={() => void perform({ type: "selectPet", id: pet.id })}>
                  <PetThumb path={pet.spritesheet} frameWidth={pet.cellWidth} frameHeight={pet.cellHeight} fallback={pet.name}/>
                  <div className="pet-row-copy">
                    <strong>{pet.name}</strong>
                      </div>
                  <div className="pet-row-tags">

                    {active && <Badge tone="positive">当前</Badge>}
                  </div>
                </button>);
            })}
          </div>)}
        <div className="card-footer pet-actions">
          <span className="muted">
            {selected ? `已选择：${selected.name}` : "选择一只宠物以查看操作"}
          </span>
          <div className="button-group">
            <Button variant="ghost" disabled={!selected || busy !== null} onClick={() => void exportSelected()}>
              {busy === "export" ? "导出中…" : "导出"}
            </Button>
            <Button variant="danger" disabled={!selected || busy !== null} onClick={() => void deleteSelected()}>
              {busy === "delete" ? "删除中…" : "删除"}
            </Button>
            <Button variant="primary" disabled={!selected || selected.id === snapshot.petId || busy !== null} onClick={() => selected && void perform({ type: "selectPet", id: selected.id })}>
              设为当前宠物
            </Button>
          </div>
        </div>
      </Card>

      {snapshot.codexPets.length > 0 && (<Card title="从 Codex 找到的宠物" description={`找到 ${snapshot.codexPets.length} 个候选；双击导入，不会修改 Codex 原始文件。`}>
          <div className="pet-list compact">
            {snapshot.codexPets.map((pet) => (<div className="pet-row pet-row-static" key={pet.id} onDoubleClick={() => void importCodex(pet.path)} title="双击导入">
                <PetThumb path={pet.spritesheet} frameWidth={pet.cellWidth} frameHeight={pet.cellHeight} fallback={pet.name}/>
                <div className="pet-row-copy">
                  <strong>{pet.name}</strong>
                  <span>来自 Codex</span>
                </div>
                <Button variant="secondary" disabled={busy !== null} onClick={() => void importCodex(pet.path)}>
                  {busy === `codex:${pet.path}` ? "导入中…" : "导入"}
                </Button>
              </div>))}
          </div>
        </Card>)}

      {notice && <InlineNotice tone={notice.tone}>{notice.message}</InlineNotice>}
      <HelpText>导入会复制宠物到本地；删除只影响 Petsona 的副本，角色与记忆会保留。</HelpText>

      {conflict && (<Dialog title="这只宠物已经导入过" onClose={() => void run({ type: "clearImportConflict" })}>
          <p>
            本地已有宠物「{conflict.name || conflict.id}」。覆盖会替换 Petsona 宠物库中的副本，
            不会影响 Codex 原始文件。
          </p>
          <div className="modal-actions">
            <Button variant="ghost" onClick={() => void run({ type: "clearImportConflict" })}>
              取消
            </Button>
            <Button variant="danger" onClick={() => {
                void run({
                    type: "importPet",
                    path: conflict.path,
                    overwrite: true,
                });
            }}>
              覆盖导入
            </Button>
          </div>
        </Dialog>)}
      {confirmation}
    </div>);
}
