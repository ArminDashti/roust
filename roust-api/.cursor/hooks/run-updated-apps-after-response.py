#!/usr/bin/env python3
"""Project hook: restart this app's API + WebUI after an agent response that edited it.

afterFileEdit: record when files under this pair (*-api / *-webui) change.
afterAgentResponse: start Run-ListedApps.ps1 -Name <stem> (detached), then clear.

State is shared under the *-api project's .cursor/hooks/state so a multi-root
workspace with the same hook in both repos does not lose edit tracking.

Fail-open: always emit {} and exit 0.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from datetime import datetime
from pathlib import Path


HOOKS_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = HOOKS_DIR.parent.parent

RUNNER = (
    Path.home()
    / ".cursor"
    / "plugins"
    / "local"
    / "devops-by-armin"
    / "skills"
    / "run-all-apps-locally"
    / "scripts"
    / "Run-ListedApps.ps1"
)

LEAF_SUFFIXES = (
    ("-api", "api"),
    ("-webui", "webui"),
    ("-web", "webui"),
    ("-ui", "webui"),
)

DEBOUNCE_SECONDS = 45


def stem_from_leaf(leaf: str) -> tuple[str, str] | None:
    lower = leaf.lower()
    for suffix, side in LEAF_SUFFIXES:
        if lower.endswith(suffix) and len(leaf) > len(suffix):
            return leaf[: -len(suffix)].lower(), side
    return None


def this_app_stem() -> str | None:
    parsed = stem_from_leaf(PROJECT_ROOT.name)
    return parsed[0] if parsed else None


def shared_hooks_dir() -> Path:
    """Prefer <stem>-api/.cursor/hooks so api+webui share one state file."""
    stem = this_app_stem()
    if stem:
        api_hooks = PROJECT_ROOT.parent / f"{stem}-api" / ".cursor" / "hooks"
        if api_hooks.is_dir() or PROJECT_ROOT.name.lower().endswith("-api"):
            target = PROJECT_ROOT.parent / f"{stem}-api" / ".cursor" / "hooks"
            target.mkdir(parents=True, exist_ok=True)
            (target / "state").mkdir(parents=True, exist_ok=True)
            return target
    STATE = HOOKS_DIR / "state"
    STATE.mkdir(parents=True, exist_ok=True)
    return HOOKS_DIR


SHARED = shared_hooks_dir()
STATE_DIR = SHARED / "state"
STATE_PATH = STATE_DIR / "run-updated-apps.json"
DEBUG_LOG = SHARED / "hook-debug.log"


def debug(message: str) -> None:
    try:
        stamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with DEBUG_LOG.open("a", encoding="utf-8") as handle:
            handle.write(
                f"[{stamp}] run-updated-apps ({PROJECT_ROOT.name}): {message}\n"
            )
    except OSError:
        pass


def emit_ok() -> None:
    sys.stdout.write("{}")
    sys.stdout.flush()


def read_input() -> dict:
    try:
        if hasattr(sys.stdin, "reconfigure"):
            sys.stdin.reconfigure(encoding="utf-8-sig", errors="replace")
        raw = sys.stdin.read()
    except Exception as exc:
        debug(f"stdin read failed: {exc}")
        return {}

    if not raw or not raw.strip():
        debug("stdin was empty")
        return {}

    if raw.startswith("\ufeff"):
        raw = raw.lstrip("\ufeff")

    try:
        data = json.loads(raw)
    except json.JSONDecodeError as exc:
        debug(f"json parse failed: {exc}; raw={raw[:500]!r}")
        return {}

    return data if isinstance(data, dict) else {}


def normalize_root(raw: str) -> Path | None:
    value = raw.strip()
    if not value:
        return None

    if value.startswith("file://"):
        value = value[7:]

    if len(value) >= 3 and value[0] == "/" and value[2] == ":":
        value = value[1:]

    value = value.replace("\\", "/")
    path = Path(value)
    try:
        return path.resolve()
    except OSError:
        return path


def pair_roots(stem: str) -> list[Path]:
    parent = PROJECT_ROOT.parent
    roots: list[Path] = []
    for suffix in ("-api", "-webui", "-web", "-ui"):
        candidate = parent / f"{stem}{suffix}"
        if candidate.is_dir():
            roots.append(candidate)
    if PROJECT_ROOT not in roots:
        roots.append(PROJECT_ROOT)
    return roots


def path_belongs_to_pair(path: Path, stem: str) -> bool:
    try:
        resolved = path.resolve()
    except OSError:
        resolved = path

    for root in pair_roots(stem):
        try:
            resolved.relative_to(root.resolve())
            return True
        except (ValueError, OSError):
            continue
    return False


def load_state() -> dict:
    if not STATE_PATH.is_file():
        return {"conversations": {}}
    try:
        data = json.loads(STATE_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        debug(f"state load failed: {exc}")
        return {"conversations": {}}
    if not isinstance(data, dict):
        return {"conversations": {}}
    if not isinstance(data.get("conversations"), dict):
        data["conversations"] = {}
    return data


def save_state(data: dict) -> None:
    try:
        STATE_DIR.mkdir(parents=True, exist_ok=True)
        STATE_PATH.write_text(
            json.dumps(data, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    except OSError as exc:
        debug(f"state save failed: {exc}")


def conversation_key(data: dict) -> str:
    for key in ("conversation_id", "session_id", "generation_id"):
        value = data.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return "default"


def handle_after_file_edit(data: dict) -> None:
    stem = this_app_stem()
    if not stem:
        debug(f"project name not an api/webui pair: {PROJECT_ROOT.name}")
        return

    file_path = data.get("file_path") or data.get("path") or ""
    if not isinstance(file_path, str) or not file_path.strip():
        debug("afterFileEdit missing file_path")
        return

    normalized = file_path.replace("\\", "/").lower()
    skip_parts = (
        "/.cursor/hooks/",
        "/.git/",
        "/node_modules/",
        "/target/",
        "/dist/",
        "/.run-listed",
    )
    if any(part in normalized for part in skip_parts):
        debug(f"afterFileEdit skip meta path {file_path!r}")
        return

    if not path_belongs_to_pair(Path(file_path), stem):
        debug(f"afterFileEdit outside pair stem={stem} path={file_path!r}")
        return

    key = conversation_key(data)
    state = load_state()
    conversations = state.setdefault("conversations", {})
    entry = conversations.setdefault(key, {"stems": [], "updated_at": None})
    stems = set(entry.get("stems") or [])
    stems.add(stem)
    entry["stems"] = sorted(stems)
    entry["updated_at"] = datetime.now().isoformat(timespec="seconds")
    save_state(state)
    debug(f"afterFileEdit recorded stem={stem} conversation={key}")


def recently_started(names: list[str], now: float) -> bool:
    state = load_state()
    last = state.get("last_start")
    if not isinstance(last, dict):
        return False
    started_at = last.get("at")
    started_names = last.get("names")
    if not isinstance(started_at, (int, float)) or not isinstance(started_names, list):
        return False
    if now - float(started_at) > DEBOUNCE_SECONDS:
        return False
    return set(str(n).lower() for n in started_names) == set(names)


def start_apps(names: list[str]) -> None:
    if not RUNNER.is_file():
        debug(f"runner missing: {RUNNER}")
        return

    log_path = SHARED / "run-updated-apps.log"
    args = [
        "powershell.exe",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        str(RUNNER),
        "-Name",
        *names,
    ]

    creationflags = 0
    if os.name == "nt":
        creationflags = 0x00000008 | 0x00000200 | 0x08000000

    try:
        with log_path.open("a", encoding="utf-8") as log_handle:
            stamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            log_handle.write(
                f"\n[{stamp}] ({PROJECT_ROOT.name}) starting: {' '.join(names)}\n"
            )
            log_handle.flush()
            subprocess.Popen(
                args,
                stdout=log_handle,
                stderr=subprocess.STDOUT,
                stdin=subprocess.DEVNULL,
                cwd=str(RUNNER.parent),
                creationflags=creationflags,
                close_fds=True,
            )
        debug(f"started Run-ListedApps for {names}")
    except OSError as exc:
        debug(f"Popen failed: {exc}")


def handle_after_agent_response(data: dict) -> None:
    stem = this_app_stem()
    if not stem:
        debug(f"project name not an api/webui pair: {PROJECT_ROOT.name}")
        return

    key = conversation_key(data)
    state = load_state()
    conversations = state.setdefault("conversations", {})
    entry = conversations.get(key) or {}
    stems = [str(s).lower() for s in (entry.get("stems") or []) if str(s).strip()]

    if stem not in stems:
        debug("afterAgentResponse no edited stems for this pair; skip")
        return

    names = [stem]
    now = datetime.now().timestamp()
    if recently_started(names, now):
        debug(f"afterAgentResponse debounce skip names={names}")
        return

    start_apps(names)
    state["last_start"] = {"at": now, "names": names}
    if key in conversations:
        remaining = [s for s in stems if s != stem]
        if remaining:
            conversations[key] = {
                "stems": remaining,
                "updated_at": datetime.now().isoformat(timespec="seconds"),
            }
        else:
            del conversations[key]
    save_state(state)


def main() -> int:
    data = read_input()
    event = str(data.get("hook_event_name") or "").strip()
    if not event and len(sys.argv) > 1:
        event = sys.argv[1].strip()

    if not event:
        if "file_path" in data or "edits" in data:
            event = "afterFileEdit"
        elif "text" in data:
            event = "afterAgentResponse"

    event_lower = event.lower()
    try:
        if event_lower in {"afterfileedit", "after_file_edit"}:
            handle_after_file_edit(data)
        elif event_lower in {
            "afteragentresponse",
            "after_agent_response",
            "agentresponse",
        }:
            handle_after_agent_response(data)
        else:
            debug(f"ignored event={event!r} keys={sorted(data.keys())}")
    except Exception as exc:
        debug(f"unhandled error: {exc}")

    emit_ok()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
