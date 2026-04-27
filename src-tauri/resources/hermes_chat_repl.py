#!/usr/bin/env python3
"""
Hermes Chat REPL — persistent Hermes AIAgent process.

Reads JSON queries from stdin, streams JSON responses to stdout.
One process stays alive for the entire Tauri app lifetime,
eliminating Python cold start latency on every chat message.

Protocol (stdin, line-delimited JSON):
    {"message": "user query", "session_id": ""}

Protocol (stdout, prefix-delimited lines):
    __READY__                          (sent once after agent init)
    __TOKEN__{"text": "Hello"}         (streaming token)
    __DONE__{"session_id": "abc123"}   (response complete)
    __ERROR__error message             (error occurred)
"""

import json
import os
import sys
import traceback
from contextlib import redirect_stdout

# ── Bootstrap ──────────────────────────────────────────────────────────────

AGENT_DIR = os.environ.get("HERMES_AGENT_DIR", "")
if AGENT_DIR and AGENT_DIR not in sys.path:
    sys.path.insert(0, AGENT_DIR)

# Redirect Hermes internal logs to stderr so they don't pollute our protocol
import logging

logging.basicConfig(stream=sys.stderr, level=logging.WARNING)

# ── Helpers ────────────────────────────────────────────────────────────────


def emit_token(text: str) -> None:
    """Send a streaming token to stdout."""
    sys.stdout.write(f'__TOKEN__{json.dumps({"text": text})}\n')
    sys.stdout.flush()


def emit_done(session_id: str = "") -> None:
    """Signal that a response is complete."""
    sys.stdout.write(f'__DONE__{json.dumps({"session_id": session_id})}\n')
    sys.stdout.flush()


def emit_error(msg: str) -> None:
    """Report an error."""
    sys.stdout.write(f"__ERROR__{msg}\n")
    sys.stdout.flush()


# ── Agent Initialization ───────────────────────────────────────────────────

def init_agent():
    """Create and return a configured AIAgent instance."""
    from run_agent import AIAgent

    base_url = os.environ.get(
        "MINIMAX_BASE_URL",
        "https://api.minimaxi.com/v1",
    )
    api_key = os.environ.get("MINIMAX_API_KEY", "")
    model = os.environ.get("HERMES_MODEL", "MiniMax-M2.7-highspeed")

    return AIAgent(
        base_url=base_url,
        api_key=api_key,
        model=model,
        quiet_mode=True,
        max_iterations=30,
        enabled_toolsets=["research", "web", "code"],
        request_overrides={"enable_search": True},
    )


# ── Main Loop ──────────────────────────────────────────────────────────────

def main():
    # Redirect all Hermes import/init print() noise to stderr
    with redirect_stdout(sys.stderr):
        agent = init_agent()
    sys.stdout.write("__READY__\n")
    sys.stdout.flush()

    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        if line == "__EXIT__":
            break

        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            emit_error(f"invalid JSON: {e}")
            continue

        message = req.get("message", "").strip()
        session_id = req.get("session_id", "")
        if not message:
            emit_error("empty message")
            continue

        try:
            response = agent.chat(
                message,
                stream_callback=emit_token,
            )
            emit_done(session_id)
        except Exception as e:
            traceback.print_exc(file=sys.stderr)
            emit_error(str(e))


if __name__ == "__main__":
    main()
