#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "fastmcp==3.0.0b1",
# ]
# ///

from __future__ import annotations

# pyright: reportMissingImports=false

import json
import logging
import os
import shutil
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Literal, NoReturn

from fastmcp import Context, FastMCP
from fastmcp.exceptions import ToolError
from fastmcp.server.lifespan import lifespan

logger = logging.getLogger(__name__)

DEFAULT_DAEMON_URL = "http://localhost:19277"
KNOWN_CLIENTS: dict[str, str] = {
    "claude-code": "claude",
    "claude": "claude",
    "opencode": "opencode",
}
_VALID_BACKENDS = frozenset(KNOWN_CLIENTS.values())

_SPAWN_TOOL_BASE_DESCRIPTION = (
    "Spawn a new teammate in a tmux pane. The teammate receives its initial "
    "prompt via inbox and begins working autonomously. Names must be unique "
    "within the team. cwd must be an absolute path to the teammate's working directory."
)


def discover_harness_binary(name: str) -> str | None:
    return shutil.which(name)


def discover_opencode_models(opencode_binary: str) -> list[str]:
    try:
        result = subprocess.run(
            [opencode_binary, "models", "--refresh"],
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        if result.returncode != 0:
            return []
        lines = result.stdout.strip().splitlines()
        if len(lines) <= 1:
            return []
        return [line.strip() for line in lines[1:] if line.strip()]
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        return []


def list_opencode_agents(server_url: str) -> list[dict[str, str]]:
    try:
        data = _get("/agent")
    except DaemonAPIError:
        return []
    if not isinstance(data, list):
        return []
    internal = {"title", "summary", "compaction"}
    out: list[dict[str, str]] = []
    for row in data:
        if not isinstance(row, dict):
            continue
        name = row.get("name")
        desc = row.get("description")
        if (
            isinstance(name, str)
            and isinstance(desc, str)
            and name not in internal
            and desc
        ):
            out.append({"name": name, "description": desc})
    return out


def _parse_backends_env(raw: str) -> list[str]:
    if not raw:
        return []
    return list(
        dict.fromkeys(
            b.strip()
            for b in raw.split(",")
            if b.strip() and b.strip() in _VALID_BACKENDS
        )
    )


def _build_spawn_description(
    claude_binary: str | None,
    opencode_binary: str | None,
    opencode_models: list[str],
    opencode_server_url: str | None,
    opencode_agents: list[dict[str, str]],
    enabled_backends: list[str] | None = None,
) -> str:
    parts = [_SPAWN_TOOL_BASE_DESCRIPTION]
    backends: list[str] = []
    show_claude = claude_binary is not None
    show_opencode = opencode_binary is not None and opencode_server_url is not None
    if enabled_backends is not None:
        show_claude = show_claude and "claude" in enabled_backends
        show_opencode = show_opencode and "opencode" in enabled_backends
    if show_claude:
        backends.append("'claude' (default, models: sonnet, opus, haiku)")
    if show_opencode:
        model_list = (
            ", ".join(opencode_models) if opencode_models else "none discovered"
        )
        backends.append(f"'opencode' (models: {model_list})")
    if backends:
        parts.append(f"Available backends: {'; '.join(backends)}.")
    if show_opencode and opencode_agents:
        agent_lines = [f"  - {a['name']}: {a['description']}" for a in opencode_agents]
        parts.append(
            "Available opencode agents (pass as subagent_type when backend_type='opencode'):\n"
            + "\n".join(agent_lines)
        )
    return " ".join(parts)


def _update_spawn_tool(
    tool: Any,
    enabled: list[str],
    claude_binary: str | None,
    opencode_binary: str | None,
    opencode_models: list[str],
    opencode_server_url: str | None,
    opencode_agents: list[dict[str, str]],
) -> None:
    props = tool.parameters.get("properties", {})
    backend_prop = props.get("backend_type")
    if isinstance(backend_prop, dict):
        backend_prop["enum"] = list(enabled)
        if enabled:
            backend_prop["default"] = enabled[0]
    tool.description = _build_spawn_description(
        claude_binary,
        opencode_binary,
        opencode_models,
        opencode_server_url,
        opencode_agents,
        enabled_backends=enabled,
    )


@lifespan
async def app_lifespan(server: Any):
    del server
    claude_binary = discover_harness_binary("claude")
    opencode_binary = discover_harness_binary("opencode")
    opencode_server_url = os.environ.get("OPENCODE_SERVER_URL")
    opencode_models: list[str] = []
    if opencode_binary:
        opencode_models = discover_opencode_models(opencode_binary)
    opencode_agents: list[dict[str, str]] = []
    if opencode_server_url:
        opencode_agents = list_opencode_agents(opencode_server_url.rstrip("/"))

    enabled_backends = _parse_backends_env(os.environ.get("CLAUDE_TEAMS_BACKENDS", ""))
    if "opencode" in enabled_backends and not opencode_server_url:
        enabled_backends.remove("opencode")
    if not enabled_backends:
        if claude_binary:
            enabled_backends.append("claude")
        if opencode_binary and opencode_server_url:
            enabled_backends.append("opencode")

    spawn_tool = await mcp.get_tool("spawn_teammate")
    _update_spawn_tool(
        spawn_tool,
        enabled_backends,
        claude_binary,
        opencode_binary,
        opencode_models,
        opencode_server_url,
        opencode_agents,
    )
    yield {
        "claude_binary": claude_binary,
        "opencode_binary": opencode_binary,
        "opencode_models": opencode_models,
        "opencode_agents": opencode_agents,
        "enabled_backends": enabled_backends,
    }


mcp = FastMCP(
    name="claude-teams",
    instructions=(
        "MCP server for orchestrating Claude Code agent teams. "
        "Manages team creation, teammate spawning, messaging, and task tracking."
    ),
    lifespan=app_lifespan,
)


class DaemonAPIError(Exception):
    pass


def _daemon_url() -> str:
    return os.environ.get("OPENCODE_SERVER_URL", DEFAULT_DAEMON_URL).rstrip("/")


def _request(method: str, path: str, body: dict | None = None) -> Any:
    url = f"{_daemon_url()}{path}"
    headers = {"Content-Type": "application/json"}
    data = json.dumps(body).encode("utf-8") if body is not None else None
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=20) as resp:
            raw = resp.read()
    except urllib.error.HTTPError as e:
        raw = b""
        try:
            raw = e.read()
        except Exception:
            pass
        detail = raw.decode("utf-8", errors="replace")[:1000]
        raise DaemonAPIError(
            f"Daemon request failed ({e.code}) for {path}: {detail or e.reason}"
        )
    except urllib.error.URLError as e:
        raise DaemonAPIError(f"Cannot reach daemon at {url}: {e.reason}")

    if not raw:
        return {"ok": True}

    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        text = raw.decode("utf-8", errors="replace")
        raise DaemonAPIError(f"Daemon returned invalid JSON for {path}: {text[:200]}")


