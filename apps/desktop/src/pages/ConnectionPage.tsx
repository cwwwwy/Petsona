import { useCallback, useEffect, useRef, useState } from "react";
import {
  Badge,
  Button,
  Card,
  InlineNotice,
  PageHeader,
  SelectField,
  SettingRow,
  TextField,
} from "../components/ui";
import { useConfirm } from "../components/ConfirmDialog";
import { loadSnapshot } from "../lib/api";
import type { DeepSeekConfig, PageProps } from "../types";

const DEEPSEEK_BASE_URL = "https://api.deepseek.com/v1";
const PROVIDERS = [
  { value: "deepseek", label: "DeepSeek" },
  { value: "custom", label: "自定义（OpenAI 兼容）" },
] as const;

type NoticeTone = "info" | "warning" | "danger";
type Notice = { tone: NoticeTone; message: string } | null;
type ModelState =
  | { kind: "idle"; message: string }
  | { kind: "loading"; message: string }
  | { kind: "success"; message: string }
  | { kind: "warning"; message: string }
  | { kind: "error"; message: string };

function validHttpUrl(value: string): boolean {
  return /^https?:\/\/\S+/i.test(value.trim());
}

export function ConnectionPage({ snapshot, run }: PageProps) {
  const { confirm, confirmation } = useConfirm();
  const source = snapshot.deepseek;
  const dirty = useRef(false);
  const [config, setConfig] = useState<DeepSeekConfig>(source);
  const [apiKey, setApiKey] = useState("");
  const [savingConnection, setSavingConnection] = useState(false);
  const [savingKey, setSavingKey] = useState(false);
  const [connectionNotice, setConnectionNotice] = useState<Notice>(null);
  const [keyNotice, setKeyNotice] = useState<Notice>(null);
  const [modelState, setModelState] = useState<ModelState>({
    kind: "idle",
    message: snapshot.models.length
      ? `已有 ${snapshot.models.length} 个模型可选`
      : "可以先手动填写模型名",
  });

  useEffect(() => {
    if (!dirty.current) setConfig(source);
  }, [source]);

  useEffect(() => {
    if (modelState.kind !== "loading") return;
    const status = snapshot.status;
    if (status.includes("拉取模型列表失败")) {
      setModelState({ kind: "error", message: status });
    } else if (status.includes("没有返回任何模型")) {
      setModelState({ kind: "warning", message: `${status}；请手动填写` });
    } else if (status.includes("已拉取")) {
      setModelState({
        kind: "success",
        message: `${status}，可从列表选择或继续手动输入`,
      });
    }
  }, [snapshot.status, modelState.kind]);

  const patch = (value: Partial<DeepSeekConfig>) => {
    dirty.current = true;
    setConfig((current) => ({ ...current, ...value }));
    setConnectionNotice(null);
  };

  const persistConnection = useCallback(async () => {
    if (!validHttpUrl(config.baseUrl)) {
      setConnectionNotice({
        tone: "danger",
        message: "Base URL 必须是完整的 http/https 地址。",
      });
      return;
    }
    setSavingConnection(true);
    setConnectionNotice(null);
    try {
      const { keyConfigured: _keyConfigured, ...payload } = config;
      await run({ type: "updateDeepSeek", config: payload });
      dirty.current = false;
      const next = await loadSnapshot();
      setConnectionNotice(
        next.status.includes("失败")
          ? { tone: "danger", message: next.status }
          : { tone: "info", message: next.status || "连接设置已自动保存" },
      );
    } finally {
      setSavingConnection(false);
    }
  }, [config, run]);

  useEffect(() => {
    if (!dirty.current) return;
    const timer = window.setTimeout(() => {
      void persistConnection();
    }, 450);
    return () => window.clearTimeout(timer);
  }, [persistConnection]);

  const saveKey = async () => {
    if (!apiKey.trim()) return;
    setSavingKey(true);
    setKeyNotice(null);
    try {
      await run({ type: "saveDeepSeekKey", key: apiKey.trim() });
      const next = await loadSnapshot();
      setApiKey("");
      setKeyNotice(
        next.status.includes("失败")
          ? { tone: "danger", message: next.status }
          : {
              tone: "info",
              message: next.deepseek.keyConfigured
                ? "密钥已保存，并检测到当前服务商凭据可用。"
                : "密钥已保存；凭据探测可能需要一点时间。",
            },
      );
    } finally {
      setSavingKey(false);
    }
  };

  const clearKey = async () => {
    if (!await confirm({ title: "清除密钥", message: "确定清除当前模型服务商保存的 API Key 吗？", confirmLabel: "清除" })) return;
    setSavingKey(true);
    setKeyNotice(null);
    try {
      await run({ type: "saveDeepSeekKey", key: "" });
      const next = await loadSnapshot();
      setApiKey("");
      setKeyNotice({
        tone: next.deepseek.keyConfigured ? "warning" : "info",
        message: next.deepseek.keyConfigured
          ? next.status || "系统凭据已清除，但环境变量仍提供了一个密钥。"
          : "当前服务商密钥已清除。",
      });
    } finally {
      setSavingKey(false);
    }
  };

  const pullModels = async () => {
    setModelState({ kind: "loading", message: "正在从服务商拉取模型列表…" });
    await run({ type: "listModels" });
  };

  const baseUrlValid = validHttpUrl(config.baseUrl);

  return (
    <div className="page">
      <PageHeader
        eyebrow="连接"
        title="模型服务"
        description="服务商、URL、模型修改后自动保存；API Key 单独保存。"
        actions={
          <Badge tone={savingConnection ? "warning" : "positive"}>
            {savingConnection ? "自动保存中…" : "自动保存"}
          </Badge>
        }
      />

      <Card
        title="服务"
        description="选择用于聊天和问候的模型服务。"
      >
        <SettingRow label="模型服务商" hint="DeepSeek 使用官方 OpenAI 兼容端点；自定义可接任意兼容服务。">
          <div className="connection-control">
            <SelectField
              value={config.provider}
              options={PROVIDERS.map((provider) => ({
                value: provider.value as string,
                label: provider.label,
              }))}
              onChange={(provider) => {
                patch({
                  provider,
                  baseUrl: provider === "deepseek" ? DEEPSEEK_BASE_URL : config.baseUrl,
                });
                setModelState({
                  kind: "idle",
                  message: "切换服务商后保存设置，再拉取模型列表。",
                });
              }}
            />
            {config.keyConfigured ? (
              <Badge tone="positive">Key 已配置</Badge>
            ) : (
              <Badge tone="warning">Key 未配置</Badge>
            )}
          </div>
        </SettingRow>
        <SettingRow
          label="Base URL"
          hint="通常以 /v1 结尾，不要填写 /chat/completions。"
        >
          <div className="connection-control">
            <TextField value={config.baseUrl} onChange={(baseUrl) => patch({ baseUrl })} />
            {!baseUrlValid && <Badge tone="warning">格式无效</Badge>}
          </div>
        </SettingRow>
        <SettingRow
          label="API Key"
          hint={
            config.keyConfigured
              ? "当前凭据已保存在系统凭据库；输入新值会覆盖。"
              : "尚未配置。密钥不会写入设置文件。"
          }
        >
          <div className="key-control">
            <TextField
              value={apiKey}
              onChange={setApiKey}
              type="password"
              placeholder={config.keyConfigured ? "已配置（••••••••）" : "输入 API Key"}
            />
            <Button
              variant="secondary"
              onClick={() => void saveKey()}
              disabled={!apiKey.trim() || savingKey}
            >
              {savingKey ? "处理中…" : "保存 Key"}
            </Button>
            <Button
              variant="ghost"
              onClick={() => void clearKey()}
              disabled={!config.keyConfigured || savingKey}
            >
              清除
            </Button>
          </div>
        </SettingRow>
        <SettingRow
          label="模型"
          hint={
            snapshot.models.length
              ? `已拉取 ${snapshot.models.length} 个模型，可从列表选择或手动输入。`
              : "可以先手动填写；配置 Key 后可从服务商拉取。"
          }
        >
          <div className="model-control">
            <input
              className="control-input"
              aria-label="模型"
              list="petsona-model-options"
              value={config.model}
              onChange={(event) => patch({ model: event.target.value })}
              placeholder="例如：deepseek-v4-flash"
            />
            <datalist id="petsona-model-options">
              {snapshot.models.map((model) => (
                <option key={model} value={model} />
              ))}
            </datalist>
            <Button
              variant="secondary"
              onClick={() => void pullModels()}
              disabled={modelState.kind === "loading"}
            >
              {modelState.kind === "loading" ? "拉取中…" : "拉取模型"}
            </Button>
          </div>
        </SettingRow>

        {modelState.kind !== "idle" && (
          <p
            className={`status-line model-status model-status-${modelState.kind}`}
            role={modelState.kind === "error" ? "alert" : undefined}
          >
            <Badge
              tone={
                modelState.kind === "error"
                  ? "warning"
                  : modelState.kind === "success"
                    ? "positive"
                    : "neutral"
              }
            >
              模型列表
            </Badge>
            {modelState.message}
          </p>
        )}
        {connectionNotice && (
          <InlineNotice tone={connectionNotice.tone}>
            {connectionNotice.message}
          </InlineNotice>
        )}
        {keyNotice && <InlineNotice tone={keyNotice.tone}>{keyNotice.message}</InlineNotice>}
      </Card>

      <InlineNotice>
        密钥保存在系统凭据库，不会写入设置文件。
        拉取模型和聊天会访问你填写的服务商。
      </InlineNotice>
      {confirmation}
    </div>
  );
}
