import { useEffect, useRef, useState } from "react";
import {
  Badge,
  Button,
  Card,
  InlineNotice,
  NumberField,
  PageHeader,
  SelectField,
  SettingRow,
  Switch,
  TextField,
} from "../components/ui";
import type { DeepSeekConfig, PageProps } from "../types";

const DEEPSEEK_BASE_URL = "https://api.deepseek.com/v1";
const PROVIDERS = [
  { value: "deepseek", label: "DeepSeek" },
  { value: "custom", label: "自定义（OpenAI 兼容）" },
] as const;

export function ConnectionPage({ snapshot, run }: PageProps) {
  const source = snapshot.deepseek;
  const dirty = useRef(false);
  const [config, setConfig] = useState<DeepSeekConfig>(source);
  const [apiKey, setApiKey] = useState("");
  const greeting = snapshot.settings.greeting;

  useEffect(() => {
    if (!dirty.current) setConfig(source);
  }, [source]);

  const patch = (value: Partial<DeepSeekConfig>) => {
    dirty.current = true;
    setConfig((current) => ({ ...current, ...value }));
  };

  const saveConnection = async () => {
    const { keyConfigured: _keyConfigured, ...payload } = config;
    await run({ type: "updateDeepSeek", config: payload });
    dirty.current = false;
  };

  const saveKey = async () => {
    if (!apiKey.trim()) return;
    await run({ type: "saveDeepSeekKey", key: apiKey.trim() });
    setApiKey("");
  };

  const clearKey = async () => {
    if (!window.confirm("确定清除当前模型服务商保存的 API Key 吗？")) return;
    await run({ type: "saveDeepSeekKey", key: "" });
    setApiKey("");
  };

  const updateGreeting = (value: Partial<typeof greeting>) => {
    void run({ type: "updateGreeting", config: { ...greeting, ...value } });
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="连接与问候"
        title="模型服务"
        description="服务商、URL、API Key、模型，再到高级参数。保存后即时生效。"
        actions={
          <Button variant="primary" onClick={() => void saveConnection()}>
            保存连接
          </Button>
        }
      />

      <Card title="服务">
        <SettingRow label="模型服务商" hint="DeepSeek 使用官方 OpenAI 兼容端点；自定义可接任意兼容服务。">
          <SelectField
            value={config.provider}
            options={PROVIDERS.map((provider) => ({
              value: provider.value as string,
              label: provider.label,
            }))}
            onChange={(provider) =>
              patch({
                provider,
                baseUrl: provider === "deepseek" ? DEEPSEEK_BASE_URL : config.baseUrl,
              })
            }
          />
        </SettingRow>
        <SettingRow label="Base URL" hint="通常以 /v1 结尾，不要填写 /chat/completions。">
          <TextField value={config.baseUrl} onChange={(baseUrl) => patch({ baseUrl })} />
        </SettingRow>
        <SettingRow
          label="API Key"
          hint={
            config.keyConfigured
              ? "当前凭据已保存在系统凭据库；输入新值会覆盖。"
              : "尚未配置。Key 不会写入 config.json。"
          }
        >
          <div className="key-control">
            <TextField
              value={apiKey}
              onChange={setApiKey}
              type="password"
              placeholder={config.keyConfigured ? "已配置（••••••••）" : "输入 API Key"}
            />
            <Button variant="secondary" onClick={() => void saveKey()} disabled={!apiKey.trim()}>
              保存 Key
            </Button>
            <Button variant="ghost" onClick={() => void clearKey()} disabled={!config.keyConfigured}>
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
              onClick={() => void run({ type: "listModels" })}
              disabled={!config.keyConfigured && config.provider === "deepseek"}
            >
              拉取模型
            </Button>
          </div>
        </SettingRow>
        {snapshot.status && (
          <p className="status-line">
            <Badge tone={snapshot.status.includes("失败") ? "warning" : "neutral"}>
              运行时
            </Badge>
            {snapshot.status}
          </p>
        )}
      </Card>

      <Card title="高级参数" description="通常保持默认即可。修改后使用“保存连接”写入。">
        <SettingRow label="请求超时">
          <NumberField
            value={config.timeoutSeconds}
            min={5}
            max={120}
            suffix="秒"
            onChange={(value) => patch({ timeoutSeconds: value })}
          />
        </SettingRow>
        <SettingRow label="短回复最大 token">
          <NumberField
            value={config.maxTokens}
            min={16}
            max={4096}
            step={16}
            onChange={(value) => patch({ maxTokens: value })}
          />
        </SettingRow>
        <SettingRow label="对话最大 token">
          <NumberField
            value={config.conversationMaxTokens}
            min={64}
            max={32768}
            step={64}
            onChange={(value) => patch({ conversationMaxTokens: value })}
          />
        </SettingRow>
        <SettingRow label="温度">
          <NumberField
            value={config.temperature}
            min={0}
            max={2}
            step={0.1}
            onChange={(value) => patch({ temperature: value })}
          />
        </SettingRow>
        <SettingRow label="短问候关闭思考" hint="减少简单问候的等待与 token 消耗。">
          <Switch
            label="短问候关闭思考"
            checked={config.thinkingDisabled}
            onChange={(value) => patch({ thinkingDisabled: value })}
          />
        </SettingRow>
      </Card>

      <Card title="空闲问候" description="无凭据时使用固定文案；配置模型后可生成更自然的问候。">
        <SettingRow label="启用空闲问候">
          <Switch
            label="启用空闲问候"
            checked={greeting.enabled}
            onChange={(enabled) => updateGreeting({ enabled })}
          />
        </SettingRow>
        <SettingRow label="空闲多久后问候">
          <NumberField
            value={greeting.idleMinutes}
            min={1}
            max={1440}
            suffix="分钟"
            onChange={(idleMinutes) => updateGreeting({ idleMinutes })}
          />
        </SettingRow>
        <SettingRow label="冷却时间">
          <NumberField
            value={greeting.cooldownMinutes}
            min={1}
            max={10080}
            suffix="分钟"
            onChange={(cooldownMinutes) => updateGreeting({ cooldownMinutes })}
          />
        </SettingRow>
        <SettingRow label="最大字数">
          <NumberField
            value={greeting.maxChars}
            min={8}
            max={200}
            suffix="字"
            onChange={(maxChars) => updateGreeting({ maxChars })}
          />
        </SettingRow>
      </Card>

      <InlineNotice>
        隐私说明：凭据保存在系统凭据库，配置文件中只保留 provider、URL、模型等非敏感字段。
        拉取模型和聊天会访问你填写的服务商。
      </InlineNotice>
    </div>
  );
}
