#!/usr/bin/env bash
# Smoke the native SwiftUI/AppKit app through the Rust FFI state protocol.
# This script intentionally never launches the legacy petsona-macos shell.
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PETSONA_APP="${PETSONA_NATIVE_APP:-$PETSONA_ROOT/.scratch/macos-native-gates/Build/Products/Release/Petsona.app}"
PETSONA_BINARY="$PETSONA_APP/Contents/MacOS/Petsona"
PETSONA_SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/petsona-native-smoke.XXXXXX")"
PETSONA_STATE_PORT="${PETSONA_SMOKE_STATE_PORT:-17872}"
PETSONA_PID=""
PETSONA_SECOND_PID=""
PETSONA_PASS_COUNT=0

cleanup() {
  local exit_code=$?
  trap - EXIT INT TERM
  if [[ -n "$PETSONA_PID" ]] && kill -0 "$PETSONA_PID" 2>/dev/null; then
    kill "$PETSONA_PID" 2>/dev/null || true
    for _ in {1..30}; do
      kill -0 "$PETSONA_PID" 2>/dev/null || break
      sleep 0.1
    done
    kill -9 "$PETSONA_PID" 2>/dev/null || true
  fi
  if [[ -n "$PETSONA_SECOND_PID" ]] && kill -0 "$PETSONA_SECOND_PID" 2>/dev/null; then
    kill "$PETSONA_SECOND_PID" 2>/dev/null || true
    kill -9 "$PETSONA_SECOND_PID" 2>/dev/null || true
  fi
  if [[ "${PETSONA_SMOKE_KEEP_ARTIFACTS:-0}" == "1" ]]; then
    printf 'Kept native smoke artifacts at %s\n' "$PETSONA_SMOKE_HOME"
  else
    rm -rf "$PETSONA_SMOKE_HOME"
  fi
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

fail() {
  printf '[FAIL] %s\n' "$1" >&2
  [[ -f "$PETSONA_SMOKE_HOME/stderr.log" ]] && sed -n '1,120p' "$PETSONA_SMOKE_HOME/stderr.log" >&2 || true
  exit 1
}

pass() {
  PETSONA_PASS_COUNT=$((PETSONA_PASS_COUNT + 1))
  printf '[PASS] %s\n' "$1"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "缺少命令：$1"
}

wait_for_health() {
  local path="$1"
  local expected="$2"
  local deadline=$((SECONDS + 8))
  local payload value
  while (( SECONDS <= deadline )); do
    payload="$(curl -fsS --connect-timeout 1 "http://127.0.0.1:$PETSONA_STATE_PORT/health" 2>/dev/null || true)"
    value="$(printf '%s' "$payload" | plutil -extract "$path" raw -o - - 2>/dev/null || true)"
    [[ "$value" == "$expected" ]] && return 0
    sleep 0.1
  done
  fail "等待原生 app health $path=$expected 超时（最后值：${value:-<empty>}）"
}

require_command curl
require_command plutil
[[ "$(uname -s)" == "Darwin" ]] || fail 'macOS smoke 只能在 macOS 上运行'
[[ -x "$PETSONA_BINARY" ]] || fail "找不到原生 app 可执行文件：$PETSONA_BINARY"

if command -v lsof >/dev/null 2>&1 && lsof -nP -iTCP:"$PETSONA_STATE_PORT" -sTCP:LISTEN -t 2>/dev/null | grep -q .; then
  fail "状态端口已被占用：$PETSONA_STATE_PORT"
fi

FIXTURE="$PETSONA_ROOT/crates/petsona-core/testdata/v2-test-pet"
[[ -f "$FIXTURE/pet.json" ]] || fail "缺少仓库 V2 smoke 夹具：$FIXTURE"
PET_ID="$(plutil -extract id raw -o - - <<< "$(cat "$FIXTURE/pet.json")" 2>/dev/null || sed -n 's/.*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$FIXTURE/pet.json" | head -1)"
[[ -n "$PET_ID" ]] || fail '无法读取 V2 smoke 夹具 id'
mkdir -p "$PETSONA_SMOKE_HOME/pets"
cp -R "$FIXTURE" "$PETSONA_SMOKE_HOME/pets/$PET_ID"
cat > "$PETSONA_SMOKE_HOME/config.json" <<EOF
{"activePet":"$PET_ID","firstRun":false,"greeting":{"enabled":false},"window":{"autoWalk":{"enabled":false}},"stateServer":{"enabled":true,"port":$PETSONA_STATE_PORT}}
EOF

