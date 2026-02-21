#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "fastmcp==3.0.0b1",
#   "filelock==3.16.0",
# ]
# ///

# pyright: reportMissingImports=false

from pathlib import Path
import os
import sys

DEFAULT_LOCAL_SRC = Path("__NIGHTSHIFT_CLAUDE_TEAMS_SRC__")


def main() -> None:
    env_src = os.environ.get("NIGHTSHIFT_CLAUDE_TEAMS_SRC")
    if env_src:
        local_src = Path(env_src).expanduser()
    elif str(DEFAULT_LOCAL_SRC).startswith("__NIGHTSHIFT_"):
        local_src = (
            Path(__file__).resolve().parent.parent.parent
            / "claude-code-teams-mcp"
            / "src"
        )
    else:
        local_src = DEFAULT_LOCAL_SRC
    if not local_src.exists():
        raise RuntimeError(
            f"Local claude-code-teams-mcp src not found at {local_src}. "
            "Set NIGHTSHIFT_CLAUDE_TEAMS_SRC to the repo src path."
        )
    sys.path.insert(0, str(local_src))
    from claude_teams.server import main as server_main

    server_main()


if __name__ == "__main__":
    main()
