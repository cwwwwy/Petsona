import { useState } from "react";
import { Badge, Button, Card, InlineNotice, PageHeader, SettingRow, Switch } from "../components/ui";
import { openExternalUrl, openPath, setAutostart } from "../lib/api";
import type { PageProps } from "../types";

const REPOSITORY_URL = "https://github.com/cwwwwy/Petsona";

function shortPath(value: string): string {
  if (value.length <= 64) return value;
  return `${value.slice(0, 28)}…${value.slice(-28)}`;
}

export function SystemPage({ snapshot }: PageProps) {
  const paths = snapshot.settings.paths;
  const [notice, setNotice] = useState("");
  const [autostartBusy, setAutostartBusy] = useState(false);

  const open = async (path: string) => {
    try {
      await openPath(path);
    } catch (reason) {
      setNotice(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const openRepository = async () => {
    try {
      await openExternalUrl(REPOSITORY_URL);
    } catch (reason) {
      setNotice(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const toggleAutostart = async (enabled: boolean) => {
    setAutostartBusy(true);
    setNotice("");
    try {
      await setAutostart(enabled);
      setNotice(enabled ? "已启用开机自启。" : "已关闭开机自启。");
    } catch (reason) {
      setNotice(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setAutostartBusy(false);
    }
  };

  const copyDiagnostics = async () => {
    const text = [
      `Petsona v${snapshot.appVersion}`,
      `platform=${snapshot.platform}/${snapshot.arch}`,
      `debug=${snapshot.debugBuild}`,
      `stateServer=127.0.0.1:${snapshot.stateServerPort}`,
      `revision=${snapshot.revision}`,
      `status=${snapshot.status || ""}`,
      `data=${paths.dataDir}`,
      `pets=${paths.petsDir}`,
      `logs=${paths.logsDir}`,
      `config=${paths.configFile}`,
      `memory=${paths.memoryFile}`,
    ].join("\n");
    try {
      await navigator.clipboard.writeText(text);
      setNotice("诊断信息已复制到剪贴板。");
    } catch {
      setNotice(text);
    }
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="系统"
        title="关于与诊断"
        description="版本、运行状态和本机数据位置。不会在这里发送任何内容。"
        actions={
          <div className="button-group">
            <Button variant="secondary" onClick={() => void copyDiagnostics()}>
              复制诊断
            </Button>

          </div>
        }
      />

      <Card title="Petsona">
        <div className="about-hero">
          <div className="brand-mark large">P</div>
          <div>
            <h2>Petsona</h2>
            <p>桌面上的宠物与 AI 伙伴</p>
          </div>
          <Badge tone="accent">v{snapshot.appVersion}</Badge>
        </div>
        <div className="about-links">
          <span className="mono">{REPOSITORY_URL}</span>
          <Button variant="ghost" onClick={() => void openRepository()}>
            访问
          </Button>
        </div>
        <SettingRow label="运行时">
          <Badge tone={snapshot.faulted ? "warning" : snapshot.ready ? "positive" : "neutral"}>
            {snapshot.faulted ? "故障" : snapshot.ready ? "就绪" : "启动中"}
          </Badge>
        </SettingRow>
      </Card>

      <Card title="启动" description="登录后自动启动 Petsona。">
        <SettingRow
          label="开机自启"
          hint="开启后，下次登录时 Petsona 会自动出现。"
        >
          <Switch
            label="开机自启"
            checked={snapshot.autostart}
            disabled={autostartBusy}
            onChange={(value) => void toggleAutostart(value)}
          />
        </SettingRow>
      </Card>

      <Card title="本机数据" description="宠物、设置和记忆保存在数据目录；日志用于排查问题。">
        <SettingRow label="数据目录">
          <div className="path-control">
            <span className="mono" title={paths.dataDir}>
              {shortPath(paths.dataDir)}
            </span>
            <Button variant="ghost" onClick={() => void open(paths.dataDir)}>
              打开
            </Button>
          </div>
        </SettingRow>
        <SettingRow label="日志目录">
          <div className="path-control">
            <span className="mono" title={paths.logsDir}>
              {shortPath(paths.logsDir)}
            </span>
            <Button variant="ghost" onClick={() => void open(paths.logsDir)}>
              打开
            </Button>
          </div>
        </SettingRow>
      </Card>

      <InlineNotice>主题跟随系统自动切换。</InlineNotice>
      {notice && <InlineNotice>{notice}</InlineNotice>}
    </div>
  );
}