def _post(path: str, body: dict[str, Any]) -> dict[str, Any]:
    out = _request("POST", path, body)
    if isinstance(out, dict):
        return out
    return {"data": out}


def _get(path: str, params: dict[str, Any] | None = None) -> Any:
    if params:
        qp = {k: v for k, v in params.items() if v is not None}
        if qp:
            path = f"{path}?{urllib.parse.urlencode(qp)}"
    return _request("GET", path)


def _project_cwd() -> str:
    try:
        data = _get("/project/absolute_path")
        if isinstance(data, dict) and isinstance(data.get("path"), str):
            return data["path"]
    except DaemonAPIError:
        logger.warning("Falling back to process cwd for team lead cwd")
    return os.getcwd()


def _next_external_task_id(team_name: str) -> str:
    try:
        snap = _get(f"/teams/{team_name}/snapshot")
        if not isinstance(snap, dict):
            return str(int(time.time() * 1000))
        team = snap.get("team") if isinstance(snap.get("team"), dict) else {}
        tasks = team.get("tasks") if isinstance(team, dict) else []
        max_id = 0
        if isinstance(tasks, list):
            for task in tasks:
                if not isinstance(task, dict):
                    continue
                raw = task.get("id")
                if isinstance(raw, str) and raw.isdigit():
                    max_id = max(max_id, int(raw))
        if max_id > 0:
            return str(max_id + 1)
    except DaemonAPIError:
        pass
    return str(int(time.time() * 1000))


