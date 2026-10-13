import { useState } from "react";
import { BrandMark } from "../components/BrandMark";
import { Badge, Button, Card, HelpText, InlineNotice, PageHeader, SettingRow, Switch } from "../components/ui";
import { openExternalUrl, openPath, setAutostart } from "../lib/api";
import type { PageProps } from "../types";
const REPOSITORY_URL = "https://github.com/cwwwwy/Petsona";
function shortPath(value: string): string {
    if (value.length <= 64)
        return value;
    return `${value.slice(0, 28)}…${value.slice(-28)}`;
}
export function SystemPage({ snapshot }: PageProps) {
    const paths = snapshot.settings.paths;
    const [notice, setNotice] = useState("");
    const [noticeTone, setNoticeTone] = useState<"info" | "success" | "danger">("info");
    const [autostartBusy, setAutostartBusy] = useState(false);
    const open = async (path: string) => {
        try {
            await openPath(path);
        }
        catch (reason) {
            setNoticeTone("danger");
            setNotice(reason instanceof Error ? reason.message : String(reason));
        }
    };
    const openRepository = async () => {
        try {
            await openExternalUrl(REPOSITORY_URL);
        }
        catch (reason) {
            setNoticeTone("danger");
            setNotice(reason instanceof Error ? reason.message : String(reason));
        }
    };
    const toggleAutostart = async (enabled: boolean) => {
        setAutostartBusy(true);
        setNotice("");
        try {
            await setAutostart(enabled);
            setNoticeTone("success");
            setNotice(enabled ? "已启用开机自启。" : "已关闭开机自启。");
        }
        catch (reason) {
            setNoticeTone("danger");
            setNotice(reason instanceof Error ? reason.message : String(reason));
        }
        finally {
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
            setNoticeTone("success");
            setNotice("问题信息已复制，可以粘贴给协助排查的人。");
        }
        catch {
            setNoticeTone("danger");
            setNotice("复制未完成，请确认剪贴板权限后重试。");
        }
    };
    return (<div className="page">
      <PageHeader title="系统" description="管理启动方式与本机数据。"/>

      <Card>
        <div className="about-hero">
          <BrandMark large/>
          <div>
            <h2>Petsona</h2>
            <p>桌面上的宠物与 AI 伙伴</p>
          </div>
          <Badge tone="accent">v{snapshot.appVersion}</Badge>
        </div>
        <div className="about-links">
          <span>项目主页</span>
          <Button variant="ghost" onClick={() => void openRepository()}>
            打开
          </Button>
        </div>
        <SettingRow label="应用状态">
          <Badge tone={snapshot.faulted ? "warning" : snapshot.ready ? "positive" : "neutral"}>
            {snapshot.faulted ? "故障" : snapshot.ready ? "就绪" : "启动中"}
          </Badge>
        </SettingRow>
      </Card>

      <Card title="启动">
        <SettingRow label="开机自启" hint="开启后，下次登录时 Petsona 会自动出现。">
          <Switch label="开机自启" checked={snapshot.autostart} disabled={autostartBusy} onChange={(value) => void toggleAutostart(value)}/>
        </SettingRow>
      </Card>

      <Card title="本机数据">
        <SettingRow label="数据文件夹">
          <div className="path-control">
            <span className="mono" title={paths.dataDir}>
              {shortPath(paths.dataDir)}
            </span>
            <Button variant="ghost" onClick={() => void open(paths.dataDir)}>
              打开
            </Button>
          </div>
        </SettingRow>
        <details className="advanced-disclosure"><summary>遇到问题？查看排查工具</summary><Button variant="secondary" onClick={() => void copyDiagnostics()}>复制问题信息</Button>
        <SettingRow label="排查日志">
          <div className="path-control">
            <span className="mono" title={paths.logsDir}>
              {shortPath(paths.logsDir)}
            </span>
            <Button variant="ghost" onClick={() => void open(paths.logsDir)}>
              打开
            </Button>
          </div>
        </SettingRow>
        </details>
      </Card>

      <HelpText>外观跟随系统的浅色与深色模式。</HelpText>
      {notice && <InlineNotice tone={noticeTone}>{notice}</InlineNotice>}
    </div>);
}