PETSONA_HOME="$PETSONA_SMOKE_HOME" RUST_LOG=info "$PETSONA_BINARY" \
  >"$PETSONA_SMOKE_HOME/stdout.log" 2>"$PETSONA_SMOKE_HOME/stderr.log" &
PETSONA_PID=$!

for _ in {1..80}; do
  kill -0 "$PETSONA_PID" 2>/dev/null || fail '原生 Petsona app 启动后退出'
  curl -fsS --connect-timeout 1 "http://127.0.0.1:$PETSONA_STATE_PORT/health" >/dev/null 2>&1 && break
  sleep 0.1
done
kill -0 "$PETSONA_PID" 2>/dev/null || fail '原生 Petsona app 未保持运行'
pass '原生 SwiftUI/AppKit app 进程保持运行'

health="$(curl -fsS "http://127.0.0.1:$PETSONA_STATE_PORT/health")"
[[ "$(printf '%s' "$health" | plutil -extract ok raw -o - - 2>/dev/null)" == "true" ]] || fail '原生 app /health 未返回 ok'
[[ "$(printf '%s' "$health" | plutil -extract pet raw -o - - 2>/dev/null)" == "Pearl" || -n "$(printf '%s' "$health" | plutil -extract pet raw -o - - 2>/dev/null)" ]] || fail '原生 app /health 缺少宠物名'
pass '原生入口 /health 返回宠物快照'

pets="$(curl -fsS "http://127.0.0.1:$PETSONA_STATE_PORT/pets")"
[[ "$pets" == *"$PET_ID"* ]] || fail '原生入口 /pets 缺少本地 V2 宠物'
pass '原生入口 /pets 返回本地宠物库'

curl -fsS -X POST -H 'content-type: application/json' \
  --data '{"source":"native-smoke","state":"waiting","message":"native smoke","ttlMs":1000}' \
  "http://127.0.0.1:$PETSONA_STATE_PORT/state" >/dev/null
wait_for_health state waiting
pass '原生入口接收状态协议事件'
sleep 1.4
wait_for_health state idle
pass '原生入口执行 TTL 回退'

PETSONA_FIRST_HOME="$PETSONA_SMOKE_HOME"
PETSONA_SECOND_STDERR="$PETSONA_SMOKE_HOME/second-stderr.log"
PETSONA_HOME="$PETSONA_FIRST_HOME" RUST_LOG=info "$PETSONA_BINARY" \
  >"$PETSONA_SMOKE_HOME/second-stdout.log" 2>"$PETSONA_SECOND_STDERR" &
PETSONA_SECOND_PID=$!
for _ in {1..80}; do
  kill -0 "$PETSONA_SECOND_PID" 2>/dev/null || break
  sleep 0.1
done
if kill -0 "$PETSONA_SECOND_PID" 2>/dev/null; then
  fail '同一数据目录的第二个原生实例未自动退出'
fi
PETSONA_SECOND_PID=""
pass '同一数据目录的第二实例自动退出'

kill "$PETSONA_PID" 2>/dev/null || true
for _ in {1..30}; do
  kill -0 "$PETSONA_PID" 2>/dev/null || break
  sleep 0.1
done
kill -0 "$PETSONA_PID" 2>/dev/null && fail '原生 app 未能安全退出'
PETSONA_PID=""
pass '原生入口可安全退出并释放 worker'

printf '\nmacOS native smoke passed: %s checks\n' "$PETSONA_PASS_COUNT"
