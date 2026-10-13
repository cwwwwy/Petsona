import { useEffect, useRef, useState } from "react";
import { MemoryEditor, ActionFooter, Button, Card, HelpText, InlineNotice, PageHeader, SelectField, SettingRow, Switch, TextArea } from "../components/ui";
import { ActionMenu } from "../components/ActionMenu";
import { useConfirm } from "../components/ConfirmDialog";
import { openChatWindow, pickMemoryExport, pickMemoryImport } from "../lib/api";
import type { MemoryFact, PageProps } from "../types";
const kinds = [{ value: "关于你", label: "关于你" }, { value: "共同经历", label: "共同经历" }, { value: "约定", label: "你们的约定" }];
const sentence = (fact: {
    key: string;
    value: string;
}) => kinds.some(kind => kind.value === fact.key) ? fact.value : `${fact.key}：${fact.value}`;
const kindOf = (key: string) => ["共同经历", "约定"].includes(key) ? key : "关于你";
const sources: Record<string, string> = { manual: "你亲自填写", conversation: "来自聊天", import: "从备份导入", compressed: "来自早期记录" };
export function MemoryPage({ snapshot, run }: PageProps) {
    const { confirm, confirmation } = useConfirm();
    const memory = snapshot.memory;
    const facts = [...(memory.facts ?? []), ...(memory.archivedFacts ?? [])].filter(fact => fact.source !== "compressed");
    const pending = (memory.candidates ?? []).filter(candidate => candidate.status === "pending");
    const [expanded, setExpanded] = useState(false), [adding, setAdding] = useState(!facts.length);
    const [editing, setEditing] = useState<string | null>(null), [candidateEdit, setCandidateEdit] = useState<string | null>(null);
    const [editValue, setEditValue] = useState(""), [editKind, setEditKind] = useState("关于你");
    const [newValue, setNewValue] = useState(""), [newKind, setNewKind] = useState("关于你");
    const [busy, setBusy] = useState<string | null>(null);
    const [notice, setNotice] = useState<{
        tone: "info" | "success" | "danger";
        message: string;
    } | null>(null);
    const editor = useRef<HTMLTextAreaElement>(null), candidateEditor = useRef<HTMLTextAreaElement>(null), newEditor = useRef<HTMLTextAreaElement>(null);
    const focusRequest = useRef(false), returnId = useRef<string | null>(null), restore = useRef(false);
    const livePet = useRef(snapshot.petId);
    livePet.current = snapshot.petId;
    useEffect(() => { setExpanded(false); setAdding(![...(memory.facts ?? []), ...(memory.archivedFacts ?? [])].length); setEditing(null); setCandidateEdit(null); setNewValue(""); setNewKind("关于你"); setNotice(null); focusRequest.current = false; restore.current = false; }, [snapshot.petId]);
    useEffect(() => {
        if (focusRequest.current) {
            const target = editing ? editor.current : candidateEdit ? candidateEditor.current : newEditor.current;
            if (target?.getClientRects().length) {
                target.focus();
                target.scrollIntoView({ block: "nearest", behavior: "instant" });
            }
            focusRequest.current = false;
        }
        if (restore.current) {
            [...document.querySelectorAll<HTMLButtonElement>("[data-memory-edit]")].find(node => node.dataset.memoryEdit === returnId.current)?.focus();
            restore.current = false;
        }
    }, [editing, candidateEdit, adding]);
    const finishEdit = () => { restore.current = true; setEditing(null); setCandidateEdit(null); };
    const beginEdit = (id: string, key: string, value: string, candidate = false) => { returnId.current = id; focusRequest.current = true; setEditing(candidate ? null : id); setCandidateEdit(candidate ? id : null); setEditKind(kindOf(key)); setEditValue(sentence({ key, value })); setNotice(null); };
    const saveEdit = async (fact: MemoryFact) => {
        if (!editValue.trim())
            return;
        const pet = snapshot.petId;
        setBusy(fact.id);
        try {
            const next = await run({ type: "updateFact", fact: { id: fact.id, key: editKind, value: editValue.trim(), confidence: fact.confidence } });
            if (livePet.current !== pet)
                return;
            if (!next || ![...next.memory.facts, ...next.memory.archivedFacts].some(item => item.id === fact.id && item.value === editValue.trim())) {
                setNotice({ tone: "danger", message: "修改尚未确认，内容已保留，请重试。" });
                return;
            }
            setNotice({ tone: "success", message: "记忆已修改。" });
            finishEdit();
        }
        finally {
            setBusy(null);
        }
    };
    const remember = async () => {
        if (!newValue.trim())
            return;
        const pet = snapshot.petId;
        setBusy("add");
        try {
            const next = await run({ type: "rememberFact", fact: { key: newKind, value: newValue.trim(), confidence: 1 } });
            if (livePet.current !== pet)
                return;
            if (!next || ![...next.memory.facts, ...next.memory.archivedFacts].some(fact => fact.key === newKind && fact.value === newValue.trim())) {
                setNotice({ tone: "danger", message: "尚未确认保存，输入已保留，请重试。" });
                return;
            }
            setNewValue("");
            restore.current = true;
            returnId.current = "__add__";
            setAdding(false);
            setNotice({ tone: "success", message: "已记住这件事。" });
        }
        finally {
            setBusy(null);
        }
    };
    const review = async (id: string, accept: boolean) => {
        setBusy(id);
        try {
            const next = await run({ type: "reviewMemoryCandidate", candidate_id: id, accept });
            if (!next || next.status.includes("失败") || next.memory.candidates.some(item => item.id === id && item.status === "pending")) {
                setNotice({ tone: "danger", message: "尚未完成处理，请重试。" });
                return;
            }
            setNotice({ tone: accept ? "success" : "info", message: accept ? "已记住这条内容。" : "不会记住这条内容。" });
        }
        finally {
            setBusy(null);
        }
    };
    const saveCandidate = async (id: string, originalKey: string) => {
        if (!editValue.trim())
            return;
        const pet = snapshot.petId;
        setBusy(id);
        try {
            const oldAddress = originalKey === "称呼" && editKind === "关于你" ? facts.find(fact => fact.key === "称呼") : undefined;
            const next = oldAddress ? await run({ type: "updateFact", fact: { id: oldAddress.id, key: editKind, value: editValue.trim(), confidence: 1 } }) : await run({ type: "rememberFact", fact: { key: editKind, value: editValue.trim(), confidence: 1 } });
            if (livePet.current !== pet)
                return;
            if (!next || ![...next.memory.facts, ...next.memory.archivedFacts].some(fact => fact.key === editKind && fact.value === editValue.trim())) {
                setNotice({ tone: "danger", message: "尚未确认保存，编辑内容已保留。" });
                return;
            }
            const reviewed = await run({ type: "reviewMemoryCandidate", candidate_id: id, accept: false });
            if (!reviewed || reviewed.memory.candidates.some(item => item.id === id && item.status === "pending")) {
                setNotice({ tone: "danger", message: "修改已保存，但原候选尚未处理，请重试。" });
                return;
            }
            setNotice({ tone: "success", message: "已记住修改后的内容。" });
            finishEdit();
        }
        finally {
            setBusy(null);
        }
    };
    const importMemory = async () => {
        const path = await pickMemoryImport();
        if (!path)
            return;
        const name = path.split(/[\\/]/).pop();
        if (!await confirm({ title: "用备份替换记忆", message: `导入“${name}”会替换当前伙伴的相处记忆。现有聊天记录不会删除，确定继续吗？`, confirmLabel: "替换记忆" }))
            return;
        setBusy("import");
        try {
            const next = await run({ type: "importMemory", path });
            setNotice(!next || next.status.includes("失败") ? { tone: "danger", message: "记忆导入未完成，请检查备份文件后重试。" } : { tone: "success", message: "备份记忆已导入。" });
        }
        finally {
            setBusy(null);
        }
    };
    const exportMemory = async () => { const path = await pickMemoryExport(`memory-${snapshot.petId}.json`); if (!path)
        return; setBusy("export"); try {
        const next = await run({ type: "exportMemory", path });
        setNotice(!next || next.status.includes("失败") ? { tone: "danger", message: "未能完成导出，请重试。" } : { tone: "success", message: "记忆备份已导出。" });
    }
    finally {
        setBusy(null);
    } };
    const hasStored = !!(facts.length || memory.candidates.length || memory.events.length || memory.archivedFacts.length);
    const clear = async () => { if (!await confirm({ title: "清空相处记忆", message: "将清空当前伙伴的长期记忆、待确认内容和近期互动，无法恢复。聊天记录仍会保留。", confirmLabel: "清空记忆" }))
        return; const next = await run({ type: "clearMemory", scope: 0 }); setNotice(!next || next.status.includes("失败") ? { tone: "danger", message: "尚未确认清空，请重试。" } : { tone: "info", message: "相处记忆已清空。" }); };
    return <div className="page">
    <PageHeader title="它记得什么" description="留住关于你和你们相处的事情。" actions={<div className="button-group"><button type="button" className="button button-secondary" data-memory-edit="__add__" onClick={() => { returnId.current = "__add__"; if (adding) {
        newEditor.current?.focus();
        newEditor.current?.scrollIntoView({ block: "nearest", behavior: "instant" });
    }
    else {
        focusRequest.current = true;
        setAdding(true);
    } }}>补充记忆</button><ActionMenu disabled={busy !== null} actions={[{ label: "导入备份", onClick: () => void importMemory() }, { label: "导出备份", onClick: () => void exportMemory() }, { label: "查看聊天记录", onClick: () => void openChatWindow() }]}/></div>}/>
    {notice && <InlineNotice tone={notice.tone}>{notice.message}</InlineNotice>}
    <Card title="记忆设置"><SettingRow label="启用记忆" hint="关闭后，不再自动学习或在聊天中使用长期记忆。"><Switch label="启用记忆" checked={memory.config.enabled} onChange={enabled => void run({ type: "updateMemoryConfig", config: { ...memory.config, enabled } })}/></SettingRow><SettingRow label="保存聊天记录" hint="关闭只影响新对话，已有记录仍会保留。"><Switch label="保存聊天记录" checked={snapshot.settings.conversation.saveHistory} onChange={saveHistory => void run({ type: "updateConversation", config: { ...snapshot.settings.conversation, saveHistory } })}/></SettingRow></Card>
    {memory.learning && <InlineNotice>正在整理近期聊天，有值得留下的内容时会请你确认。</InlineNotice>}
    {!facts.length && <p className="compact-empty">还没有长期记忆。可以先聊聊，也可以亲自补充一件事。</p>}
    {facts.length > 0 && [{ title: "关于你", about: true }, { title: "共同经历与约定", about: false }].map(group => {
            const items = facts.filter(fact => !["共同经历", "约定"].includes(fact.key) === group.about);
            if (!items.length)
                return null;
            return <Card key={group.title} title={group.title}><div className="fact-list">{(expanded ? items : items.slice(0, 8)).map(fact => <div className="fact-row" key={fact.id}>{editing === fact.id ? <MemoryEditor inputRef={editor} value={editValue} onChange={setEditValue} kind={editKind} onKindChange={setEditKind} busy={busy === fact.id} onCancel={finishEdit} onSave={() => void saveEdit(fact)}/> : <><div className="fact-copy"><span>{sentence(fact)}</span><details><summary>查看来源</summary><small>{sources[fact.source] || "保存的记录"} · {new Date(fact.updatedAt).toLocaleDateString()}</small></details></div><div className="button-group"><button type="button" className="button button-ghost" data-memory-edit={fact.id} onClick={() => beginEdit(fact.id, fact.key, fact.value)}>修改</button><Button variant="ghost" disabled={busy !== null} onClick={async () => { if (await confirm({ title: "忘记这条记忆", message: `确定忘记“${fact.value}”吗？聊天记录仍会保留。`, confirmLabel: "忘记" })) {
                const next = await run({ type: "forgetFact", id: fact.id });
                setNotice(!next || next.status.includes("失败") ? { tone: "danger", message: "尚未确认已忘记，请重试。" } : { tone: "info", message: "已忘记这条长期记忆。" });
            } }}>忘记</Button></div></>}</div>)}</div></Card>;
        })}
    {facts.length > 8 && <Button variant="ghost" disabled={!!editing} onClick={() => setExpanded(!expanded)}>{expanded ? "收起" : `查看全部 ${facts.length} 条记忆`}</Button>}
    {adding && <Card title="补充一条记忆"><SelectField label="记忆类型" value={newKind} onChange={setNewKind} options={kinds}/><TextArea inputRef={newEditor} label="希望它记住什么" value={newValue} onChange={setNewValue} rows={3} placeholder="例如：我们约好了，周末继续讨论我的故事。"/><ActionFooter status={<HelpText>你亲自填写的内容会直接保存。</HelpText>}><Button variant="ghost" disabled={busy === "add"} onClick={() => { restore.current = true; returnId.current = "__add__"; setAdding(false); }}>取消</Button><Button variant="primary" disabled={busy === "add" || !newValue.trim()} onClick={() => void remember()}>{busy === "add" ? "正在保存…" : "记住这件事"}</Button></ActionFooter></Card>}
    {pending.length > 0 && <Card title="需要你确认" description="从聊天中整理的内容，确认后才会长期记住。"><div className="candidate-list">{pending.map(candidate => <div className="candidate-row" key={candidate.id}>{candidateEdit === candidate.id ? <MemoryEditor inputRef={candidateEditor} value={editValue} onChange={setEditValue} kind={editKind} onKindChange={setEditKind} busy={busy === candidate.id} onCancel={finishEdit} onSave={() => void saveCandidate(candidate.id, candidate.key)} saveLabel="保存并记住"/> : <><div className="candidate-copy"><span>{sentence(candidate)}</span><details><summary>为什么记住这件事</summary>{candidate.evidence.map((text, index) => <p key={index}>{text.replace(/^[0-9a-f-]{36}：/i, "")}</p>)}</details></div><div className="button-group"><Button variant="primary" disabled={busy !== null} onClick={() => void review(candidate.id, true)}>记住</Button><button type="button" className="button button-ghost" data-memory-edit={candidate.id} disabled={busy !== null} onClick={() => beginEdit(candidate.id, candidate.key, candidate.value, true)}>修改</button><Button variant="ghost" disabled={busy !== null} onClick={() => void review(candidate.id, false)}>不记住</Button></div></>}</div>)}</div></Card>}
    <details className="memory-management"><summary>隐私与记忆管理</summary><HelpText>记忆保存在本机。使用模型聊天时，相关记忆会发送给你选择的服务。清空聊天记录不会删除长期记忆。</HelpText><ActionFooter><Button variant="danger" disabled={!hasStored || busy !== null} onClick={() => void clear()}>清空相处记忆</Button></ActionFooter></details>
    {confirmation}
  </div>;
}
