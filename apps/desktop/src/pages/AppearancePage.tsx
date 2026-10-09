import { useEffect, useRef, useState } from "react";
import {
  Badge,
  Button,
  Card,
  NumberField,
  PageHeader,
  SettingRow,
  Switch,
} from "../components/ui";
import type { PageProps } from "../types";
import { InlineNotice } from "../components/ui";

const SCALE_STOPS = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];

export function AppearancePage({ snapshot, run }: PageProps) {
  const [scale, setScale] = useState(snapshot.scale);
  const dirty = useRef(false);
  const timer = useRef<number | null>(null);
  const greeting = snapshot.settings.greeting;
  const conversation = snapshot.settings.conversation;

  useEffect(() => {
    if (!dirty.current) setScale(snapshot.scale);
  }, [snapshot.scale]);

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    },
    [],
  );

  const changeScale = (value: number) => {
    setScale(value);
    dirty.current = true;
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(async () => {
      try {
        await run({ type: "setScale", value });
      } finally {
        dirty.current = false;
        timer.current = null;
      }
    }, 100);
  };

  const updateGreeting = (patch: Partial<typeof greeting>) => {
    void run({ type: "updateGreeting", config: { ...greeting, ...patch } });
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="外观与交互"
        title="让它更像你习惯的样子"
        description="所有改动即时生效，不需要保存按钮。"
      />

      <Card title="缩放" description="0.5× 到 2.0×，拖动时实时预览，松手后写入配置。">
        <div className="scale-control">
          <div className="scale-value">
            <strong>{scale.toFixed(2)}×</strong>
            <span>当前 {snapshot.scale.toFixed(2)}×</span>
          </div>
          <input
            className="range"
            type="range"
            min={0.5}
            max={2}
            step={0.25}
            value={scale}
            onChange={(event) => changeScale(Number(event.target.value))}
            aria-label="宠物缩放"
          />
          <div className="range-stops" aria-hidden>
            {SCALE_STOPS.map((stop) => (
              <button
                key={stop}
                type="button"
                className={Math.abs(scale - stop) < 0.001 ? "active" : ""}
                onClick={() => changeScale(stop)}
              >
                {stop}
              </button>
            ))}
          </div>
        </div>
      </Card>

      <Card title="显示与交互">
        <SettingRow
          label="显示宠物"
          hint="隐藏后仍保留托盘与设置窗口，可随时恢复。"
        >
          <Switch
            label="显示宠物"
            checked={snapshot.petVisible}
            onChange={(value) => void run({ type: "setVisibility", value })}
          />
        </SettingRow>
        <SettingRow
          label="点击穿透"
          hint="鼠标点击透明像素和宠物都会落到桌面；关闭后可直接点击宠物。"
        >
          <Switch
            label="点击穿透"
            checked={snapshot.clickThrough}
            onChange={(value) => void run({ type: "setClickThrough", value })}
          />
        </SettingRow>
        <SettingRow
          label="始终置顶"
          hint="让宠物保持在其他普通窗口之上。"
        >
          <Switch
            label="始终置顶"
            checked={snapshot.alwaysOnTop}
            onChange={(value) => void run({ type: "setAlwaysOnTop", value })}
          />
        </SettingRow>
        <SettingRow
          label="重力下落"
          hint="松开宠物后，如果悬在空中，会落到当前显示器工作区底部。"
        >
          <Switch
            label="重力下落"
            checked={snapshot.gravityEnabled}
            onChange={(value) => void run({ type: "setGravity", value })}
          />
        </SettingRow>
        <SettingRow
          label="活动提醒"
          hint="空闲一段时间后，让宠物在自己的活动范围内走一小段。"
        >
          <Switch
            label="活动提醒"
            checked={snapshot.autoWalk}
            onChange={(value) => void run({ type: "setAutoWalk", value })}
          />
        </SettingRow>
      </Card>

      <Card title="空闲问候" description="问候文案在「人格」页配置；这里控制触发节奏。">
        <SettingRow label="启用空闲问候">
          <Switch
            label="启用空闲问候"
            checked={greeting.enabled}
            onChange={(value) => updateGreeting({ enabled: value })}
          />
        </SettingRow>
        <SettingRow label="空闲多久后问候">
          <NumberField
            value={greeting.idleMinutes}
            min={1}
            max={1440}
            suffix="分钟"
            onChange={(value) => updateGreeting({ idleMinutes: value })}
          />
        </SettingRow>
        <SettingRow label="两次问候的冷却时间">
          <NumberField
            value={greeting.cooldownMinutes}
            min={1}
            max={10080}
            suffix="分钟"
            onChange={(value) => updateGreeting({ cooldownMinutes: value })}
          />
        </SettingRow>
        <SettingRow label="问候最大字数">
          <NumberField
            value={greeting.maxChars}
            min={8}
            max={200}
            suffix="字"
            onChange={(value) => updateGreeting({ maxChars: value })}
          />
        </SettingRow>
      </Card>

      <Card title="聊天与主题">
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
        <SettingRow label="主题" hint="跟随 Windows 系统主题，切换后窗口内容即时更新。">
          <Badge tone="accent">跟随系统</Badge>
        </SettingRow>
      </Card>

      <InlineNotice>
        当前批次先接通配置读写；活动范围、速度和穿透快捷键等高级参数会在后续设置批次补充。
      </InlineNotice>
    </div>
  );
}
