#!/usr/bin/env bash
# Deterministic macOS application smoke test.
#
# This starts the real app with the opt-in test-hooks feature and checks its
# loopback control channel. It does not claim visual, focus, power, or
# multi-monitor coverage.
set -euo pipefail

PETSONA_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PETSONA_BINARY="$PETSONA_ROOT/target/release/petsona"
PETSONA_SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/petsona-macos-smoke.XXXXXX")"
PETSONA_STATE_PORT="${PETSONA_SMOKE_STATE_PORT:-17872}"
PETSONA_HOOK_PORT="${PETSONA_SMOKE_HOOK_PORT:-17873}"
PETSONA_HOOK_TOKEN="${PETSONA_SMOKE_TOKEN:-$(uuidgen 2>/dev/null || printf 'petsona-macos-smoke-%s' "$$")}"
PETSONA_PID=""
PETSONA_V2_PET_DIR=""
PETSONA_PASS_COUNT=0

cleanup() {
  local exit_code=$?
  trap - EXIT INT TERM
  if [[ -n "$PETSONA_PID" ]] && kill -0 "$PETSONA_PID" 2>/dev/null; then
    kill "$PETSONA_PID" 2>/dev/null || true
    for _ in {1..20}; do
      kill -0 "$PETSONA_PID" 2>/dev/null || break
      sleep 0.1
    done
    kill -9 "$PETSONA_PID" 2>/dev/null || true
  fi
  if [[ "${PETSONA_SMOKE_KEEP_ARTIFACTS:-0}" == "1" ]]; then
    printf 'Kept smoke artifacts at %s\n' "$PETSONA_SMOKE_HOME"
  else
    rm -rf "$PETSONA_SMOKE_HOME"
  fi
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

fail() {
  printf '[FAIL] %s\n' "$1" >&2
  if [[ -f "$PETSONA_SMOKE_HOME/stderr.log" ]]; then
    printf '%s\n' '--- Petsona stderr ---' >&2
    sed -n '1,120p' "$PETSONA_SMOKE_HOME/stderr.log" >&2 || true
  fi
  exit 1
}

pass() {
  PETSONA_PASS_COUNT=$((PETSONA_PASS_COUNT + 1))
  printf '[PASS] %s\n' "$1"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "缺少命令：$1"
}

json_value() {
  local payload="$1"
  local keypath="$2"
  printf '%s' "$payload" | /usr/bin/plutil -extract "$keypath" raw -o - - 2>/dev/null || true
}

hook_status() {
  curl -fsS --connect-timeout 1 -H "x-petsona-token: $PETSONA_HOOK_TOKEN" "http://127.0.0.1:$PETSONA_HOOK_PORT/test/status"
}

hook_action() {
  curl -fsS --connect-timeout 1 -X POST -H "x-petsona-token: $PETSONA_HOOK_TOKEN" -H 'content-type: application/json' --data "$1" "http://127.0.0.1:$PETSONA_HOOK_PORT/test/action" >/dev/null
}

state_health() {
  curl -fsS --connect-timeout 1 "http://127.0.0.1:$PETSONA_STATE_PORT/health"
}

state_pets() {
  curl -fsS --connect-timeout 1 "http://127.0.0.1:$PETSONA_STATE_PORT/pets"
}

state_post() {
  curl -fsS --connect-timeout 1 -X POST -H 'content-type: application/json' --data "$1" "http://127.0.0.1:$PETSONA_STATE_PORT/state" >/dev/null
}

wait_for_hook() {
  local keypath="$1"
  local expected="$2"
  local timeout_seconds="${3:-3}"
  local deadline=$((SECONDS + timeout_seconds))
  local payload value=""
  while (( SECONDS <= deadline )); do
    payload="$(hook_status 2>/dev/null || true)"
    value="$(json_value "$payload" "$keypath")"
    [[ "$value" == "$expected" ]] && return 0
    sleep 0.05
  done
  fail "等待 test status $keypath=$expected 超时（最后值：${value:-<empty>}）"
}

wait_for_health() {
  local keypath="$1"
  local expected="$2"
  local timeout_seconds="${3:-3}"
  local deadline=$((SECONDS + timeout_seconds))
  local payload value=""
  while (( SECONDS <= deadline )); do
    payload="$(state_health 2>/dev/null || true)"
    value="$(json_value "$payload" "$keypath")"
    [[ "$value" == "$expected" ]] && return 0
    sleep 0.05
  done
  fail "等待 health $keypath=$expected 超时（最后值：${value:-<empty>}）"
}

wait_for_hook_empty() {
  local keypath="$1"
  local timeout_seconds="${2:-3}"
  local deadline=$((SECONDS + timeout_seconds))
  local payload value=""
  while (( SECONDS <= deadline )); do
    payload="$(hook_status 2>/dev/null || true)"
    value="$(json_value "$payload" "$keypath")"
    [[ -z "$value" ]] && return 0
    sleep 0.05
  done
  fail "等待 test status $keypath 为空超时（最后值：${value}）"
}

assert_hook_value() {
  local keypath="$1"
  local expected="$2"
  local name="$3"
  local payload value
  payload="$(hook_status)"
  value="$(json_value "$payload" "$keypath")"
  [[ "$value" == "$expected" ]] || fail "${name}（实际：${value:-<empty>}，预期：${expected}）"
  pass "$name"
}

find_v2_pet() {
  local root manifest version
  if [[ -n "${PETSONA_SMOKE_V2_PET_DIR:-}" ]]; then
    [[ -f "$PETSONA_SMOKE_V2_PET_DIR/pet.json" ]] || fail "V2 宠物目录无 pet.json：$PETSONA_SMOKE_V2_PET_DIR"
    printf '%s\n' "$PETSONA_SMOKE_V2_PET_DIR"
    return 0
  fi
  for root in "$HOME/.codex/pets" "$HOME/.unipet/pets"; do
    [[ -d "$root" ]] || continue
    while IFS= read -r -d '' manifest; do
      version="$(json_value "$(<"$manifest")" spriteVersionNumber)"
      if [[ "$version" == "2" ]]; then
        printf '%s\n' "${manifest%/pet.json}"
        return 0
      fi
    done < <(find "$root" -mindepth 2 -maxdepth 3 -type f -name pet.json -print0 2>/dev/null)
  done
  return 1
}

require_command cargo
require_command curl
require_command plutil
[[ "$(uname -s)" == "Darwin" ]] || fail 'macOS smoke 只能在 macOS 上运行'

if [[ "${PETSONA_SMOKE_SKIP_BUILD:-0}" != "1" ]]; then
  printf 'Building petsona-app with test hooks...\n'
  (cd "$PETSONA_ROOT" && cargo build -p petsona-app --release --features test-hooks --locked)
fi
[[ -x "$PETSONA_BINARY" ]] || fail "找不到 release 可执行文件：$PETSONA_BINARY"

if command -v lsof >/dev/null 2>&1; then
  for port in "$PETSONA_STATE_PORT" "$PETSONA_HOOK_PORT"; do
    if lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null | grep -q .; then
      fail "端口已被占用：${port}；请停止现有 Petsona 或通过 PETSONA_SMOKE_*_PORT 改端口"
    fi
  done
fi

if PETSONA_V2_PET_DIR="$(find_v2_pet)"; then
  PETSONA_V2_ID="$(json_value "$(<"$PETSONA_V2_PET_DIR/pet.json")" id)"
  printf 'V2 gaze fixture: %s (%s)\n' "$PETSONA_V2_ID" "$PETSONA_V2_PET_DIR"
  PETSONA_ACTIVE_PET_JSON="\"$PETSONA_V2_ID\""
else
  printf 'No V2 pet found; gaze app smoke will be skipped (set PETSONA_SMOKE_V2_PET_DIR to enable it).\n'
  PETSONA_ACTIVE_PET_JSON='null'
fi

mkdir -p "$PETSONA_SMOKE_HOME"
printf '{"activePet":%s,"greeting":{"enabled":false},"window":{"autoWalk":{"enabled":false}},"stateServer":{"enabled":true,"port":%s}}\n' "$PETSONA_ACTIVE_PET_JSON" "$PETSONA_STATE_PORT" > "$PETSONA_SMOKE_HOME/config.json"

PETSONA_HOME="$PETSONA_SMOKE_HOME" PETSONA_TEST_HOOKS=1 PETSONA_TEST_HOOKS_PORT="$PETSONA_HOOK_PORT" PETSONA_TEST_HOOKS_TOKEN="$PETSONA_HOOK_TOKEN" RUST_LOG=info "$PETSONA_BINARY" >"$PETSONA_SMOKE_HOME/stdout.log" 2>"$PETSONA_SMOKE_HOME/stderr.log" &
PETSONA_PID=$!

for _ in {1..80}; do
  kill -0 "$PETSONA_PID" 2>/dev/null || fail 'Petsona 启动后退出'
  hook_status >/dev/null 2>&1 && break
  sleep 0.1
done
hook_status >/dev/null 2>&1 || fail 'test-hooks 服务未启动'

assert_hook_value processId "$PETSONA_PID" '进程和 test-hooks PID 一致'
assert_hook_value petVisible true '宠物默认可见'
assert_hook_value alwaysOnTop true '宠物默认置顶'
assert_hook_value clickThrough true '宠物默认启用像素穿透'
wait_for_hook nativeMenuReady true 3
pass 'macOS 原生菜单已创建'

# Keep the real desktop cursor from changing the result of unrelated protocol
# checks. The actual global-cursor path is a separate manual acceptance item.
hook_action '{"action":"cancel-gaze"}'
hook_action '{"action":"set-glance-side","value":0}'

polls_before="$(json_value "$(hook_status)" cursorPollCount)"
sleep 0.5
polls_after="$(json_value "$(hook_status)" cursorPollCount)"
if [[ "$polls_before" =~ ^[0-9]+$ && "$polls_after" =~ ^[0-9]+$ ]]; then
  poll_delta=$((polls_after - polls_before))
  (( poll_delta <= 10 )) || fail "macOS 全局指针轮询过于频繁（0.5 秒增加 ${poll_delta} 次）"
  pass 'macOS 全局指针保持低频轮询'
else
  fail '无法读取 macOS 全局指针轮询计数'
fi

wait_for_health ok true
health="$(state_health)"
[[ -n "$(json_value "$health" pet)" ]] || fail 'health 返回了空宠物名'
pass 'GET /health 返回宠物快照'
pets="$(state_pets)"
[[ -n "$(json_value "$pets" 0)" ]] || fail 'GET /pets 返回空列表'
pass 'GET /pets 返回宠物列表'

state_post '{"source":"macos-smoke","state":"waiting","message":"smoke","ttlMs":300}'
wait_for_hook state waiting
pass 'POST /state 切换 waiting'
sleep 0.45
wait_for_hook state idle
pass '状态 TTL 到期回到 idle'

hook_action '{"action":"open-settings"}'
wait_for_hook settingsOpen true
pass '设置窗口状态可控'
wait_for_hook settingsKeyWindow true 3
pass '设置窗口获得键盘焦点'
hook_action '{"action":"close-settings"}'
wait_for_hook settingsOpen false
pass '设置窗口可关闭'

hook_action '{"action":"set-scale","value":1.5}'
wait_for_hook scale 1.500000
pass '缩放设置可控'
hook_action '{"action":"set-click-through","enabled":false}'
wait_for_hook clickThrough false
pass '点击穿透可关闭'
hook_action '{"action":"set-click-through","enabled":true}'
wait_for_hook clickThrough true
pass '点击穿透可恢复'

hook_action '{"action":"show-bubble","text":"mac smoke","ttlMs":1000}'
wait_for_hook bubbleText 'mac smoke'
wait_for_hook bubbleWindowCreated true
pass '气泡文本可控'
hook_action '{"action":"clear-bubble"}'
wait_for_hook_empty bubbleText
wait_for_hook bubbleWindowCreated false
pass '气泡可清除'

hook_action '{"action":"hide-pet"}'
wait_for_hook petVisible false
pass '隐藏宠物'
hook_action '{"action":"show-pet"}'
wait_for_hook petVisible true
pass '恢复显示宠物'

hook_action '{"action":"save-config"}'
saved_scale=""
for _ in {1..50}; do
  saved_scale="$(json_value "$(<"$PETSONA_SMOKE_HOME/config.json")" window.scale)"
  [[ "$saved_scale" == "1.5" || "$saved_scale" == "1.500000" ]] && break
  sleep 0.05
done
[[ "$saved_scale" == "1.5" || "$saved_scale" == "1.500000" ]] || fail "配置没有保存缩放值（实际：${saved_scale:-<empty>}）"
pass '配置可以保存并被机器读取'

if [[ -n "$PETSONA_V2_PET_DIR" ]]; then
  hook_action '{"action":"start-gaze","value":1}'
  wait_for_hook state look-row-9
  wait_for_hook gazePhase holding 3
  gaze_sprite="$(json_value "$(hook_status)" spriteIndex)"
  sleep 0.25
  held_sprite="$(json_value "$(hook_status)" spriteIndex)"
  [[ "$gaze_sprite" == "$held_sprite" ]] || fail "持续注视时帧发生变化（$gaze_sprite -> $held_sprite）"
  pass '持续注视保持最强方向帧'

  hook_action '{"action":"set-glance-side","value":0}'
  wait_for_hook gazePhase returning
  pass '离开触发区进入返回阶段'
  wait_for_hook state idle 3
  wait_for_hook_empty gazePhase 3
  pass '返回阶段完成并恢复 idle'
else
  printf '[SKIP] 持续注视 app smoke（需要一个 V2 宠物；可设置 PETSONA_SMOKE_V2_PET_DIR）\n'
fi

hook_action '{"action":"quit"}'
for _ in {1..50}; do
  kill -0 "$PETSONA_PID" 2>/dev/null || break
  sleep 0.1
done
kill -0 "$PETSONA_PID" 2>/dev/null && fail '退出动作后进程仍在运行'
PETSONA_PID=""
pass '退出动作结束进程'

printf '\nmacOS smoke passed: %s checks\n' "$PETSONA_PASS_COUNT"
