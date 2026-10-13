import { useEffect, useMemo, useRef, useState } from "react";
import { Badge, Button, Card, ActionFooter, HelpText, InlineNotice, SelectField, SettingRow, TextArea, TextField, Switch, } from "../components/ui";
import { useConfirm } from "./ConfirmDialog";
import { loadSnapshot, pickPersonaSource } from "../lib/api";
import type { PersonaStyleProfile, PageProps, SettingsAction, } from "../types";
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
    return [label, kind === "chat_import" ? "聊天资料" : "人物描述"].filter((value) => typeof value === "string" && value).join(" · ");
}
export function PersonaSourcePanel({ snapshot, run, onApplied, beforeGenerate, }: Pick<PageProps, "snapshot" | "run"> & {
    onApplied?: () => void;
    beforeGenerate?: () => Promise<boolean>;
}) {
    const { confirm, confirmation } = useConfirm();
    const [keepMemory, setKeepMemory] = useState(true);
    const source = snapshot.personaSource;
    const draftState = snapshot.personaDraft;
    const preview = snapshot.personaPreview;
    const draft = draftState.draft;
    const [mode, setMode] = useState<Mode>("profile");
    const [profileName, setProfileName] = useState("");
    const [description, setDescription] = useState("");
    const [pasteText, setPasteText] = useState("");
    const [sourceName, setSourceName] = useState("");
    const [targetSpeaker, setTargetSpeaker] = useState("");
    const [busy, setBusy] = useState<"parse" | "generate" | "apply" | "preview" | null>(null);
    const [notice, setNotice] = useState("");
    const [noticeTone, setNoticeTone] = useState<"info" | "success" | "warning" | "danger">("info");
    const draftRef = useRef<HTMLElement>(null);
    const [draftName, setDraftName] = useState("");
    const [definition, setDefinition] = useState("");
    const [tested, setTested] = useState<{
        request: string;
        text: string;
    } | null>(null);
    const [previewPrompt, setPreviewPrompt] = useState("你好，介绍一下你自己吧");
    const initializedDraft = useRef("");
    const speakerKey = useMemo(() => (source.speakers ?? []).map((speaker) => speaker.name).join("\u0000"), [source.speakers]);
    useEffect(() => {
        const speakers = source.speakers ?? [];
        if (speakers.length && !speakers.some((speaker) => speaker.name === targetSpeaker)) {
            setTargetSpeaker(speakers[0].name);
        }
    }, [source.id, speakerKey, targetSpeaker]);
    useEffect(() => {
        if (!draft || initializedDraft.current === draft.id)
            return;
        initializedDraft.current = draft.id;
        requestAnimationFrame(() => { if (draftRef.current?.getClientRects().length) {
            draftRef.current.focus();
            draftRef.current.scrollIntoView({ block: "start", behavior: "instant" });
        } });
        setDraftName(draft.name);
        const style = draft.style;
        setDefinition([`角色名称：${draft.name}`, ...[["性格与身份", style.personality], ["说话方式", style.expressionStyle], ["回应习惯", style.responseHabits], ["与你的关系", style.relationship]].filter(([, value]) => value?.trim()).map(([label, value]) => `${label}：${value}`), style.examples.length ? `典型表达（风格参考）：\n${style.examples.join("\n")}` : ""].filter(Boolean).join("\n\n"));
    }, [draft]);
    const runQuiet = async (action: SettingsAction, phase: NonNullable<typeof busy>) => {
        setNotice("");
        setNoticeTone("info");
        setBusy(phase);
        try {
            const next = await run(action);
            if (!next) {
                setNotice("操作尚未确认，内容已保留，请重试。");
                setNoticeTone("danger");
                return false;
            }
            return true;
        }
        finally {
            setBusy(null);
        }
    };
    const parseText = async () => {
        if (!pasteText.trim()) {
            setNotice("请先粘贴聊天记录。");
            return;
        }
        await runQuiet({
            type: "parsePersonaSource",
            request_id: uuid(),
            label: sourceName.trim() || "粘贴的聊天记录",
            format: /^[\[{]/.test(pasteText.trim()) ? "json" : "txt",
            path: null,
            text: pasteText,
        }, "parse");
    };
    const parseFile = async () => {
        const path = await pickPersonaSource();
        if (!path)
            return;
        await runQuiet({
            type: "parsePersonaSource",
            request_id: uuid(),
            label: sourceName.trim() || fileName(path),
            format: formatOf(path),
            path,
            text: null,
        }, "parse");
    };
    const generate = async () => {
        const isProfile = mode === "profile";
        if (isProfile && !profileName.trim()) {
            setNotice("请输入角色名称。");
            return;
        }
        if (!isProfile && (!source.id || !targetSpeaker)) {
            setNotice("请先解析聊天记录并选择目标说话人。");
            return;
        }
        setBusy("generate");
        if (beforeGenerate && !await beforeGenerate()) {
            setNotice("请先完成当前角色的保存，再生成新设定。");
            setNoticeTone("warning");
            setBusy(null);
            return;
        }
        await runQuiet({
            type: "generatePersonaProfile",
            request_id: uuid(),
            source_id: isProfile ? "" : source.id ?? "",
            kind: isProfile ? "character_description" : "chat_import",
            label: isProfile ? profileName.trim() : (source.label ?? sourceName) || "聊天记录",
            description: description.trim(),
            target_speaker: isProfile ? "" : targetSpeaker,
            target_speaker_label: isProfile ? "" : targetSpeaker,
            start_index: 0,
            end_index: isProfile ? 0 : source.messageCount ?? 0,
        }, "generate");
    };
    const apply = async () => {
        if (!draft)
            return;
        if (!draftName.trim()) {
            setNotice("草稿名称不能为空。");
            return;
        }
        if (!keepMemory && !await confirm({ title: "开始新的相处记录", message: "应用新角色后会清空当前宠物的长期记忆，无法恢复。聊天记录会保留，但不再作为新角色的近期上下文。", confirmLabel: "清空记忆并应用" }))
            return;
        const submitted = await runQuiet({
            type: "applyPersonaDraft",
            draft_id: draft.id,
            pet_id: snapshot.petId,
            name: draftName.trim(),
            style: EMPTY_STYLE,
            keep_memory: keepMemory,
            character_definition: definition,
        }, "apply");
        if (!submitted)
            return;
        const next = await loadSnapshot();
        if (!next.personaDraft.error && !next.personaDraft.draft && next.persona.characterDefinition === definition && next.persona.name === draftName.trim() && next.petId === snapshot.petId) {
            onApplied?.();
            setNoticeTone("success");
            setNotice(keepMemory ? "新角色设定已保存，记忆保持不变。" : "新角色已保存，已开始新的相处记录。");
        }
        else {
            setNoticeTone("danger");
            setNotice(next.personaDraft.error || "尚未完成应用，请稍候或重试；原设定仍可查看。");
        }
    };
    const previewDraft = async () => {
        if (!draft || !previewPrompt.trim())
            return;
        const request = uuid();
        setTested({ request, text: definition });
        await runQuiet({
            type: "previewPersonaDraft",
            request_id: request,
            pet_id: snapshot.petId,
            draft_id: draft.id,
            prompt: previewPrompt,
            character_definition: definition,
        }, "preview");
    };
    const clearDraft = async () => {
        await runQuiet({ type: "clearPersonaDraft" }, "apply");
        initializedDraft.current = "";
        setNotice("已放弃草稿，当前角色保持不变。");
        setNoticeTone("info");
    };
    const draftSource = draft?.source as Record<string, unknown> | undefined;
    const generating = draftState.generating || busy === "generate";
    const parsing = source.parsing || busy === "parse";
    return (<>
      {draft && <p className="draft-shortcut" role="status">有一份待应用的新角色。<Button variant="ghost" onClick={() => { draftRef.current?.focus(); draftRef.current?.scrollIntoView({ block: "start", behavior: "instant" }); }}>查看草稿</Button></p>}
      <details className="card detail-card">
        <summary>用资料塑造新角色</summary>
        <HelpText>提供人物介绍或聊天样本，生成可编辑的草稿。应用前当前角色保持不变。</HelpText>
        <div className="source-tabs">
          <button type="button" aria-pressed={mode === "profile"} disabled={generating} onClick={() => setMode("profile")}>
            <strong>人物描述</strong>
            <span>输入名字和风格说明</span>
          </button>
          <button type="button" aria-pressed={mode === "chat"} disabled={generating} onClick={() => setMode("chat")}>
            <strong>聊天记录</strong>
            <span>粘贴文字或选择聊天文件</span>
          </button>
        </div>

        {mode === "profile" ? (<>
            <SettingRow label="角色名称">
              <TextField value={profileName} onChange={setProfileName} placeholder="例如：沉静而幽默的旅人"/>
            </SettingRow>
            <SettingRow stacked label="补充说明">
              <TextArea value={description} onChange={setDescription} rows={3} placeholder="描述性格、表达方式、关系边界和你喜欢的回应习惯"/>
            </SettingRow>
          </>) : (<>
            <SettingRow label="样本名称">
              <TextField value={sourceName} onChange={setSourceName} placeholder="例如：和朋友的聊天记录"/>
            </SettingRow>
            <SettingRow stacked label="粘贴聊天记录">
              <TextArea value={pasteText} onChange={setPasteText} rows={6} placeholder="每行可以写成「说话人：消息内容」，也可以直接粘贴导出的聊天记录。"/>
            </SettingRow>
            <div className="source-actions">
              <Button variant="secondary" disabled={busy !== null} onClick={() => void parseFile()}>
                选择聊天记录文件
              </Button>
              <Button variant="secondary" disabled={parsing} onClick={() => void parseText()}>
                {parsing ? "正在读取…" : "读取粘贴内容"}
              </Button>
            </div>
            {source.id && (<div className="source-summary">
                <Badge tone="positive">已解析</Badge>
                <span>
                  {source.label || sourceName || "聊天记录"} · {source.messageCount ?? 0} 条消息
                </span>
                {!!source.speakers?.length && (<span>说话人：{source.speakers.map((speaker) => speaker.name).join("、")}</span>)}
              </div>)}
            {!!source.preview?.length && (<div className="source-preview">
                {source.preview.slice(0, 3).map((message, index) => (<p key={`${message.speaker}-${index}`}>
                    <strong>{message.speaker}</strong>
                    {message.text}
                  </p>))}
              </div>)}
            {source.id && !!source.speakers?.length && (<SettingRow label="目标说话人" hint="至少需要该说话人的 3 条消息。">
                <SelectField value={targetSpeaker} options={source.speakers.map((speaker) => ({
                    value: speaker.name,
                    label: `${speaker.name}（${speaker.count} 条）`,
                }))} onChange={setTargetSpeaker}/>
              </SettingRow>)}
          </>)}

        <div className="source-actions">
          <Button variant="primary" disabled={generating || parsing || busy !== null || !snapshot.hasPet || !snapshot.deepseek.keyConfigured} onClick={() => void generate()}>
            {generating ? "生成中…" : "生成角色设定"}
          </Button>
        </div>
        {source.error && <InlineNotice tone="danger">{source.error}</InlineNotice>}
        {draftState.error && <InlineNotice tone="danger">{draftState.error}</InlineNotice>}
        {notice && <InlineNotice tone={noticeTone}>{notice}</InlineNotice>}
        {!snapshot.hasPet && <HelpText>先在宠物页导入一只伙伴，再为它生成角色。</HelpText>}
        {!snapshot.deepseek.keyConfigured && <HelpText>生成角色需要模型服务，请先填写连接密钥并选择模型。</HelpText>}
        {generating && <HelpText>正在阅读资料并生成设定，你可以稍后再回来查看。</HelpText>}
      </details>

      {draft && (<section ref={draftRef} tabIndex={-1} className="draft-result" aria-label="新角色草稿">
        <Card title="待应用的新角色" description="这是待应用的草稿，当前角色仍在使用原设定。试聊使用你正在编辑的内容。可以保留相处记忆，也可以确认清空后开始新的关系。" tone="accent">
          <SettingRow label="草稿名称">
            <TextField value={draftName} onChange={setDraftName}/>
          </SettingRow>
          <TextArea label="新角色设定" value={definition} onChange={setDefinition} rows={7}/>
          <div className="draft-source">
            <span className="setting-label">来源</span>
            <span className="muted">{sourceLabel(draftSource ?? {}) || "人格资料"}</span>
          </div>
          <SettingRow stacked label="试聊">
            <div className="preview-control">
              <TextArea value={previewPrompt} onChange={setPreviewPrompt} rows={2}/>
              <Button variant="secondary" disabled={preview.inFlight || busy !== null || !definition.trim()} onClick={() => void previewDraft()}>
                {preview.inFlight ? "生成中…" : "试聊一句"}
              </Button>
            </div>
          </SettingRow>
          {tested?.text === definition && tested.request === preview.requestId && preview.error && <InlineNotice tone="danger">{preview.error}</InlineNotice>}
          {tested?.text === definition && tested.request === preview.requestId && preview.text && <div className="preview-answer">{preview.text}</div>}
          <SettingRow label="保留现有相处记忆" hint="关闭后，应用时会再次确认清空长期记忆；聊天记录仍可查看。"><Switch label="保留现有相处记忆" checked={keepMemory} onChange={setKeepMemory}/></SettingRow>
          <ActionFooter status={<span className="muted">确认风格后再应用</span>}>
            <Button variant="ghost" disabled={busy !== null} onClick={() => void clearDraft()}>
              放弃草稿
            </Button>
            <Button variant="primary" disabled={busy !== null || !definition.trim() || !draftName.trim()} onClick={() => void apply()}>
              {busy === "apply" ? "应用中…" : "替换当前设定"}
            </Button>
          </ActionFooter>
        </Card>
        </section>)}
      {confirmation}
    </>);
}
