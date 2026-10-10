import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Badge, Button, EmptyState, InlineNotice } from "./components/ui";
import { useConfirm } from "./components/ConfirmDialog";
import { useSettings } from "./lib/useSettings";
import type { ConversationTurn, SettingsAction } from "./types";

function requestId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function isNearBottom(element: HTMLDivElement): boolean {
  return element.scrollHeight - element.scrollTop - element.clientHeight < 96;
}

export default function ChatApp() {
  const { confirm, confirmation } = useConfirm();
  const { snapshot, loading, error, apply } = useSettings(250);
  const [draft, setDraft] = useState("");
  const [composing, setComposing] = useState(false);
  const [localNotice, setLocalNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const messagesRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const stickToBottom = useRef(true);

  const run = async (action: SettingsAction) => {
    try {
      await apply(action);
    } catch (reason) {
      setLocalNotice(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const conversation = snapshot?.conversation ?? null;
  const turns = conversation?.turns ?? [];
  const latestText = turns.length ? turns[turns.length - 1]?.text ?? "" : "";

  useEffect(() => {
    const element = messagesRef.current;
    if (element && stickToBottom.current) {
      element.scrollTop = element.scrollHeight;
    }
  }, [turns.length, latestText]);

  useEffect(() => {
    window.setTimeout(() => composerRef.current?.focus(), 80);
  }, []);

  if (loading && !snapshot) {
    return (
      <main className="chat-boot">
        <h1>正在打开聊天</h1>
        <p>读取宠物与历史记录…</p>
      </main>
    );
  }

  if (!snapshot) {
    return (
      <main className="chat-boot">
        <h1>无法打开聊天</h1>
        <p>{error || "运行时没有返回会话信息。"}</p>
      </main>
    );
  }

  if (!snapshot.ready && !snapshot.faulted) {
    return (
      <main className="chat-boot">
        <h1>正在连接 Petsona</h1>
        <p>等待运行时准备完成…</p>
      </main>
    );
  }

  const send = async (retry?: ConversationTurn) => {
    const text = retry?.text ?? draft;
    if (!text.trim() || conversation?.inFlight || busy) return;
    setLocalNotice("");
    setBusy(true);
    try {
      const action: SettingsAction = {
        type: "startConversation",
        request_id: requestId(),
        pet_id: snapshot.petId,
        text,
        retry_turn_id: retry?.id ?? null,
      };
      await run(action);
      if (!retry) setDraft("");
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    if (!conversation?.inFlight) return;
    setBusy(true);
    try {
      await run({
        type: "cancelConversation",
        request_id: conversation.requestId ?? "",
      });
    } finally {
      setBusy(false);
    }
  };

  const clearHistory = async () => {
    if (!await confirm({ title: "清空聊天记录", message: "确定清空当前宠物的聊天记录吗？此操作不可撤销，长期记忆会保留。", confirmLabel: "清空" })) return;
    setBusy(true);
    try {
      await run({ type: "clearConversationHistory", pet_id: snapshot.petId });
    } finally {
      setBusy(false);
    }
  };

  const loadEarlier = async () => {
    setBusy(true);
    try {
      await run({
        type: "loadEarlierConversationHistory",
        pet_id: snapshot.petId,
      });
    } finally {
      setBusy(false);
    }
  };

  const onComposerKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (
      event.key === "Enter" &&
      !event.shiftKey &&
      !composing &&
      !event.nativeEvent.isComposing
    ) {
      event.preventDefault();
      void send();
    }
  };

  const retryTarget = (turn: ConversationTurn): ConversationTurn | null => {
    const index = turns.findIndex((item) => item.id === turn.id);
    for (let cursor = index - 1; cursor >= 0; cursor -= 1) {
      if (turns[cursor].user) return turns[cursor];
    }
    return null;
  };

  return (
    <div className="chat-shell">
      <header className="chat-header">
        <div className="chat-heading">
          <div className="brand-mark">P</div>
          <div>
            <strong>{snapshot.petName || "Petsona"}</strong>
            <span>
              {snapshot.persona?.name || "默认说话方式"}
              {conversation?.saveHistory ? " · 已保存历史" : " · 本次会话"}
            </span>
          </div>
        </div>
        <div className="chat-header-actions">
          {conversation?.inFlight && <Badge tone="warning">生成中</Badge>}
          <Button variant="ghost" disabled={busy || !snapshot.petId} onClick={() => void clearHistory()}>
            清空
          </Button>
        </div>
      </header>

      <div
        className="chat-messages"
        ref={messagesRef}
        onScroll={() => {
          const element = messagesRef.current;
          if (element) stickToBottom.current = isNearBottom(element);
        }}
      >
        {conversation?.hasEarlier && (
          <div className="load-earlier">
            <Button variant="ghost" disabled={busy} onClick={() => void loadEarlier()}>
              加载更早的记录
            </Button>
          </div>
        )}

        {turns.length === 0 ? (
          <EmptyState
            title="还没有聊天记录"
            description="在下面输入一句话，开始和宠物聊天。"
          />
        ) : (
          turns.map((turn) => (
            <article
              className={`chat-turn ${turn.user ? "chat-turn-user" : "chat-turn-pet"}`}
              key={turn.id}
            >
              <div className="chat-turn-meta">
                <span>{turn.user ? "你" : snapshot.petName || "宠物"}</span>
                {turn.status === "streaming" && <Badge tone="warning">输入中</Badge>}
                {turn.status === "cancelled" && <Badge>已停止</Badge>}
                {turn.status === "failed" && <Badge tone="warning">失败</Badge>}
              </div>
              <div className="chat-bubble">
                {turn.text || (turn.status === "streaming" ? "正在思考…" : "没有内容")}
              </div>
              {!turn.user && (turn.status === "failed" || turn.status === "cancelled") && (
                <div className="chat-turn-actions">
                  <Button
                    variant="ghost"
                    disabled={busy || conversation?.inFlight}
                    onClick={() => {
                      const target = retryTarget(turn);
                      if (target) void send(target);
                    }}
                  >
                    重试
                  </Button>
                </div>
              )}
            </article>
          ))
        )}

        {conversation?.error && <InlineNotice tone="danger">{conversation.error}</InlineNotice>}
        {snapshot.faulted && snapshot.error && (
          <InlineNotice tone="danger">{snapshot.error}</InlineNotice>
        )}
        {localNotice && <InlineNotice tone="warning">{localNotice}</InlineNotice>}
      </div>

      <footer className="chat-composer">
        <textarea
          ref={composerRef}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={onComposerKeyDown}
          onCompositionStart={() => setComposing(true)}
          onCompositionEnd={() => setComposing(false)}
          placeholder="和宠物说点什么…（Enter 发送，Shift+Enter 换行）"
          rows={3}
          disabled={conversation?.inFlight || busy}
        />
        <div className="chat-composer-actions">
          <span className="muted">Enter 发送 · Shift+Enter 换行</span>
          {conversation?.inFlight ? (
            <Button variant="danger" disabled={busy} onClick={() => void stop()}>
              停止
            </Button>
          ) : (
            <Button
              variant="primary"
              disabled={!draft.trim() || busy || !snapshot.petId}
              onClick={() => void send()}
            >
              发送
            </Button>
          )}
        </div>
      </footer>
      {confirmation}
    </div>
  );
}
