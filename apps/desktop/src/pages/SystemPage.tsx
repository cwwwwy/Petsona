import { Badge, Button, Card, PageHeader, SettingRow } from "../components/ui";
import { openPath } from "../lib/api";
import type { PageProps } from "../types";

function shortPath(value: string): string {
  if (value.length <= 64) return value;
  return `${value.slice(0, 28)}…${value.slice(-28)}`;
}

export function SystemPage({ snapshot }: PageProps) {
  const paths = snapshot.settings.paths;

  const open = async (path: string) => {
    try {
      await openPath(path);
    } catch (reason) {
      window.alert(reason instanceof Error ? reason.message : String(reason));
    }
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="系统"
        title="关于与诊断"
        description="版本、运行状态和本机数据位置。不会在这里发送任何内容。"
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
        <SettingRow label="平台">
          <span className="mono">{snapshot.platform}</span>
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
    </div>
  );
}
