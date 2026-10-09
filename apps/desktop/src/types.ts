export type PageId =
  | "pets"
  | "appearance"
  | "persona"
  | "memory"
  | "connection"
  | "system";

export interface PetInfo {
  id: string;
  name: string;
  v2: boolean;
  spritesheet: string;
  cellWidth: number;
  cellHeight: number;
}

export interface CodexPetInfo extends PetInfo {
  path: string;
  columns: number;
}

export interface PersonaTraits {
  tone: string;
  verbosity: "short" | "normal" | "detailed" | string;
  emoji: boolean;
}

export interface PersonaStyleProfile {
  personality: string;
  expressionStyle: string;
  responseHabits: string;
  relationship: string;
  examples: string[];
}

export interface Persona {
  id: string;
  name: string;
  systemPrompt: string;
  greeting?: string | null;
  traits: PersonaTraits;
  styleProfile?: PersonaStyleProfile | null;
  source?: unknown;
  builtin: boolean;
}

export interface PersonaSummary {
  id: string;
  name: string;
  builtin: boolean;
}

export interface DeepSeekConfig {
  provider: "deepseek" | "custom" | string;
  baseUrl: string;
  model: string;
  apiKeyEnv: string;
  timeoutSeconds: number;
  maxTokens: number;
  conversationMaxTokens: number;
  temperature: number;
  thinkingDisabled: boolean;
  keyConfigured: boolean;
}

export interface GreetingConfig {
  enabled: boolean;
  idleMinutes: number;
  cooldownMinutes: number;
  maxChars: number;
}

export interface MemoryConfig {
  enabled: boolean;
  recentEvents: number;
  factLimit: number;
  eventRetentionDays: number;
  factCompress: boolean;
}

export interface ConversationConfig {
  saveHistory: boolean;
}

export interface AutoWalkConfig {
  enabled: boolean;
  intervalMinutes: number;
  walkSeconds: number;
  speedPxS: number;
  rangePx: number;
  userGraceSeconds: number;
}

export interface PetWindowConfig {
  scale: number;
  opacity: number;
  alwaysOnTop: boolean;
  clickThrough: boolean;
  gravityEnabled: boolean;
  autoWalk: AutoWalkConfig;
  startPosition: {
    x: number;
    y: number;
    displayId?: string | null;
    backingScale: number;
  } | null;
}

export interface SettingsPaths {
  dataDir: string;
  petsDir: string;
  personasDir: string;
  logsDir: string;
  configFile: string;
  memoryFile: string;
}

export interface SettingsConfig {
  activePet?: string | null;
  activePersona?: string | null;
  firstRun: boolean;
  window: PetWindowConfig;
  greeting: GreetingConfig;
  memory: MemoryConfig;
  conversation: ConversationConfig;
  stateServer: { enabled: boolean; port: number };
  paths: SettingsPaths;
}

export interface MemoryFact {
  id: string;
  key: string;
  value: string;
  confidence: number;
  createdAt: number;
  updatedAt: number;
  source: "manual" | "conversation" | "import" | "compressed" | string;
}

export interface MemoryEvent {
  id: string;
  kind: string;
  text?: string | null;
  createdAt: number;
}

export interface MemoryProjection {
  config: MemoryConfig;
  greeting: GreetingConfig;
  facts: MemoryFact[];
  archivedFacts: MemoryFact[];
  candidates: unknown[];
  learning: boolean;
  events: MemoryEvent[];
  lastSeenAt?: number | null;
  lastGreetingAt?: number | null;
  lastTrigger?: string | null;
}

export interface SettingsSnapshot {
  appVersion: string;
  platform: string;
  revision: number;
  ready: boolean;
  faulted: boolean;
  error: string;
  status: string;
  hasPet: boolean;
  petVisible: boolean;
  clickThrough: boolean;
  scale: number;
  autoWalk: boolean;
  gravityEnabled: boolean;
  alwaysOnTop: boolean;
  stateServerPort: number;
  petId: string;
  petName: string;
  settings: SettingsConfig;
  pets: PetInfo[];
  codexPets: CodexPetInfo[];
  persona: Persona;
  personas: PersonaSummary[];
  deepseek: DeepSeekConfig;
  memory: MemoryProjection;
  models: string[];
  importConflict: unknown;
}

export type SettingsAction =
  | { type: "setScale"; value: number }
  | { type: "setClickThrough"; value: boolean }
  | { type: "setAlwaysOnTop"; value: boolean }
  | { type: "setGravity"; value: boolean }
  | { type: "setAutoWalk"; value: boolean }
  | { type: "setVisibility"; value: boolean }
  | { type: "selectPet"; id: string }
  | { type: "refreshPets" }
  | { type: "scanCodexPets" }
  | {
      type: "updatePersona";
      patch: {
        name?: string;
        tone?: string;
        verbosity?: string;
        emoji?: boolean;
        greeting?: string;
        system_prompt?: string;
      };
    }
  | { type: "savePersona" }
  | { type: "resetPersona" }
  | { type: "updateDeepSeek"; config: Omit<DeepSeekConfig, "keyConfigured"> }
  | { type: "saveDeepSeekKey"; key: string }
  | { type: "listModels" }
  | { type: "updateGreeting"; config: GreetingConfig }
  | { type: "updateMemoryConfig"; config: MemoryConfig }
  | { type: "updateConversation"; config: ConversationConfig }
  | { type: "clearMemory"; scope: 0 | 1 | 2 }
  | { type: "forgetFact"; id: string }
  | { type: "updateFact"; fact: Pick<MemoryFact, "id" | "key" | "value" | "confidence"> }
  | { type: "rememberFact"; fact: { key: string; value: string; confidence?: number } };

export interface PageProps {
  snapshot: SettingsSnapshot;
  run: (action: SettingsAction) => Promise<void>;
}
