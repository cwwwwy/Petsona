import { useEffect, useRef, useState } from "react";
import { Button, ActionFooter, SaveStatus, Card, PageHeader, SettingRow, Switch, TextArea } from "../components/ui";
import type { PageProps } from "../types";
export function AppearancePage({ snapshot, run }: PageProps) {
    const [scale, setScale] = useState(snapshot.scale);
    const dirty = useRef(false);
    const timer = useRef<number | null>(null);
    const greeting = snapshot.settings.greeting;
    const [greetingText, setGreetingText] = useState(snapshot.persona.greeting ?? "");
    const greetingDirty = useRef(false);
    const greetingTimer = useRef<number | null>(null);
    const [greetingPhase, setGreetingPhase] = useState<"saved" | "pending" | "saving" | "error">("saved");
    const greetingVersion = useRef(0);
    const personaId = useRef(snapshot.persona.id);
    useEffect(() => {
        if (!dirty.current)
            setScale(snapshot.scale);
    }, [snapshot.scale]);
    useEffect(() => {
        personaId.current = snapshot.persona.id;
        greetingDirty.current = false;
        setGreetingText(snapshot.persona.greeting ?? "");
        setGreetingPhase("saved");
    }, [snapshot.persona.id]);
    useEffect(() => { if (!greetingDirty.current)
        setGreetingText(snapshot.persona.greeting ?? ""); }, [snapshot.persona.greeting]);
    useEffect(() => () => {
        if (timer.current !== null)
            window.clearTimeout(timer.current);
        if (greetingTimer.current !== null)
            window.clearTimeout(greetingTimer.current);
    }, []);
    const changeScale = (value: number) => {
        setScale(value);
        dirty.current = true;
        if (timer.current !== null)
            window.clearTimeout(timer.current);
        timer.current = window.setTimeout(async () => {
            try {
                await run({ type: "setScale", value });
            }
            finally {
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
        setGreetingPhase("pending");
        if (greetingTimer.current !== null)
            window.clearTimeout(greetingTimer.current);
        const submitted = ++greetingVersion.current;
        const targetPersonaId = personaId.current;
        greetingTimer.current = window.setTimeout(async () => {
            if (personaId.current !== targetPersonaId)
                return;
            setGreetingPhase("saving");
            const changed = await run({ type: "updatePersona", patch: { pet_id: snapshot.petId, greeting: value } });
            const saved = changed && !changed.status.includes("失败") ? await run({ type: "savePersona" }) : null;
            if (submitted !== greetingVersion.current || personaId.current !== targetPersonaId)
                return;
            if (!saved || saved.status.includes("失败") || saved.persona.greeting !== value) {
                setGreetingPhase("error");
                return;
            }
            setGreetingPhase("saved");
            greetingDirty.current = false;
            greetingTimer.current = null;
        }, 450);
    };
    return (<div className="page">
      <PageHeader title="显示与问候" description="调整宠物大小，以及它主动与你打招呼的方式。"/>

      <Card title="缩放" description="拖动即可预览，大小会自动保存。">
        <div className="scale-control">
          <div className="scale-value">
            <strong>{scale.toFixed(2)}×</strong>
          </div>
          <input className="range" type="range" min={0.5} max={2} step={0.25} value={scale} onChange={(event) => changeScale(Number(event.target.value))} aria-label="宠物缩放"/>

        </div>
      </Card>

      <Card title="空闲问候" description="开启后，空闲时它会主动打招呼。">
        <SettingRow label="启用空闲问候">
          <Switch label="启用空闲问候" checked={greeting.enabled} onChange={(value) => updateGreeting({ enabled: value })}/>
        </SettingRow>
        <SettingRow stacked label="固定问候文案" hint="模型服务不可用时显示这句话；修改后随当前宠物自动保存。">
          <TextArea value={greetingText} disabled={!snapshot.hasPet} onChange={changeGreetingText} rows={2} placeholder="例如：我在这儿呢，需要我陪你聊聊吗？"/>
        </SettingRow>
        <ActionFooter status={<SaveStatus phase={greetingPhase}/>}>{greetingPhase === "error" && <Button onClick={() => changeGreetingText(greetingText)}>重试保存</Button>}</ActionFooter>
      </Card>

    </div>);
}
