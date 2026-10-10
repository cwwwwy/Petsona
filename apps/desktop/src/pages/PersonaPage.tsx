import { useCallback, useEffect, useRef, useState } from "react";
import {
  Badge,
  Button,
  Card,
  InlineNotice,
  PageHeader,
  TextArea,
} from "../components/ui";
import { ActionMenu } from "../components/ActionMenu";
import { Dialog, useConfirm } from "../components/ConfirmDialog";
import { PersonaSourcePanel } from "../components/PersonaSourcePanel";
import {
  loadSnapshot,
  pickPersonaExport,
  pickPersonaImport,
} from "../lib/api";
import type { PageProps } from "../types";

// Matches the existing builtin prompt; no business field is changed.
const DEFAULT_SYSTEM_PROMPT = "你是一只住在用户桌面上的宠物伙伴。你友好、好奇、说话简洁。你可以陪用户聊天、帮忙梳理思路，但不要编造事实。回答时优先使用用户使用的语言。";

type Busy = "autosave" | "import" | "export" | null;

function safeFileName(value: string): string {
  const cleaned = value.trim().replace(/[\\/:*?"<>|]+/g, "_");
  return cleaned || "persona";
}

export function PersonaPage({ snapshot, run }: PageProps) {
  const persona = snapshot.persona;
  const dirty = useRef(false);
  const personaId = useRef(persona.id);
  const { confirm, confirmation } = useConfirm();
  const [systemPrompt, setSystemPrompt] = useState(persona.systemPrompt ?? "");
  const [busy, setBusy] = useState<Busy>(null);
  const [copyOpen, setCopyOpen] = useState(false);
  const [importConflict, setImportConflict] = useState<{
    path: string;
    message: string;
  } | null>(null);

  useEffect(() => {
    if (personaId.current !== persona.id) {
      personaId.current = persona.id;
      dirty.current = false;
    }
    if (dirty.current) return;
    setSystemPrompt(persona.systemPrompt ?? "");
  }, [persona]);

  const markDirty = () => {
    dirty.current = true;
  };

  const persistDraft = useCallback(async () => {
    setBusy("autosave");
    try {
      await run({
        type: "updatePersona",
        patch: {
          system_prompt: systemPrompt,
        },
      });
      await run({ type: "savePersona" });
      dirty.current = false;
    } finally {
      setBusy(null);
    }
  }, [run, systemPrompt]);

  useEffect(() => {
    if (!dirty.current) return;
    const targetPersonaId = persona.id;
    const timer = window.setTimeout(() => {
      if (personaId.current === targetPersonaId) void persistDraft();
    }, 450);
    return () => window.clearTimeout(timer);
  }, [persistDraft, persona.id, systemPrompt]);

  const reset = async () => {
    if (!await confirm({ title: "重置说话方式", message: "确定把当前宠物的说话方式重置为内置默认值吗？宠物记忆会保留。", confirmLabel: "重置" })) return;
    await run({ type: "resetPersona" });
    dirty.current = false;
  };

  const exportPersona = async () => {
    setBusy("export");
    try {
      if (dirty.current) await persistDraft();
      const path = await pickPersonaExport(`${safeFileName(persona.name || persona.id)}.json`);
      if (!path) return;
      await run({ type: "exportPersona", id: persona.id, path });
    } finally {
      setBusy(null);
    }
  };

  const importPersona = async () => {
    setBusy("import");
    try {
      const path = await pickPersonaImport();
      if (!path) return;
      await run({ type: "importPersona", path, overwrite: false });
      const next = await loadSnapshot();
      if (
        next.status.includes("already exists") ||
        next.status.includes("已存在")
      ) {
        setImportConflict({ path, message: next.status });
      }
    } finally {
      setBusy(null);
    }
  };

  const otherPets = snapshot.pets.filter((pet) => pet.id !== snapshot.petId);
  const statusIsError = snapshot.status.includes("失败") || snapshot.status.includes("已存在");

  return (
    <div className="page">
      <PageHeader
        eyebrow="人格"
        title="说话方式"
        description="说话方式跟随当前宠物保存。修改后自动保存，切换宠物后每只宠物恢复自己的设置。"
        actions={
          <div className="button-group">
            <Button
              variant="secondary"
              disabled={busy !== null || otherPets.length === 0}
              title={otherPets.length === 0 ? "至少需要两只本地宠物" : "复制到其他宠物"}
              onClick={() => setCopyOpen(true)}
            >
              复制到…
            </Button>
            <ActionMenu disabled={busy !== null} actions={[
              { label: "导入", onClick: () => void importPersona() },
              { label: "导出", onClick: () => void exportPersona() },
              { label: "重置为内置", onClick: () => void reset() },
            ]} />
          </div>
        }
      />

      <PersonaSourcePanel
        snapshot={snapshot}
        run={run}
        onApplied={() => {
          dirty.current = false;
        }}
      />

      <Card title="高级：系统提示词" description="会作为模型对话的基础指令。普通使用无需修改。">
        <details className="advanced-disclosure">
          <summary>展开编辑 <Badge>{systemPrompt.trim() === DEFAULT_SYSTEM_PROMPT ? "默认" : "已自定义"}</Badge></summary>
          <TextArea
            label="系统提示词"
            value={systemPrompt}
            onChange={(value) => {
              setSystemPrompt(value);
              markDirty();
            }}
            rows={7}
            placeholder="描述宠物的身份、边界和回答方式"
          />
        </details>
      </Card>

      {snapshot.status && (
        <p className="status-line">
          <Badge tone={statusIsError ? "warning" : "neutral"}>状态</Badge>
          {snapshot.status}
        </p>
      )}

      <p className="status-line" role="status">{busy === "autosave" ? "自动保存中…" : "修改后自动保存"}</p>
      <InlineNotice>
        系统提示词会自动保存；固定问候文案已归入「外观与交互」页。
        导入与导出的都是 Petsona 人格 JSON；导入会更新当前宠物的说话方式，覆盖前会二次确认。
      </InlineNotice>

      {copyOpen && (
        <Dialog title="复制到其他宠物" onClose={() => setCopyOpen(false)}>
          <p>选择目标宠物。会创建当前说话方式的独立副本，不改变源宠物。</p>
          <div className="copy-target-list">
            {otherPets.map((pet) => (
              <button
                key={pet.id}
                type="button"
                onClick={() => {
                  setCopyOpen(false);
                  void run({ type: "copyPersonaToPet", target_pet_id: pet.id });
                }}
              >
                <strong>{pet.name}</strong>
                <span className="mono">{pet.id}</span>
              </button>
            ))}
          </div>
          <div className="modal-actions">
            <Button variant="ghost" onClick={() => setCopyOpen(false)}>
              取消
            </Button>
          </div>
        </Dialog>
      )}

      {importConflict && (
        <Dialog title="人格 ID 已存在" onClose={() => setImportConflict(null)}>
          <p>
            本地已经存在同 ID 的人格文件。覆盖会替换现有文件，但不会影响其他宠物的绑定副本。
          </p>
          <p className="modal-detail">{importConflict.message}</p>
          <div className="modal-actions">
            <Button variant="ghost" onClick={() => setImportConflict(null)}>
              取消
            </Button>
            <Button
              variant="danger"
              onClick={() => {
                const path = importConflict.path;
                setImportConflict(null);
                void run({ type: "importPersona", path, overwrite: true });
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
