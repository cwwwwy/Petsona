import { useEffect, useMemo, useRef, useState } from "react";
import {
  Badge,
  Button,
  Card,
  InlineNotice,
  SelectField,
  SettingRow,
  TextArea,
  TextField,
} from "../components/ui";
import { pickPersonaSource } from "../lib/api";
import type {
  PersonaStyleProfile,
  PageProps,
  SettingsAction,
} from "../types";

type Mode = "profile" | "chat";

const EMPTY_STYLE: PersonaStyleProfile = {
  personality: "",
  expressionStyle: "",
  responseHabits: "",
  relationship: "",
  examples: [],
};

function uuid(): string {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() || "人格资料";
}

function formatOf(path: string): "txt" | "json" {
  return path.toLowerCase().endsWith(".json") ? "json" : "txt";
}

function sourceLabel(source: Record<string, unknown>): string {
  const label = source.label;
  const kind = source.kind;
  return [label, kind].filter((value) => typeof value === "string" && value).join(" · ");
}

export function PersonaSourcePanel({
  snapshot,
  run,
  onApplied,
}: Pick<PageProps, "snapshot" | "run"> & { onApplied?: () => void }) {
  const source = snapshot.personaSource;
  const draftState = snapshot.personaDraft;
  const preview = snapshot.personaPreview;
  const draft = draftState.draft;
  const [mode, setMode] = useState<Mode>("profile");
  const [profileName, setProfileName] = useState("");
  const [description, setDescription] = useState("");
  const [pasteText, setPasteText] = useState("");
  const [pasteFormat, setPasteFormat] = useState<"txt" | "json">("txt");
  const [sourceName, setSourceName] = useState("");
  const [targetSpeaker, setTargetSpeaker] = useState("");
  const [busy, setBusy] = useState<"parse" | "generate" | "apply" | "preview" | null>(null);
  const [notice, setNotice] = useState("");
  const [draftName, setDraftName] = useState("");
  const [style, setStyle] = useState<PersonaStyleProfile>(EMPTY_STYLE);
  const [previewPrompt, setPreviewPrompt] = useState("你好，介绍一下你自己吧");
  const initializedDraft = useRef("");

  const speakerKey = useMemo(
    () => (source.speakers ?? []).map((speaker) => speaker.name).join("\u0000"),
    [source.speakers],
  );

  useEffect(() => {
    const speakers = source.speakers ?? [];
    if (speakers.length && !speakers.some((speaker) => speaker.name === targetSpeaker)) {
      setTargetSpeaker(speakers[0].name);
    }
  }, [source.id, speakerKey, targetSpeaker]);

  useEffect(() => {
    if (!draft || initializedDraft.current === draft.id) return;
    initializedDraft.current = draft.id;
    setDraftName(draft.name);
    setStyle({
      personality: draft.style?.personality ?? "",
      expressionStyle: draft.style?.expressionStyle ?? "",
      responseHabits: draft.style?.responseHabits ?? "",
      relationship: draft.style?.relationship ?? "",
      examples: draft.style?.examples ?? [],
    });
  }, [draft]);

  const runQuiet = async (action: SettingsAction, phase: NonNullable<typeof busy>) => {
    setNotice("");
    setBusy(phase);
    try {
      await run(action);
    } finally {
      setBusy(null);
    }
  };

  const parseText = async () => {
    if (!pasteText.trim()) {
      setNotice("请先粘贴 TXT 或 JSON 聊天记录。");
      return;
    }
    await runQuiet(
      {
        type: "parsePersonaSource",
        request_id: uuid(),
        label: sourceName.trim() || "粘贴的聊天记录",
        format: pasteFormat,
        path: null,
        text: pasteText,
      },
      "parse",
    );
  };

  const parseFile = async () => {
    const path = await pickPersonaSource();
    if (!path) return;
    await runQuiet(
      {
        type: "parsePersonaSource",
        request_id: uuid(),
        label: sourceName.trim() || fileName(path),
        format: formatOf(path),
        path,
        text: null,
      },
      "parse",
    );
  };

  const generate = async () => {
    const isProfile = mode === "profile";
    if (isProfile && !profileName.trim()) {
      setNotice("请输入参考人物或风格名称。");
      return;
    }
    if (!isProfile && (!source.id || !targetSpeaker)) {
      setNotice("请先解析聊天记录并选择目标说话人。");
      return;
    }
    await runQuiet(
      {
        type: "generatePersonaProfile",
        request_id: uuid(),
        source_id: isProfile ? "" : source.id ?? "",
        kind: isProfile ? "public_figure" : "chat_import",
        label: isProfile ? profileName.trim() : (source.label ?? sourceName) || "聊天记录",
        description: description.trim(),
        target_speaker: isProfile ? "" : targetSpeaker,
        target_speaker_label: isProfile ? "" : targetSpeaker,
        start_index: 0,
        end_index: isProfile ? 0 : source.messageCount ?? 0,
      },
      "generate",
    );
  };

  const apply = async () => {
    if (!draft) return;
    if (!draftName.trim()) {
      setNotice("草稿名称不能为空。");
      return;
    }
    await runQuiet(
      {
        type: "applyPersonaDraft",
        draft_id: draft.id,
        pet_id: snapshot.petId,
        name: draftName.trim(),
        style,
      },
      "apply",
    );
    onApplied?.();
    setNotice("说话方式已应用；宠物记忆保持不变。");
  };

  const previewDraft = async () => {
    if (!draft || !previewPrompt.trim()) return;
    await runQuiet(
      {
        type: "previewPersonaDraft",
        request_id: uuid(),
        pet_id: snapshot.petId,
        draft_id: draft.id,
        prompt: previewPrompt,
      },
      "preview",
    );
  };

  const clearDraft = async () => {
    await runQuiet({ type: "clearPersonaDraft" }, "apply");
    initializedDraft.current = "";
    setNotice("草稿已清空。");
  };

  const draftSource = draft?.source as Record<string, unknown> | undefined;
  const generating = draftState.generating || busy === "generate";
  const parsing = source.parsing || busy === "parse";

  return (
    <>
      <Card
        title="从资料学习说话方式"
        description="可以用人物描述、粘贴的聊天文字，或 TXT/JSON 文件生成草稿。生成不会修改当前宠物。"
      >
        <div className="source-tabs">
          <button
            type="button"
            className={mode === "profile" ? "source-tab-active" : ""}
            onClick={() => setMode("profile")}
          >
            <strong>人物描述</strong>
            <span>输入名字和风格说明</span>
          </button>
          <button
            type="button"
            className={mode === "chat" ? "source-tab-active" : ""}
            onClick={() => setMode("chat")}
          >
            <strong>聊天记录</strong>
            <span>粘贴或导入 TXT / JSON</span>
          </button>
        </div>

        {mode === "profile" ? (
          <>
            <SettingRow label="人物 / 风格名称">
              <TextField
                value={profileName}
                onChange={setProfileName}
                placeholder="例如：冷静但幽默的技术伙伴"
              />
            </SettingRow>
            <SettingRow stacked label="补充说明">
              <TextArea
                value={description}
                onChange={setDescription}
                rows={4}
                placeholder="描述性格、表达方式、关系边界和你喜欢的回应习惯"
              />
            </SettingRow>
          </>
        ) : (
          <>
            <SettingRow label="样本名称">
              <TextField
                value={sourceName}
                onChange={setSourceName}
                placeholder="例如：和朋友的聊天记录"
              />
            </SettingRow>
            <SettingRow label="粘贴格式">
              <SelectField
                value={pasteFormat}
                options={[
                  { value: "txt", label: "TXT / 纯文本" },
                  { value: "json", label: "JSON 消息数组" },
                ]}
                onChange={setPasteFormat}
              />
            </SettingRow>
            <SettingRow stacked label="粘贴聊天记录">
              <TextArea
                value={pasteText}
                onChange={setPasteText}
                rows={6}
                placeholder="每行可以写成「说话人：消息内容」，JSON 请使用标准消息数组。"
              />
            </SettingRow>
            <div className="source-actions">
              <Button variant="secondary" disabled={busy !== null} onClick={() => void parseFile()}>
                选择 TXT / JSON 文件
              </Button>
              <Button variant="secondary" disabled={parsing} onClick={() => void parseText()}>
                {parsing ? "解析中…" : "解析粘贴内容"}
              </Button>
            </div>
            {source.id && (
              <div className="source-summary">
                <Badge tone="positive">已解析</Badge>
                <span>
                  {source.label || sourceName || "聊天记录"} · {source.messageCount ?? 0} 条消息
                </span>
                {!!source.speakers?.length && (
                  <span>说话人：{source.speakers.map((speaker) => speaker.name).join("、")}</span>
                )}
              </div>
            )}
            {!!source.preview?.length && (
              <div className="source-preview">
                {source.preview.slice(0, 3).map((message, index) => (
                  <p key={`${message.speaker}-${index}`}>
                    <strong>{message.speaker}</strong>
                    {message.text}
                  </p>
                ))}
              </div>
            )}
            {source.id && !!source.speakers?.length && (
              <SettingRow label="目标说话人" hint="至少需要该说话人的 3 条消息。">
                <SelectField
                  value={targetSpeaker}
                  options={source.speakers.map((speaker) => ({
                    value: speaker.name,
                    label: `${speaker.name}（${speaker.count} 条）`,
                  }))}
                  onChange={setTargetSpeaker}
                />
              </SettingRow>
            )}
          </>
        )}

        <div className="source-actions">
          <Button
            variant="primary"
            disabled={generating || parsing || busy !== null}
            onClick={() => void generate()}
          >
            {generating ? "生成中…" : "生成人格草稿"}
          </Button>
        </div>
        {source.error && <InlineNotice tone="danger">{source.error}</InlineNotice>}
        {draftState.error && <InlineNotice tone="danger">{draftState.error}</InlineNotice>}
        {notice && <InlineNotice tone="warning">{notice}</InlineNotice>}
      </Card>

      {draft && (
        <Card
          title="人格草稿：试聊后应用"
          description="先试聊确认风格，再应用到当前宠物。应用不会清空或覆盖宠物记忆。"
          tone="danger"
        >
          <SettingRow label="草稿名称">
            <TextField value={draftName} onChange={setDraftName} />
          </SettingRow>
          <SettingRow stacked label="性格">
            <TextArea
              value={style.personality}
              onChange={(value) => setStyle((current) => ({ ...current, personality: value }))}
              rows={2}
            />
          </SettingRow>
          <SettingRow stacked label="表达方式">
            <TextArea
              value={style.expressionStyle}
              onChange={(value) => setStyle((current) => ({ ...current, expressionStyle: value }))}
              rows={2}
            />
          </SettingRow>
          <SettingRow stacked label="回应习惯">
            <TextArea
              value={style.responseHabits}
              onChange={(value) => setStyle((current) => ({ ...current, responseHabits: value }))}
              rows={2}
            />
          </SettingRow>
          <SettingRow stacked label="与用户的关系">
            <TextArea
              value={style.relationship}
              onChange={(value) => setStyle((current) => ({ ...current, relationship: value }))}
              rows={2}
            />
          </SettingRow>
          <SettingRow stacked label="表达示例（每行一条）">
            <TextArea
              value={style.examples.join("\n")}
              onChange={(value) =>
                setStyle((current) => ({
                  ...current,
                  examples: value
                    .split("\n")
                    .map((example) => example.trim())
                    .filter(Boolean),
                }))
              }
              rows={3}
            />
          </SettingRow>
          <div className="draft-source">
            <span className="setting-label">来源</span>
            <span className="muted">{sourceLabel(draftSource ?? {}) || "人格资料"}</span>
          </div>
          <SettingRow stacked label="试聊">
            <div className="preview-control">
              <TextArea value={previewPrompt} onChange={setPreviewPrompt} rows={2} />
              <Button
                variant="secondary"
                disabled={preview.inFlight || busy !== null}
                onClick={() => void previewDraft()}
              >
                {preview.inFlight ? "生成中…" : "试聊一句"}
              </Button>
            </div>
          </SettingRow>
          {preview.error && <InlineNotice tone="danger">{preview.error}</InlineNotice>}
          {preview.text && <div className="preview-answer">{preview.text}</div>}
          <div className="source-actions">
            <Button variant="ghost" disabled={busy !== null} onClick={() => void clearDraft()}>
              放弃草稿
            </Button>
            <Button variant="primary" disabled={busy !== null} onClick={() => void apply()}>
              {busy === "apply" ? "应用中…" : "应用到当前宠物"}
            </Button>
          </div>
        </Card>
      )}
    </>
  );
}
