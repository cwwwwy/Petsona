import { useCallback, useEffect, useRef, useState } from "react";
import { SaveStatus, ActionFooter, HelpText, Button, Card, InlineNotice, PageHeader, TextArea, TextField, } from "../components/ui";
import { ActionMenu } from "../components/ActionMenu";
import { Dialog, useConfirm } from "../components/ConfirmDialog";
import { PersonaSourcePanel } from "../components/PersonaSourcePanel";
import { pickPersonaExport, pickPersonaImport, } from "../lib/api";
import type { PageProps } from "../types";
type Busy = "autosave" | "import" | "export" | "reset" | null;
function safeFileName(value: string): string {
    const cleaned = value.trim().replace(/[\\/:*?"<>|]+/g, "_");
    return cleaned || "persona";
}
export function PersonaPage({ snapshot, run }: PageProps) {
    const persona = snapshot.persona;
    const dirty = useRef(false);
    const personaId = useRef(persona.id);
    const latestEdit = useRef({ text: "", name: "", petId: "" });
    const { confirm, confirmation } = useConfirm();
    const [systemPrompt, setSystemPrompt] = useState(persona.characterDefinition ?? persona.systemPrompt ?? "");
    const [name, setName] = useState(persona.name);
    const [previewPrompt, setPreviewPrompt] = useState("今天有点累，你会怎么陪我聊聊？");
    const [previewVersion, setPreviewVersion] = useState<{
        request: string;
        text: string;
    } | null>(null);
    const [saveNotice, setSaveNotice] = useState("已保存");
    const [saveError, setSaveError] = useState(false);
    latestEdit.current = { text: systemPrompt, name, petId: snapshot.petId };
    const [busy, setBusy] = useState<Busy>(null);
    const [operationNotice, setOperationNotice] = useState<{
        tone: "success" | "danger";
        message: string;
    } | null>(null);
    const [copyOpen, setCopyOpen] = useState(false);
    const [importConflict, setImportConflict] = useState<{
        path: string;
        message: string;
    } | null>(null);
    useEffect(() => {
        if (personaId.current !== persona.id) {
            personaId.current = persona.id;
            dirty.current = false;
            setSaveError(false);
            setSaveNotice("已保存");
        }
        if (dirty.current)
            return;
        setSystemPrompt(persona.characterDefinition ?? persona.systemPrompt ?? "");
        setName(persona.name);
    }, [persona]);
    useEffect(() => () => {
        const edit = latestEdit.current;
        if (dirty.current && edit.text.trim() && edit.name.trim()) {
            void run({ type: "updatePersona", patch: { pet_id: edit.petId, name: edit.name.trim(), character_definition: edit.text } });
        }
    }, [run]);
    const markDirty = () => {
        dirty.current = true;
        setSaveNotice("等待保存…");
        setSaveError(false);
        setPreviewVersion(null);
    };
    const persistDraft = useCallback(async () => {
        if (!name.trim() || !systemPrompt.trim()) {
            setSaveNotice("角色名称和设定不能为空，原设定已保留。");
            setSaveError(true);
            return false;
        }
        const submitted = { name, text: systemPrompt };
        setBusy("autosave");
        try {
            const result = await run({
                type: "updatePersona",
                patch: {
                    pet_id: snapshot.petId,
                    character_definition: systemPrompt,
                    name: name.trim(),
                },
            });
            if (!result) {
                setSaveNotice("尚未确认保存，编辑内容已保留。");
                setSaveError(true);
                return false;
            }
            const next = result;
            if (next.status.includes("失败")) {
                setSaveNotice("保存失败，编辑内容已保留，请重试。");
                setSaveError(true);
                return false;
            }
            if (latestEdit.current.text === submitted.text && latestEdit.current.name === submitted.name && next.petId === latestEdit.current.petId && next.persona.characterDefinition === submitted.text && next.persona.name === submitted.name.trim()) {
                dirty.current = false;
                setSaveNotice("已保存");
                setSaveError(false);
                return true;
            }
            return false;
        }
        finally {
            setBusy(null);
        }
    }, [run, systemPrompt, name, snapshot.petId]);
    useEffect(() => {
        if (!dirty.current)
            return;
        const targetPersonaId = persona.id;
        const timer = window.setTimeout(() => {
            if (dirty.current && personaId.current === targetPersonaId)
                void persistDraft();
        }, 450);
        return () => window.clearTimeout(timer);
    }, [persistDraft, persona.id, systemPrompt, name]);
    const reset = async () => {
        if (!await confirm({ title: "恢复默认设定", message: "恢复默认说话方式，角色名称和相处记忆会保留。", confirmLabel: "恢复默认" }))
            return;
        const previousEdit = { name, text: systemPrompt };
        dirty.current = false;
        setBusy("reset");
        const next = await run({ type: "resetPersona" });
        if (!next || next.status.includes("失败")) {
            dirty.current = true;
            setName(previousEdit.name);
            setSystemPrompt(previousEdit.text);
            setSaveError(true);
            setSaveNotice("未能确认恢复默认设定，请重试。");
        }
        else {
            dirty.current = false;
            setName(next.persona.name);
            setSystemPrompt(next.persona.characterDefinition ?? next.persona.systemPrompt ?? "");
            setSaveError(false);
            setSaveNotice("已保存");
        }
        setBusy(null);
    };
    const exportPersona = async () => {
        setBusy("export");
        try {
            if ((dirty.current || saveError) && !await persistDraft())
                return;
            const path = await pickPersonaExport(`${safeFileName(persona.name || persona.id)}.json`);
            if (!path)
                return;
            const next = await run({ type: "exportPersona", id: persona.id, path });
            setOperationNotice(!next || next.status.includes("失败") ? { tone: "danger", message: "未能确认角色备份已导出，请重试。" } : { tone: "success", message: "角色备份已导出。" });
        }
        finally {
            setBusy(null);
        }
    };
    const importPersona = async () => {
        setBusy("import");
        try {
            if ((dirty.current || saveError) && !await persistDraft())
                return;
            const path = await pickPersonaImport();
            if (!path)
                return;
            dirty.current = false;
            const next = await run({ type: "importPersona", path, overwrite: false });
            if (!next) {
                setOperationNotice({ tone: "danger", message: "尚未确认导入，原编辑内容仍在这里。" });
                return;
            }
            if (next.status.includes("失败")) {
                setOperationNotice({ tone: "danger", message: "角色导入未完成，原设定保持不变。" });
                return;
            }
            if (next.status.includes("already exists") ||
                next.status.includes("已存在")) {
                setImportConflict({ path, message: next.status });
            }
        }
        finally {
            setBusy(null);
        }
    };
    const otherPets = snapshot.pets.filter((pet) => pet.id !== snapshot.petId);
    return (<div className="page">
      <PageHeader eyebrow="角色" title="角色设定" description="塑造它的身份与说话方式。修改自动保存，每只宠物保留自己的角色。" actions={<div className="button-group">
            <Button variant="secondary" disabled={busy !== null || otherPets.length === 0} title={otherPets.length === 0 ? "至少需要两只本地宠物" : "复制到其他宠物"} onClick={() => { void (async () => { if ((dirty.current || saveError) && !await persistDraft())
            return; setCopyOpen(true); })(); }}>
              复制到…
            </Button>
            <ActionMenu disabled={busy !== null} actions={[
                { label: "导入", onClick: () => void importPersona() },
                { label: "导出", onClick: () => void exportPersona() },
                { label: "恢复默认设定", onClick: () => void reset() },
            ]}/>
          </div>}/>

      {operationNotice && <InlineNotice tone={operationNotice.tone}>{operationNotice.message}</InlineNotice>}
      {!snapshot.hasPet && <HelpText>先在宠物页导入一只伙伴，再为它设置角色。</HelpText>}
      <Card title="当前生效的角色" description="描述它是谁、怎样说话、与你的关系，也可以加入典型表达。">
        <TextField label="角色名称" value={name} disabled={!snapshot.hasPet} onChange={(value) => { setName(value); markDirty(); }}/>
        <TextArea label="角色设定" disabled={!snapshot.hasPet} value={systemPrompt} rows={6} onChange={(value) => { setSystemPrompt(value); markDirty(); }} placeholder="例如：你是一位沉静、偶尔带点幽默的旅人。说话简短，先听我说完，再给回应……"/>
        <ActionFooter status={<SaveStatus phase={saveError ? "error" : busy === "autosave" ? "saving" : dirty.current ? "pending" : "saved"} message={saveError ? saveNotice : undefined}/>}>
          {saveError && <Button variant="secondary" disabled={busy !== null} onClick={() => void persistDraft()}>重试保存</Button>}
        </ActionFooter>
        <details className="advanced-disclosure">
          <summary>试聊当前设定</summary>
          <TextArea label="你想对它说什么" value={previewPrompt} onChange={setPreviewPrompt} rows={2}/>
          <Button disabled={snapshot.personaPreview.inFlight || !previewPrompt.trim() || !systemPrompt.trim()} onClick={() => {
            const request = crypto.randomUUID();
            setPreviewVersion({ request, text: systemPrompt });
            void run({ type: "previewPersonaDraft", request_id: request, pet_id: snapshot.petId, draft_id: "", prompt: previewPrompt, character_definition: systemPrompt });
        }}>{snapshot.personaPreview.inFlight ? "正在回复…" : "试聊"}</Button>
          <p className="muted">试聊不会写入聊天记录或记忆。</p>
          {previewVersion?.text === systemPrompt && previewVersion.request === snapshot.personaPreview.requestId && snapshot.personaPreview.error && <InlineNotice tone="danger">{snapshot.personaPreview.error}</InlineNotice>}
          {previewVersion?.text === systemPrompt && previewVersion.request === snapshot.personaPreview.requestId && snapshot.personaPreview.text && <div className="preview-answer">{snapshot.personaPreview.text}</div>}
        </details>
      </Card>
      <PersonaSourcePanel snapshot={snapshot} run={run} beforeGenerate={() => dirty.current || saveError ? persistDraft() : Promise.resolve(true)} onApplied={() => { dirty.current = false; setSaveNotice("已应用并保存"); }}/>

      <HelpText>角色跟随当前宠物保存。导出可用来备份，普通修改不会清空相处记忆。</HelpText>

      {copyOpen && (<Dialog title="复制到其他宠物" onClose={() => setCopyOpen(false)}>
          <p>选择目标宠物。会创建当前说话方式的独立副本，不改变源宠物。</p>
          <div className="copy-target-list">
            {otherPets.map((pet) => (<button key={pet.id} type="button" onClick={() => {
                    setCopyOpen(false);
                    void (async () => { const next = await run({ type: "copyPersonaToPet", target_pet_id: pet.id }); setOperationNotice(!next || next.status.includes("失败") ? { tone: "danger", message: "复制尚未完成，请重试。" } : { tone: "success", message: `已复制给${pet.name}。` }); })();
                }}>
                <strong>{pet.name}</strong>
              </button>))}
          </div>
          <div className="modal-actions">
            <Button variant="ghost" onClick={() => setCopyOpen(false)}>
              取消
            </Button>
          </div>
        </Dialog>)}

      {importConflict && (<Dialog title="发现已有角色" onClose={() => setImportConflict(null)}>
          <p>
            已保存过这个角色。继续导入会替换它的设定，其他宠物的独立副本不受影响。
          </p>
          <p className="modal-detail">{importConflict.message}</p>
          <div className="modal-actions">
            <Button variant="ghost" onClick={() => setImportConflict(null)}>
              取消
            </Button>
            <Button variant="danger" onClick={() => {
                const path = importConflict.path;
                setImportConflict(null);
                void run({ type: "importPersona", path, overwrite: true });
            }}>
              覆盖导入
            </Button>
          </div>
        </Dialog>)}
      {confirmation}
    </div>);
}