def _raise_tool(err: Exception) -> NoReturn:
    raise ToolError(str(err))


@mcp.tool
def team_create(team_name: str, description: str = "") -> dict:
    """Create a new agent team. Sets up team config and task directories under ~/.claude/.
    One team per server session. Team names must be filesystem-safe
    (letters, numbers, hyphens, underscores)."""
    try:
        return _post(
            "/internal/teams/create",
            {
                "name": team_name,
                "description": description,
                "leadName": "team-lead",
                "leadAgentType": "team-lead",
                "model": "claude-sonnet-4-6",
                "backendType": "claude",
                "cwd": _project_cwd(),
            },
        )
    except DaemonAPIError as e:
        _raise_tool(e)


@mcp.tool
def team_delete(team_name: str) -> dict:
    """Delete a team and all its data. Fails if any teammates are still active.
    Removes both team config and task directories."""
    try:
        return _post("/internal/teams/delete", {"name": team_name})
    except DaemonAPIError as e:
        _raise_tool(e)


@mcp.tool(name="spawn_teammate")
def spawn_teammate_tool(
    team_name: str,
    name: str,
    prompt: str,
    cwd: str,
    ctx: Context,
    model: str = "sonnet",
    subagent_type: str = "general-purpose",
    plan_mode_required: bool = False,
    backend_type: Literal["claude", "opencode"] = "claude",
) -> dict:
    """Spawn a new teammate in a tmux pane. The teammate receives its initial
    prompt via inbox and begins working autonomously. Names must be unique
    within the team. cwd must be an absolute path to the teammate's working directory.
    Available backends: 'claude' (default, models: sonnet, opus, haiku); 'opencode' (models depend on runtime).
    Available opencode agents (pass as subagent_type when backend_type='opencode'):
    - team-manager: Project manager that orchestrates work across parallel workers
    - build: The default agent. Executes tools based on configured permissions.
    - plan: Plan mode. Disallows all edit tools.
    - general: General-purpose agent for researching complex questions and executing multi-step tasks.
    - explore: Fast agent specialized for exploring codebases."""
    if not os.path.isabs(cwd):
        raise ToolError("cwd is required and must be an absolute path")
    enabled = ctx.lifespan_context.get("enabled_backends", [])
    if enabled and backend_type not in enabled:
        raise ToolError(f"Backend {backend_type!r} is not enabled. Enabled: {enabled}")
    try:
        return _post(
            "/internal/teammates/spawn",
            {
                "team": team_name,
                "name": name,
                "agentType": subagent_type,
                "model": model,
                "backendType": backend_type,
                "cwd": cwd,
                "planModeRequired": plan_mode_required,
                # Kept for parity with tool signature even if daemon ignores it.
                "prompt": prompt,
            },
        )
    except DaemonAPIError as e:
        _raise_tool(e)


@mcp.tool
def send_message(
    team_name: str,
    type: Literal[
        "message",
        "broadcast",
        "shutdown_request",
        "shutdown_response",
        "plan_approval_response",
    ],
    recipient: str = "",
    content: str = "",
    summary: str = "",
    request_id: str = "",
    approve: bool | None = None,
    sender: str = "team-lead",
) -> dict:
    """Send a message to a teammate or respond to a protocol request.
    Type 'message' sends a direct message (requires recipient, summary).
    Type 'broadcast' sends to all teammates (requires summary).
    Type 'shutdown_request' asks a teammate to shut down (requires recipient; content used as reason).
    Type 'shutdown_response' responds to a shutdown request (requires sender, request_id, approve).
    Type 'plan_approval_response' responds to a plan approval request (requires recipient, request_id, approve)."""
    if type == "message" and not recipient:
        raise ToolError("recipient is required for type='message'")
    to_name = None if type == "broadcast" else (recipient or None)

    payload: dict[str, Any] | None = None
    if request_id or approve is not None:
        payload = {}
        if request_id:
            payload["requestId"] = request_id
        if approve is not None:
            payload["approve"] = approve

    try:
        return _post(
            "/internal/messages/send",
            {
                "team": team_name,
                "fromName": sender,
                "toName": to_name,
                "messageType": type,
                "summary": summary or None,
                "contentText": content or None,
                "payloadJson": payload,
            },
        )
    except DaemonAPIError as e:
        _raise_tool(e)


