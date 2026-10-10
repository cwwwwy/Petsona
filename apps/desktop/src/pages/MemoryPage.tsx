import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  Card,
  EmptyState,
  InlineNotice,
  PageHeader,
  SettingRow,
  Switch,
  TextField,
} from "../components/ui";
import { ActionMenu } from "../components/ActionMenu";
import { useConfirm } from "../components/ConfirmDialog";
import { openChatWindow, pickMemoryExport, pickMemoryImport } from "../lib/api";
import type { PageProps } from "../types";

const SOURCE_LABELS: Record<string, string> = {
  manual: "手动",
  conversation: "对话",
  import: "导入",
  compressed: "压缩",
};

export function MemoryPage({ snapshot, run }: PageProps) {
  const { confirm, confirmation } = useConfirm();
  const [expanded, setExpanded] = useState(false);
  const memory = snapshot.memory;
  const conversation = snapshot.settings.conversation;
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editKey, setEditKey] = useState("");
  const [editValue, setEditValue] = useState("");
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");
  const [busy, setBusy] = useState<"import" | "export" | null>(null);
  const displayFacts = [
    ...(memory.facts ?? []).map((fact) => ({ ...fact, archived: false })),
    ...(memory.archivedFacts ?? []).map((fact) => ({ ...fact, archived: true })),
  ];
  const pending = (memory.candidates ?? []).filter((candidate) => candidate.status === "pending");
  const visibleFacts = expanded ? displayFacts : displayFacts.slice(0, 8);
  useEffect(() => {
    setExpanded(false);
    setEditingId(null);
    setNewKey("");
    setNewValue("");
  }, [snapshot.petId]);
  const [reviewBusy, setReviewBusy] = useState<string | null>(null);

  const startEdit = (id: string, key: string, value: string) => {
    setEditingId(id);
    setEditKey(key);
    setEditValue(value);
  };

  const saveEdit = async () => {
    if (!editingId || !editKey.trim() || !editValue.trim()) return;
    const fact = displayFacts.find((item) => item.id === editingId);
    await run({
      type: "updateFact",
      fact: {
        id: editingId,
        key: editKey.trim(),
        value: editValue.trim(),
        confidence: fact?.confidence ?? 0.8,
      },
    });
    setEditingId(null);
  };

  const remember = async () => {
    if (!newKey.trim() || !newValue.trim()) return;
    await run({
      type: "rememberFact",
      fact: { key: newKey.trim(), value: newValue.trim(), confidence: 0.8 },
    });
    setNewKey("");
    setNewValue("");
  };

  const updateConfig = (patch: Partial<typeof memory.config>) => {
    void run({
      type: "updateMemoryConfig",
      config: { ...memory.config, ...patch },
    });
  };

  const reviewCandidate = async (candidateId: string, accept: boolean) => {
    setReviewBusy(candidateId);
    try {
      await run({
        type: "reviewMemoryCandidate",
        candidate_id: candidateId,
        accept,
      });
    } finally {
      setReviewBusy(null);
    }
  };

  const confirmClear = async (scope: 0 | 1 | 2, label: string) => {
    if (!await confirm({ title: label, message: `确定${label}吗？此操作不可撤销。`, confirmLabel: "清空" })) return;
    void run({ type: "clearMemory", scope });
  };

  const exportMemory = async () => {
    setBusy("export");
    try {
      const path = await pickMemoryExport(
        `memory-${snapshot.petId || snapshot.persona.id || "petsona"}.json`,
      );
      if (path) await run({ type: "exportMemory", path });
    } finally {
      setBusy(null);
    }
  };

  const importMemory = async () => {
    if (!await confirm({ title: "导入记忆", message: "导入会覆盖当前宠物的记忆文件，确定继续吗？", confirmLabel: "继续导入" })) return;
    setBusy("import");
    try {
      const path = await pickMemoryImport();
      if (path) await run({ type: "importMemory", path });
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="记忆"
        title="它记得什么"
        description="记忆只保存在本机。只有启用模型服务并发送对话时，必要上下文才会出网。"
        actions={
          <div className="button-group">
            <Button variant="ghost" onClick={() => void openChatWindow()}>
              查看聊天记录
            </Button>
            <ActionMenu disabled={busy !== null} actions={[
              { label: "导入记忆", onClick: () => void importMemory() },
              { label: "导出记忆", onClick: () => void exportMemory() },
            ]} />
          </div>
        }
      />

      <Card title="记忆与历史" description="记忆与聊天历史可以分别关闭；关闭记忆不再生成长期偏好，关闭历史不写入新对话。">
        <SettingRow label="启用记忆">
          <Switch
            label="启用记忆"
            checked={memory.config?.enabled ?? false}
            onChange={(value) => updateConfig({ enabled: value })}
          />
        </SettingRow>
        <SettingRow
          label="保存聊天历史"
          hint="关闭后新对话不会写入历史；已经保存的记录不会被自动删除。"
        >
          <Switch
            label="保存聊天历史"
            checked={conversation.saveHistory}
            onChange={(value) =>
              void run({
                type: "updateConversation",
                config: { ...conversation, saveHistory: value },
              })
            }
          />
        </SettingRow>
      </Card>

      {memory.learning && (
        <InlineNotice>
          正在整理近期习惯……候选会在达到证据门槛后出现在下方，等待你确认。
        </InlineNotice>
      )}

      <Card
        title="偏好事实"
        description={`${displayFacts.length} 条长期偏好${memory.archivedFacts?.length ? `（含 ${memory.archivedFacts.length} 条归档）` : ""}；显示来源，可编辑或删除。`}
      >
        {displayFacts.length ? (
          <div className="fact-list">
            {visibleFacts.map((fact) => {
              const editing = editingId === fact.id;
              return (
                <div className="fact-row" key={fact.id}>
                  {editing ? (
                    <div className="fact-edit">
                      <TextField
                        label="编辑记忆主题"
                        value={editKey}
                        onChange={setEditKey}
                        placeholder="称呼 / 偏好"
                      />
                      <TextField
                        label="编辑记忆内容"
                        value={editValue}
                        onChange={setEditValue}
                        placeholder="例如：喜欢喝美式咖啡"
                      />
                      <div className="button-group">
                        <Button variant="ghost" onClick={() => setEditingId(null)}>
                          取消
                        </Button>
                        <Button variant="primary" onClick={() => void saveEdit()}>
                          保存
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <>
                      <div className="fact-copy">
                        <div className="row-title">
                          <strong>{fact.key}</strong>
                          <Badge>{SOURCE_LABELS[fact.source] ?? fact.source}</Badge>
                          {fact.archived && <Badge>归档</Badge>}
                        </div>
                        <span>{fact.value}</span>
                      </div>
                      <div className="button-group">
                        <Button
                          variant="ghost"
                          onClick={() => startEdit(fact.id, fact.key, fact.value)}
                        >
                          编辑
                        </Button>
                        <Button
                          variant="ghost"
                          onClick={async () => {
                            if (await confirm({ title: "删除记忆", message: `确定删除「${fact.key}」这条记忆吗？`, confirmLabel: "删除" })) {
                              void run({ type: "forgetFact", id: fact.id });
                            }
                          }}
                        >
                          删除
                        </Button>
                      </div>
                    </>
                  )}
                </div>
              );
            })}
          </div>
        ) : (
          <EmptyState
            title="还没有长期偏好"
            description="在对话中自然提到稳定偏好，审阅通过后才会出现在这里。"
          />
        )}

        {displayFacts.length > 8 && (
          <Button variant="ghost" disabled={editingId !== null} onClick={() => setExpanded((value) => !value)}>
            {expanded ? "收起" : `展开其余 ${displayFacts.length - 8} 条`}
          </Button>
        )}
        <div className="fact-add fact-add-inline">
          <TextField label="新记忆主题" value={newKey} onChange={setNewKey} placeholder="主题，例如：称呼" />
          <TextField
            label="新记忆内容"
            value={newValue}
            onChange={setNewValue}
            placeholder="内容，例如：可以叫我小周"
          />
          <Button
            variant="primary"
            disabled={!newKey.trim() || !newValue.trim()}
            onClick={() => void remember()}
          >
            添加
          </Button>
        </div>
      </Card>

      {pending.length ? <Card
        title="待确认的习惯"
        description="模型从聊天中归纳的候选不会自动进入长期记忆，需要你确认。"
      >
        <div className="candidate-list">
          {pending.map((candidate) => (
            <div className="candidate-row" key={candidate.id}>
              <div className="candidate-copy">
                <div className="row-title">
                  <strong>{candidate.key}</strong>
                  <Badge tone="accent">
                    可信度 {Math.round(candidate.confidence * 100)}%
                  </Badge>
                </div>
                <span>{candidate.value}</span>
                {!!candidate.evidence.length && (
                  <small title={candidate.evidence.join("\n")}>
                    依据：{candidate.evidence.slice(0, 2).join("；")}
                  </small>
                )}
              </div>
              <div className="button-group">
                <Button
                  variant="primary"
                  disabled={reviewBusy !== null}
                  onClick={() => void reviewCandidate(candidate.id, true)}
                >
                  {reviewBusy === candidate.id ? "处理中…" : "确认"}
                </Button>
                <Button
                  variant="ghost"
                  disabled={reviewBusy !== null}
                  onClick={() => void reviewCandidate(candidate.id, false)}
                >
                  忽略
                </Button>
              </div>
            </div>
          ))}
        </div>
      </Card> : <p className="status-line">没有待确认的习惯；有新候选时会在这里提示。</p>}

      {snapshot.status && (
        <p className="status-line">
          <Badge tone={snapshot.status.includes("失败") ? "warning" : "neutral"}>状态</Badge>
          {snapshot.status}
        </p>
      )}

      <Card
        title="清空记忆"
        description="清空当前宠物的全部记忆。清空后无法恢复。"
        tone="danger"
      >
        <div className="danger-actions">
          <Button variant="danger" onClick={() => void confirmClear(0, "清空全部记忆")}>
            清空全部
          </Button>
        </div>
      </Card>

      <InlineNotice>
        记忆只保存在本机；只有配置并使用模型服务时，必要的记忆片段才会随对话发送给该服务。
        清空聊天记录不会删除已保存的长期偏好或互动事件。
      </InlineNotice>
      {confirmation}
    </div>
  );
}
