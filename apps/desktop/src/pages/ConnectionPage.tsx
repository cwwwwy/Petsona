import { useCallback, useEffect, useRef, useState } from "react";
import { ActionFooter, Badge, Button, Card, HelpText, InlineNotice, PageHeader, SaveStatus, SelectField, SettingRow, TextField, type SavePhase } from "../components/ui";
import { useConfirm } from "../components/ConfirmDialog";
import type { DeepSeekConfig, PageProps } from "../types";
const DEFAULT_URL = "https://api.deepseek.com/v1";
const providers = [{ value: "deepseek", label: "DeepSeek" }, { value: "custom", label: "其他服务" }];
function validUrl(value: string) { try {
    const url = new URL(value.trim());
    return ["http:", "https:"].includes(url.protocol) && !!url.hostname;
}
catch {
    return false;
} }
const normalUrl = (value: string) => value.trim().replace(/\/+$/, "");
type Notice = {
    tone: "info" | "success" | "warning" | "danger";
    message: string;
} | null;
export function ConnectionPage({ snapshot, run }: PageProps) {
    const { confirm, confirmation } = useConfirm();
    const [config, setConfig] = useState(snapshot.deepseek);
    const latest = useRef(config);
    latest.current = config;
    const dirty = useRef(false), saving = useRef(false), version = useRef(0);
    const [phase, setPhase] = useState<SavePhase>("saved");
    const [saveMessage, setSaveMessage] = useState("");
    const [apiKey, setApiKey] = useState("");
    const [savingKey, setSavingKey] = useState(false);
    const [keyNotice, setKeyNotice] = useState<Notice>(null);
    const [modelNotice, setModelNotice] = useState<Notice>(null);
    const [loadingModels, setLoadingModels] = useState(false);
    const inFlightProvider = useRef("");
    const resave = useRef<() => Promise<boolean>>();
    useEffect(() => { if (!dirty.current && !saving.current)
        setConfig(snapshot.deepseek); }, [snapshot.deepseek]);
    const patch = (change: Partial<DeepSeekConfig>) => { dirty.current = true; version.current++; setConfig(current => ({ ...current, ...change })); setPhase("pending"); setSaveMessage(""); };
    const persist = useCallback(async () => {
        if (saving.current)
            return false;
        const target = latest.current;
        if (!validUrl(target.baseUrl) || !target.model.trim()) {
            setPhase("error");
            setSaveMessage("请检查服务地址和模型名称，尚未保存。");
            return false;
        }
        const submittedVersion = version.current;
        saving.current = true;
        setPhase("saving");
        setSaveMessage("");
        try {
            const { keyConfigured: _presence, ...payload } = target;
            const next = await run({ type: "updateDeepSeek", config: { ...payload, model: payload.model.trim(), baseUrl: payload.baseUrl.trim() } });
            const matches = next && next.deepseek.provider === target.provider && next.deepseek.model === target.model.trim() && normalUrl(next.deepseek.baseUrl) === normalUrl(target.baseUrl);
            if (!next || next.status.includes("失败") || !matches) {
                if (version.current === submittedVersion) {
                    setPhase("error");
                    setSaveMessage("尚未确认保存，编辑内容已保留，可重试。");
                }
                return false;
            }
            if (version.current === submittedVersion) {
                dirty.current = false;
                setPhase("saved");
            }
            return true;
        }
        finally {
            saving.current = false;
            if (dirty.current && version.current !== submittedVersion)
                void resave.current?.();
        }
    }, [run]);
    resave.current = persist;
    useEffect(() => { if (!dirty.current)
        return; const timer = window.setTimeout(() => { void persist(); }, 450); return () => window.clearTimeout(timer); }, [config, persist]);
    useEffect(() => () => { if (dirty.current)
        void resave.current?.(); }, []);
    useEffect(() => {
        if (!loadingModels || inFlightProvider.current !== config.provider)
            return;
        if (snapshot.status.includes("拉取模型列表失败")) {
            setLoadingModels(false);
            setModelNotice({ tone: "danger", message: "未能获取模型，请检查密钥与服务地址后重试；也可以手动填写模型名。" });
        }
        else if (snapshot.status.includes("没有返回任何模型")) {
            setLoadingModels(false);
            setModelNotice({ tone: "warning", message: "服务没有返回模型列表，可以填写服务商提供的模型名。" });
        }
        else if (snapshot.status.includes("已拉取")) {
            setLoadingModels(false);
            setModelNotice({ tone: "success", message: `已找到 ${snapshot.models.length} 个模型，可从输入框的列表中选择。` });
        }
    }, [snapshot.status, snapshot.models.length, loadingModels, config.provider]);
    const saveKey = async () => {
        if (!apiKey.trim() || savingKey)
            return;
        if (dirty.current && !await persist())
            return;
        const key = apiKey.trim();
        setSavingKey(true);
        setKeyNotice(null);
        try {
            const next = await run({ type: "saveDeepSeekKey", key });
            if (!next || next.status.includes("失败")) {
                setKeyNotice({ tone: "danger", message: "未能确认密钥已保存，输入已保留，请重试。" });
                return;
            }
            setApiKey("");
            setKeyNotice({ tone: "success", message: "密钥已保存。获取可用模型后就可以开始聊天。" });
        }
        finally {
            setSavingKey(false);
        }
    };
    const clearKey = async () => {
        if (!await confirm({ title: "清除连接密钥", message: "清除后需要重新填写密钥才能使用此服务。", confirmLabel: "清除" }))
            return;
        if (dirty.current && !await persist())
            return;
        setSavingKey(true);
        try {
            const next = await run({ type: "saveDeepSeekKey", key: "" });
            if (next && !next.status.includes("失败"))
                setApiKey("");
            setKeyNotice(!next || next.status.includes("失败") ? { tone: "danger", message: "未能确认密钥已清除，请重试。" } : { tone: "info", message: next.deepseek.keyConfigured ? "已清除保存的密钥，但当前启动环境仍提供连接授权。" : "密钥已清除。" });
        }
        finally {
            setSavingKey(false);
        }
    };
    const getModels = async () => {
        if (dirty.current && !await persist())
            return;
        inFlightProvider.current = config.provider;
        setLoadingModels(true);
        setModelNotice({ tone: "info", message: "正在获取可用模型…" });
        const next = await run({ type: "listModels" });
        if (next?.status.includes("已拉取")) {
            setLoadingModels(false);
            setModelNotice({ tone: "success", message: `已找到 ${next.models.length} 个模型。` });
        }
        else if (!next) {
            setLoadingModels(false);
            setModelNotice({ tone: "danger", message: "未能开始获取模型，请重试。" });
        }
    };
    const address = <SettingRow label="服务地址" hint="填写服务商提供的接口地址。"><TextField value={config.baseUrl} type="url" disabled={loadingModels || savingKey} onChange={baseUrl => patch({ baseUrl })} error={!validUrl(config.baseUrl) ? "请输入完整的 http:// 或 https:// 地址。" : undefined}/></SettingRow>;
    return <div className="page">
    <PageHeader title="模型服务" description="选择服务、填写密钥，再选模型。连接设置会自动保存。"/>
    <Card title="聊天服务">
      <SettingRow label="服务商"><SelectField value={config.provider} disabled={loadingModels || savingKey} options={providers} onChange={provider => { patch({ provider, baseUrl: provider === "deepseek" ? DEFAULT_URL : config.baseUrl }); setKeyNotice(null); setModelNotice(null); }}/></SettingRow>
      {config.provider === "custom" ? address : <details className="advanced-disclosure"><summary>高级：修改服务地址</summary>{address}</details>}
      <SettingRow label="连接密钥" hint={snapshot.deepseek.keyConfigured ? "已安全保存在系统中，填写新密钥会替换原来的。" : "从服务商获取密钥后粘贴到这里。"}>
        <div className="key-control"><TextField value={apiKey} type="password" disabled={savingKey || loadingModels} onChange={setApiKey} placeholder={snapshot.deepseek.keyConfigured ? "已保存，输入新密钥可替换" : "粘贴连接密钥"}/><Button disabled={savingKey || loadingModels || phase === "saving" || !apiKey.trim()} onClick={() => void saveKey()}>{savingKey ? "处理中…" : "保存密钥"}</Button><Button variant="ghost" disabled={savingKey || loadingModels || phase === "saving" || !snapshot.deepseek.keyConfigured} onClick={() => void clearKey()}>清除</Button></div>
      </SettingRow>
      <SettingRow label="模型" hint="获取列表后选择，也可以填写服务商提供的模型名。"><div className="model-control"><input aria-label="模型" className="control-input" disabled={loadingModels || savingKey} list="petsona-model-options" value={config.model} onChange={event => patch({ model: event.target.value })} placeholder="填写模型名"/><datalist id="petsona-model-options">{snapshot.models.map(model => <option key={model} value={model}/>)}</datalist><Button disabled={loadingModels || savingKey || phase === "saving" || !snapshot.deepseek.keyConfigured} onClick={() => void getModels()}>{loadingModels ? "获取中…" : "获取模型"}</Button></div></SettingRow>
      <ActionFooter status={<SaveStatus phase={phase} message={saveMessage}/>}>{phase === "error" && <Button onClick={() => void persist()}>重试保存</Button>}<Badge tone="neutral">{snapshot.deepseek.keyConfigured ? "已填写密钥" : "尚未填写密钥"}</Badge></ActionFooter>
      {keyNotice && <InlineNotice tone={keyNotice.tone}>{keyNotice.message}</InlineNotice>}
      {modelNotice && <InlineNotice tone={modelNotice.tone}>{modelNotice.message}</InlineNotice>}
    </Card>
    <HelpText>密钥安全保存在系统中。获取模型和聊天会连接你选择的服务。</HelpText>
    {confirmation}
  </div>;
}
