import { useEffect, useRef, useState } from "react";
import { Card, PageHeader, SettingRow, Switch, TextArea } from "../components/ui";
import type { PageProps } from "../types";

export function AppearancePage({ snapshot, run }: PageProps) {
  const [scale, setScale] = useState(snapshot.scale);
  const dirty = useRef(false);
  const timer = useRef<number | null>(null);
  const greeting = snapshot.settings.greeting;
  const [greetingText, setGreetingText] = useState(snapshot.persona.greeting ?? "");
  const greetingDirty = useRef(false);
  const greetingTimer = useRef<number | null>(null);
  const personaId = useRef(snapshot.persona.id);

  useEffect(() => {
    if (!dirty.current) setScale(snapshot.scale);
  }, [snapshot.scale]);

  useEffect(() => {
    personaId.current = snapshot.persona.id;
    greetingDirty.current = false;
    setGreetingText(snapshot.persona.greeting ?? "");
  }, [snapshot.persona.id]);

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
      if (greetingTimer.current !== null) window.clearTimeout(greetingTimer.current);
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

  const changeGreetingText = (value: string) => {
    setGreetingText(value);
    greetingDirty.current = true;
    if (greetingTimer.current !== null) window.clearTimeout(greetingTimer.current);
    const targetPersonaId = personaId.current;
    greetingTimer.current = window.setTimeout(async () => {
      if (personaId.current !== targetPersonaId) return;
      await run({ type: "updatePersona", patch: { greeting: value } });
      await run({ type: "savePersona" });
      greetingDirty.current = false;
      greetingTimer.current = null;
    }, 450);
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

        </div>
      </Card>

      <Card title="空闲问候" description="固定文案跟随当前宠物保存；这里控制是否启用。">
        <SettingRow label="启用空闲问候">
          <Switch
            label="启用空闲问候"
            checked={greeting.enabled}
            onChange={(value) => updateGreeting({ enabled: value })}
          />
        </SettingRow>
        <SettingRow stacked label="固定问候文案" hint="没有可用模型时作为回退；修改后随当前宠物自动保存。">
          <TextArea
            value={greetingText}
            onChange={changeGreetingText}
            rows={2}
            placeholder="例如：我在这儿呢，需要我陪你聊聊吗？"
          />
        </SettingRow>
      </Card>

    </div>
  );
}
