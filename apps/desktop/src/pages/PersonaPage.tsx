import { useEffect, useRef, useState } from "react";
import {
  Badge,
  Button,
  Card,
  PageHeader,
  SettingRow,
  Switch,
  TextArea,
  TextField,
  InlineNotice,
} from "../components/ui";
import type { PageProps } from "../types";

const TONE_PRESETS = [
  { label: "温和友好", value: "friendly and concise" },
  { label: "简洁直接", value: "concise and direct" },
  { label: "活泼俏皮", value: "playful and upbeat" },
  { label: "专业沉稳", value: "professional and calm" },
  { label: "幽默轻松", value: "humorous and relaxed" },
  { label: "冷静克制", value: "reserved and composed" },
];

export function PersonaPage({ snapshot, run }: PageProps) {
  const persona = snapshot.persona;
  const dirty = useRef(false);
  const [tone, setTone] = useState(persona.traits?.tone ?? "");
  const [emoji, setEmoji] = useState(persona.traits?.emoji ?? true);
  const [greeting, setGreeting] = useState(persona.greeting ?? "");
  const [systemPrompt, setSystemPrompt] = useState(persona.systemPrompt ?? "");
  const [saving, setSaving] = useState(false);

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
    setSaving(true);
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
      setSaving(false);
    }
  };

  const reset = async () => {
    if (!window.confirm("确定把当前宠物的说话方式重置为内置默认值吗？")) return;
    await run({ type: "resetPersona" });
    dirty.current = false;
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="人格"
        title="说话方式"
        description="说话方式跟随当前宠物保存。切换宠物后，每只宠物会恢复自己的设置。"
        actions={
          <div className="button-group">
            <Button variant="ghost" onClick={() => void reset()}>
              重置为内置
            </Button>
            <Button variant="primary" onClick={() => void save()} disabled={saving}>
              {saving ? "保存中…" : "保存"}
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

      <InlineNotice>
        导入 / 导出、复制到其他宠物和聊天记录生成人格属于后续批次；本轮先把日常编辑与保存接通。
      </InlineNotice>
    </div>
  );
}
