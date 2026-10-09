import { useEffect, useRef, useState } from "react";
import {
  Badge,
  Button,
  Card,
  InlineNotice,
  PageHeader,
  SettingRow,
  Switch,
  TextArea,
  TextField,
} from "../components/ui";
import {
  loadSnapshot,
  pickPersonaExport,
  pickPersonaImport,
} from "../lib/api";
import type { PageProps } from "../types";

const TONE_PRESETS = [
  { label: "温和友好", value: "friendly and concise" },
  { label: "简洁直接", value: "concise and direct" },
  { label: "活泼俏皮", value: "playful and upbeat" },
  { label: "专业沉稳", value: "professional and calm" },
  { label: "幽默轻松", value: "humorous and relaxed" },
  { label: "冷静克制", value: "reserved and composed" },
];

type Busy = "save" | "import" | "export" | "copy" | null;

function safeFileName(value: string): string {
  const cleaned = value.trim().replace(/[\\/:*?"<>|]+/g, "_");
  return cleaned || "persona";
}

export function PersonaPage({ snapshot, run }: PageProps) {
  const persona = snapshot.persona;
  const dirty = useRef(false);
  const [tone, setTone] = useState(persona.traits?.tone ?? "");
  const [emoji, setEmoji] = useState(persona.traits?.emoji ?? true);
  const [greeting, setGreeting] = useState(persona.greeting ?? "");
  const [systemPrompt, setSystemPrompt] = useState(persona.systemPrompt ?? "");
  const [busy, setBusy] = useState<Busy>(null);
  const [copyOpen, setCopyOpen] = useState(false);
  const [importConflict, setImportConflict] = useState<{
    path: string;
    message: string;
  } | null>(null);

  useEffect(() => {
    if (dirty.current) return;
    setTone(persona.traits?.tone ?? "");
    setEmoji(persona.traits?.emoji ?? true);
    setGreeting(persona.greeting ?? "");
    setSystemPrompt(persona.systemPrompt ?? "");
  }, [persona]);

  const markDirty = () => {
    dirty.current = true;
  };

  const save = async () => {
    setBusy("save");
    try {
      await run({
        type: "updatePersona",
        patch: {
          tone,
          emoji,
          greeting,
          system_prompt: systemPrompt,
        },
      });
      await run({ type: "savePersona" });
      dirty.current = false;
    } finally {
      setBusy(null);
    }
  };

  const reset = async () => {
    if (!window.confirm("确定把当前宠物的说话方式重置为内置默认值吗？")) return;
    await run({ type: "resetPersona" });
    dirty.current = false;
  };

  const exportPersona = async () => {
    setBusy("export");
    try {
      if (dirty.current) await save();
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
        description="说话方式跟随当前宠物保存。切换宠物后，每只宠物会恢复自己的设置。"
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
            <Button
              variant="secondary"
              disabled={busy !== null}
              onClick={() => void importPersona()}
            >
              {busy === "import" ? "导入中…" : "导入"}
            </Button>
            <Button
              variant="secondary"
              disabled={busy !== null}
              onClick={() => void exportPersona()}
            >
              {busy === "export" ? "导出中…" : "导出"}
            </Button>
            <Button variant="ghost" onClick={() => void reset()}>
              重置为内置
            </Button>
            <Button variant="primary" onClick={() => void save()} disabled={busy !== null}>
              {busy === "save" ? "保存中…" : "保存"}
            </Button>
          </div>
        }
      />

      <Card title="当前绑定" description="说话方式与宠物直接绑定，不再单独维护一份“人格列表”。">
        <div className="binding-row">
          <div>
            <span className="setting-label">宠物</span>
            <strong>{snapshot.petName || "未选择宠物"}</strong>
          </div>
          <div>
            <span className="setting-label">说话方式</span>
            <strong>{persona.name || "默认"}</strong>
          </div>
          <Badge tone={persona.builtin ? "neutral" : "accent"}>
            {persona.builtin ? "内置" : "当前宠物自定义"}
          </Badge>
        </div>
      </Card>

      <Card title="语气" description="选择一个预设，或直接写出你希望的语气。">
        <div className="tone-grid">
          {TONE_PRESETS.map((preset) => (
            <button
              key={preset.value}
              type="button"
              className={`tone-card${tone === preset.value ? " tone-card-active" : ""}`}
              onClick={() => {
                setTone(preset.value);
                markDirty();
              }}
            >
              <strong>{preset.label}</strong>
              <span>{preset.value}</span>
            </button>
          ))}
        </div>
        <SettingRow label="自定义语气" hint="会覆盖上面的预设选择。">
          <TextField
            value={tone}
            onChange={(value) => {
              setTone(value);
              markDirty();
            }}
            placeholder="例如：温柔、简短，偶尔主动关心我"
          />
        </SettingRow>
        <SettingRow label="允许使用 emoji" hint="默认开启，只影响后续生成的回复。">
          <Switch
            label="允许使用 emoji"
            checked={emoji}
            onChange={(value) => {
              setEmoji(value);
              markDirty();
            }}
          />
        </SettingRow>
      </Card>

      <Card title="问候文案" description="空闲问候触发时优先使用这句话；没有配置模型时也可使用。">
        <TextArea
          value={greeting}
          onChange={(value) => {
            setGreeting(value);
            markDirty();
          }}
          rows={3}
          placeholder="例如：我在这儿呢，需要我陪你聊聊吗？"
        />
      </Card>

      <Card title="高级：系统提示词" description="会作为模型对话的基础指令。普通使用无需修改。">
        <TextArea
          value={systemPrompt}
          onChange={(value) => {
            setSystemPrompt(value);
            markDirty();
          }}
          rows={7}
          placeholder="描述宠物的身份、边界和回答方式"
        />
      </Card>

      {snapshot.status && (
        <p className="status-line">
          <Badge tone={statusIsError ? "warning" : "neutral"}>状态</Badge>
          {snapshot.status}
        </p>
      )}

      <InlineNotice>
        导入与导出的都是 Petsona 人格 JSON；导入会切换当前宠物的说话方式，覆盖前会二次确认。
        复制到其他宠物会创建独立副本，之后两边分别编辑、互不影响。
      </InlineNotice>

      {copyOpen && (
        <div className="modal-backdrop" role="presentation">
          <div className="modal" role="dialog" aria-modal="true" aria-labelledby="copy-persona-title">
            <h2 id="copy-persona-title">复制到其他宠物</h2>
            <p>选择目标宠物。会创建当前说话方式的独立副本，不改变源宠物。</p>
            <div className="copy-target-list">
              {otherPets.map((pet) => (
                <button
                  key={pet.id}
                  type="button"
                  onClick={() => {
                    setCopyOpen(false);
                    void run({ type: "copyPersonaToPet", targetPetId: pet.id });
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
          </div>
        </div>
      )}

      {importConflict && (
        <div className="modal-backdrop" role="presentation">
          <div className="modal" role="dialog" aria-modal="true" aria-labelledby="persona-conflict-title">
            <h2 id="persona-conflict-title">人格 ID 已存在</h2>
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
          </div>
        </div>
      )}
    </div>
  );
}
