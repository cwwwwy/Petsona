import { useEffect, useState } from "react";
import { Badge, Button, Card, EmptyState, PageHeader } from "../components/ui";
import type { PageProps } from "../types";

export function PetsPage({ snapshot, run }: PageProps) {
  const [selectedId, setSelectedId] = useState(snapshot.petId);
  const [scanning, setScanning] = useState(false);

  useEffect(() => {
    setSelectedId(snapshot.petId);
  }, [snapshot.petId]);

  const selected = snapshot.pets.find((pet) => pet.id === selectedId) ?? null;

  const scan = async () => {
    setScanning(true);
    try {
      await run({ type: "scanCodexPets" });
    } finally {
      setScanning(false);
    }
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow="宠物"
        title="宠物库"
        description="本地宠物与当前桌宠。双击列表中的宠物即可切换；单击只改变选择。"
        actions={
          <Button variant="primary" disabled title="文件选择将在 M3-B 接入">
            导入
          </Button>
        }
      />

      <Card title="当前宠物">
        {snapshot.hasPet ? (
          <div className="active-pet">
            <div className="pet-avatar" aria-hidden>
              {snapshot.petName.slice(0, 1).toUpperCase()}
            </div>
            <div className="active-pet-copy">
              <div className="row-title">
                <strong>{snapshot.petName}</strong>
                <Badge tone="positive">正在使用</Badge>
              </div>
              <span className="mono">{snapshot.petId}</span>
            </div>
            <div className="active-pet-meta">
              <span>缩放 {snapshot.scale.toFixed(2)}×</span>
              <span>{snapshot.petVisible ? "宠物已显示" : "宠物已隐藏"}</span>
            </div>
          </div>
        ) : (
          <EmptyState
            title="还没有可用的宠物"
            description="本地宠物库为空。可以扫描 Codex 宠物，或稍后通过导入加入一个宠物包。"
          />
        )}
      </Card>

      <Card
        title="本地宠物库"
        description={`${snapshot.pets.length} 个宠物包。单击选择，双击切换。`}
      >
        {snapshot.pets.length === 0 ? (
          <EmptyState
            title="宠物库为空"
            description="Petsona 不会内置宠物。导入或从 Codex 扫描后，宠物会显示在这里。"
          />
        ) : (
          <div className="pet-list">
            {snapshot.pets.map((pet) => {
              const active = pet.id === snapshot.petId;
              const selectedRow = pet.id === selectedId;
              return (
                <button
                  key={pet.id}
                  type="button"
                  className={`pet-row${selectedRow ? " pet-row-selected" : ""}`}
                  onClick={() => setSelectedId(pet.id)}
                  onDoubleClick={() => void run({ type: "selectPet", id: pet.id })}
                >
                  <div className="pet-thumb" aria-hidden>
                    {pet.name.slice(0, 1).toUpperCase()}
                  </div>
                  <div className="pet-row-copy">
                    <strong>{pet.name}</strong>
                    <span className="mono">{pet.id}</span>
                  </div>
                  <div className="pet-row-tags">
                    {pet.v2 ? <Badge tone="accent">V2</Badge> : <Badge>V1</Badge>}
                    {active && <Badge tone="positive">当前</Badge>}
                  </div>
                </button>
              );
            })}
          </div>
        )}
        <div className="card-footer">
          <span className="muted">
            {selected ? `已选择：${selected.name}` : "选择一只宠物以查看操作"}
          </span>
          <Button
            variant="primary"
            disabled={!selected || selected.id === snapshot.petId}
            onClick={() => selected && void run({ type: "selectPet", id: selected.id })}
          >
            设为当前宠物
          </Button>
        </div>
      </Card>

      <Card
        title="从 Codex 导入"
        description="扫描 ~/.codex/pets 中的宠物。这里不会修改 Codex 的原始文件。"
      >
        <div className="section-toolbar">
          <Button onClick={() => void scan()} disabled={scanning}>
            {scanning ? "正在扫描…" : "扫描 Codex 宠物"}
          </Button>
          <span className="muted">找到 {snapshot.codexPets.length} 个候选</span>
        </div>
        {snapshot.codexPets.length === 0 ? (
          <EmptyState
            title="还没有扫描结果"
            description="点击扫描后，Codex 宠物会在这里列出。"
          />
        ) : (
          <div className="pet-list compact">
            {snapshot.codexPets.map((pet) => (
              <div className="pet-row pet-row-static" key={pet.id}>
                <div className="pet-thumb" aria-hidden>
                  {pet.name.slice(0, 1).toUpperCase()}
                </div>
                <div className="pet-row-copy">
                  <strong>{pet.name}</strong>
                  <span className="truncate" title={pet.path}>
                    {pet.path}
                  </span>
                </div>
                <Badge>待导入</Badge>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