@mcp.tool
def task_create(
    team_name: str,
    subject: str,
    description: str,
    active_form: str = "",
    metadata: dict | None = None,
) -> dict:
    """Create a new task for the team. Tasks are auto-assigned incrementing IDs.
    Optional metadata dict is stored alongside the task."""
    external_id = _next_external_task_id(team_name)
    try:
        result = _post(
            "/internal/tasks/create",
            {
                "team": team_name,
                "externalTaskId": external_id,
                "subject": subject,
                "description": description,
                "activeForm": active_form,
                "status": "pending",
            },
        )
    except DaemonAPIError as e:
        _raise_tool(e)

    if metadata is not None:
        try:
            _post(
                "/internal/tasks/update",
                {
                    "team": team_name,
                    "externalTaskId": external_id,
                    "metadataJson": metadata,
                },
            )
        except DaemonAPIError:
            pass

    out = dict(result)
    out["id"] = external_id
    return out


@mcp.tool
def task_update(
    team_name: str,
    task_id: str,
    status: Literal["pending", "in_progress", "completed", "deleted"] | None = None,
    owner: str | None = None,
    subject: str | None = None,
    description: str | None = None,
    active_form: str | None = None,
    add_blocks: list[str] | None = None,
    add_blocked_by: list[str] | None = None,
    metadata: dict | None = None,
) -> dict:
    """Update a task's fields. Setting owner auto-notifies the assignee via
    inbox. Setting status to 'deleted' removes the task file from disk.
    Metadata keys are merged into existing metadata (set a key to null to delete it)."""
    body: dict[str, Any] = {
        "team": team_name,
        "externalTaskId": task_id,
    }
    if status is not None:
        body["status"] = status
    if owner is not None:
        body["ownerName"] = owner
    if subject is not None:
        body["subject"] = subject
    if description is not None:
        body["description"] = description
    if active_form is not None:
        body["activeForm"] = active_form
    if metadata is not None:
        body["metadataJson"] = metadata
    if add_blocks:
        body["addBlocks"] = add_blocks
    if add_blocked_by:
        body["addBlockedBy"] = add_blocked_by

    try:
        result = _post("/internal/tasks/update", body)
        out = dict(result)
        out["id"] = task_id
        return out
    except DaemonAPIError as e:
        _raise_tool(e)


@mcp.tool
def task_list(team_name: str) -> list[dict]:
    """List all tasks for a team with their current status and assignments."""
    try:
        snap = _get(f"/teams/{team_name}/snapshot")
    except DaemonAPIError as e:
        _raise_tool(e)
    if not isinstance(snap, dict):
        return []
    team = snap.get("team")
    if not isinstance(team, dict):
        return []
    tasks = team.get("tasks")
    if isinstance(tasks, list):
        return [t for t in tasks if isinstance(t, dict)]
    return []


@mcp.tool
def task_get(team_name: str, task_id: str) -> dict:
    """Get full details of a specific task by ID."""
    all_tasks = task_list(team_name)
    for t in all_tasks:
        if str(t.get("id", "")) == task_id:
            return t
    raise ToolError(f"Task {task_id!r} not found in team {team_name!r}")


