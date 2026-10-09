import { useState } from "react";
import { Badge, Button, Card, PageHeader, SettingRow } from "../components/ui";
import { openExternalUrl, openPath } from "../lib/api";
import type { PageProps } from "../types";

const REPOSITORY_URL = "https://github.com/cwwwwy/Petsona";

function shortPath(value: string): string {
  if (value.length <= 64) return value;
  return `${value.slice(0, 28)}…${value.slice(-28)}`;
}

export function SystemPage({ snapshot }: PageProps) {
  const paths = snapshot.settings.paths;
  const [notice, setNotice] = useState("");

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

  const copyDiagnostics = async () => {
    const text = [
      `Petsona v${snapshot.appVersion}`,
      `platform=${snapshot.platform}/${snapshot.arch}`,
      `debug=${snapshot.debugBuild}`,
      `stateServer=127.0.0.1:${snapshot.stateServerPort}`,
      `revision=${snapshot.revision}`,
      `status=${snapshot.status || ""}`,
      `data=${paths.dataDir}`,
      `logs=${paths.logsDir}`,
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
            <Button variant="primary" onClick={() => void openRepository()}>
              打开仓库
            </Button>
          </div>
        }
      />

      <Card title="Petsona">
        <div className="about-hero">
          <div className="brand-mark large">P</div>
          <div>
            <h2>Petsona</h2>
            <p>Rust 原生浮层 + TypeScript 内容界面</p>
          </div>
          <Badge tone="accent">v{snapshot.appVersion}</Badge>
        </div>
        <div className="about-links">
          <span className="mono">{REPOSITORY_URL}</span>
          <Button variant="ghost" onClick={() => void openRepository()}>
            访问
          </Button>
        </div>
      </Card>

      <Card title="运行状态">
        <SettingRow label="运行时">
          <Badge tone={snapshot.faulted ? "warning" : snapshot.ready ? "positive" : "neutral"}>
            {snapshot.faulted ? "故障" : snapshot.ready ? "就绪" : "启动中"}
          </Badge>
        </SettingRow>
        <SettingRow label="状态协议端口">
          <span className="mono">127.0.0.1:{snapshot.stateServerPort}</span>
        </SettingRow>
        <SettingRow label="配置版本">
          <span className="mono">revision {snapshot.revision}</span>
        </SettingRow>
        <SettingRow label="平台 / 架构">
          <span className="mono">
            {snapshot.platform} / {snapshot.arch}
          </span>
        </SettingRow>
        <SettingRow label="构建类型">
          <Badge tone={snapshot.debugBuild ? "warning" : "positive"}>
            {snapshot.debugBuild ? "调试版" : "发布版"}
          </Badge>
        </SettingRow>
        {snapshot.status && (
          <SettingRow label="最近状态">
            <span className="muted wrap">{snapshot.status}</span>
          </SettingRow>
        )}
        {snapshot.error && (
          <SettingRow label="错误信息">
            <span className="danger-text wrap">{snapshot.error}</span>
          </SettingRow>
        )}
      </Card>

      <Card title="本机数据" description="设置窗口关闭不会退出 Petsona；从托盘菜单「退出」才会结束进程。">
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
        <SettingRow label="宠物目录">
          <div className="path-control">
            <span className="mono" title={paths.petsDir}>
              {shortPath(paths.petsDir)}
            </span>
            <Button variant="ghost" onClick={() => void open(paths.petsDir)}>
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
        <SettingRow label="配置文件">
          <div className="path-control">
            <span className="mono" title={paths.configFile}>
              {shortPath(paths.configFile)}
            </span>
            <Button variant="ghost" onClick={() => void open(paths.configFile)}>
              打开
            </Button>
          </div>
        </SettingRow>
        <SettingRow label="记忆文件">
          <div className="path-control">
            <span className="mono" title={paths.memoryFile}>
              {shortPath(paths.memoryFile)}
            </span>
            <Button variant="ghost" onClick={() => void open(paths.memoryFile)}>
              打开
            </Button>
          </div>
        </SettingRow>
      </Card>

      {notice && (
        <Card title="提示">
          <p className="muted wrap">{notice}</p>
        </Card>
      )}
    </div>
  );
}
