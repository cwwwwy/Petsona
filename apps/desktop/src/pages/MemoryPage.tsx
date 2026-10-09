import { useState } from "react";
import {
  Badge,
  Button,
  Card,
  EmptyState,
  NumberField,
  PageHeader,
  SettingRow,
  Switch,
  TextField,
} from "../components/ui";
import type { PageProps } from "../types";

const SOURCE_LABELS: Record<string, string> = {
  manual: "手动",
  conversation: "对话",
  import: "导入",
  compressed: "压缩",
};

export function MemoryPage({ snapshot, run }: PageProps) {
  const memory = snapshot.memory;
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editKey, setEditKey] = useState("");
  const [editValue, setEditValue] = useState("");
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");

  const startEdit = (id: string, key: string, value: string) => {
    setEditingId(id);
    setEditKey(key);
    setEditValue(value);
  };

  const saveEdit = async () => {
    if (!editingId || !editKey.trim() || !editValue.trim()) return;
    const fact = memory.facts.find((item) => item.id === editingId);
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

  const confirmClear = (scope: 0 | 1 | 2, label: string) => {
    if (!window.confirm(`确定${label}吗？此操作不可撤销。`)) return;
    void run({ type: "clearMemory", scope });
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="记忆"
        title="它记得什么"
        description="记忆只保存在本机。只有启用模型服务并发送对话时，必要上下文才会出网。"
      />

      <Card title="记忆开关" description="关闭后，新的对话不会再生成长期偏好。">
        <SettingRow label="启用记忆">
          <Switch
            label="启用记忆"
            checked={memory.config?.enabled ?? false}
            onChange={(value) => updateConfig({ enabled: value })}
          />
        </SettingRow>
      </Card>

      <Card
        title="偏好事实"
        description={`${memory.facts?.length ?? 0} 条长期偏好；显示来源，可编辑或删除。`}
      >
        {memory.facts?.length ? (
          <div className="fact-list">
            {memory.facts.map((fact) => {
              const editing = editingId === fact.id;
              return (
                <div className="fact-row" key={fact.id}>
                  {editing ? (
                    <div className="fact-edit">
                      <TextField
                        value={editKey}
                        onChange={setEditKey}
                        placeholder="称呼 / 偏好"
                      />
                      <TextField
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
                          variant="danger"
                          onClick={() => {
                            if (window.confirm(`确定删除「${fact.key}」这条记忆吗？`)) {
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
      </Card>

      <Card title="手动添加一条" description="适合补充称呼、口味、习惯等稳定事实。">
        <div className="fact-add">
          <TextField value={newKey} onChange={setNewKey} placeholder="主题，例如：称呼" />
          <TextField
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

      <Card title="记忆容量与保留" description="控制长期偏好的数量与互动事件的保留时间。">
        <SettingRow label="长期偏好上限">
          <NumberField
            value={memory.config?.factLimit ?? 20}
            min={1}
            max={100}
            onChange={(value) => updateConfig({ factLimit: value })}
          />
        </SettingRow>
        <SettingRow label="最近互动事件数">
          <NumberField
            value={memory.config?.recentEvents ?? 5}
            min={0}
            max={50}
            onChange={(value) => updateConfig({ recentEvents: value })}
          />
        </SettingRow>
        <SettingRow label="事件保留天数" hint="0 表示永久保留。">
          <NumberField
            value={memory.config?.eventRetentionDays ?? 0}
            min={0}
            max={3650}
            onChange={(value) => updateConfig({ eventRetentionDays: value })}
          />
        </SettingRow>
        <SettingRow label="自动压缩长期偏好" hint="超过上限时折叠为一份画像，不直接丢弃。">
          <Switch
            label="自动压缩长期偏好"
            checked={memory.config?.factCompress ?? true}
            onChange={(value) => updateConfig({ factCompress: value })}
          />
        </SettingRow>
      </Card>

      <Card
        title="清空记忆"
        description="按范围清空当前宠物的记忆。清空后无法恢复。"
        tone="danger"
      >
        <div className="danger-actions">
          <Button variant="danger" onClick={() => confirmClear(1, "清空所有长期偏好")}>
            清空偏好
          </Button>
          <Button variant="danger" onClick={() => confirmClear(2, "清空互动事件")}>
            清空事件
          </Button>
          <Button variant="danger" onClick={() => confirmClear(0, "清空全部记忆")}>
            清空全部
          </Button>
        </div>
      </Card>
    </div>
  );
}