@mcp.tool
def read_inbox(
    team_name: str,
    agent_name: str,
    unread_only: bool = True,
    mark_as_read: bool = True,
) -> list[dict]:
    """Read messages from an agent's inbox. Returns unread messages by default
    and marks them as read. NOTE: As team-lead, prefer check_teammate to read
    messages from a specific teammate. check_teammate filters by sender and
    provides richer status."""
    if not unread_only:
        logger.info("read_inbox unread_only=false ignored by daemon API")
    if not mark_as_read:
        logger.info("read_inbox mark_as_read=false ignored by daemon API")
    try:
        _post(
            "/internal/inbox/read",
            {
                "team": team_name,
                "memberName": agent_name,
            },
        )
        return []
    except DaemonAPIError as e:
        _raise_tool(e)


@mcp.tool
def read_config(team_name: str) -> dict:
    """Read the current team configuration including all members."""
    try:
        snap = _get(f"/teams/{team_name}/snapshot")
    except DaemonAPIError as e:
        _raise_tool(e)
    if not isinstance(snap, dict):
        raise ToolError(f"Unexpected response for team {team_name!r}")
    team = snap.get("team")
    if not isinstance(team, dict):
        raise ToolError(f"Team {team_name!r} not found")
    return team


@mcp.tool
def force_kill_teammate(team_name: str, agent_name: str) -> dict:
    """Forcibly kill a teammate's tmux target. Use when graceful shutdown via
    send_message(type='shutdown_request') is not possible or not responding.
    Kills the tmux pane/window, removes member from config, and resets their tasks."""
    try:
        return _post(
            "/internal/teammates/kill",
            {
                "team": team_name,
                "name": agent_name,
                "reason": "force_kill_teammate",
            },
        )
    except DaemonAPIError as e:
        _raise_tool(e)


@mcp.tool
def process_shutdown_approved(team_name: str, agent_name: str) -> dict:
    """Process a teammate's shutdown by removing them from config and resetting
    their tasks. Call this after confirming shutdown_approved in the lead inbox."""
    try:
        return _post(
            "/internal/teammates/kill",
            {
                "team": team_name,
                "name": agent_name,
                "reason": "shutdown_approved",
            },
        )
    except DaemonAPIError as e:
        _raise_tool(e)


@mcp.tool
def check_teammate(
    team_name: str,
    agent_name: str,
    include_output: bool = False,
    output_lines: int = 20,
    include_messages: bool = True,
    max_messages: int = 5,
    notify_after_minutes: int | None = None,
) -> dict:
    """Check a single teammate's status: alive/dead, unread messages from them,
    their unread count, and optionally terminal output. Always non-blocking.
    Use parallel calls to check multiple teammates. Push notifications may be
    available in this session. Use notify_after_minutes to schedule a deferred reminder."""
    del output_lines, include_messages, max_messages, notify_after_minutes
    try:
        snap = _get(f"/teams/{team_name}/snapshot")
        timeline = _get(
            f"/teams/{team_name}/members/{agent_name}/timeline",
            params={"limit": 1},
        )
    except DaemonAPIError as e:
        _raise_tool(e)

    statuses = []
    if isinstance(snap, dict) and isinstance(snap.get("statuses"), list):
        statuses = snap["statuses"]

    match = None
    for status in statuses:
        if isinstance(status, dict) and status.get("memberName") == agent_name:
            match = status
            break

    if match is None:
        raise ToolError(f"Teammate {agent_name!r} not found in team {team_name!r}")

    error = match.get("lastError")
    if not error and isinstance(timeline, list) and timeline:
        first = timeline[0]
        if isinstance(first, dict):
            error = first.get("headline")

    result = {
        "name": agent_name,
        "alive": bool(match.get("alive", False)),
        "pending_from": [],
        "their_unread_count": 0,
        "error": error,
        "notification_scheduled": False,
        "push_available": False,
    }
    if include_output:
        result["output"] = ""
    return result


def main() -> None:
    logging.basicConfig(
        level=logging.INFO, format="%(levelname)s %(name)s: %(message)s"
    )
    mcp.run()


if __name__ == "__main__":
    main()
