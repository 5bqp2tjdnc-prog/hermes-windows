#!/usr/bin/env python3
"""
Hermes Chat HTTP Server — persistent Hermes AIAgent as a local HTTP service.

Starts on port 9120, exposes a chat endpoint that the Tauri frontend calls.
AIAgent stays alive in memory between requests — no cold start latency.

Endpoints:
    POST /api/chat        — send a message, get streaming SSE response
    GET  /api/health      — health check (returns ready status)
"""

import json
import os
import sys
import traceback
import logging
from contextlib import redirect_stdout
from pathlib import Path

# ── Bootstrap ──────────────────────────────────────────────────────────────

AGENT_DIR = os.environ.get("HERMES_AGENT_DIR", "")
if AGENT_DIR and AGENT_DIR not in sys.path:
    sys.path.insert(0, AGENT_DIR)

logging.basicConfig(stream=sys.stderr, level=logging.WARNING)
logger = logging.getLogger("hermes_chat_server")

# ── FastAPI Imports ────────────────────────────────────────────────────────

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import uvicorn

app = FastAPI(title="Hermes Chat")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

# ── Agent Instance (lazy initialized) ─────────────────────────────────────

_agent = None
_agent_ready = False


def get_agent():
    global _agent, _agent_ready
    if _agent is not None:
        return _agent
    with redirect_stdout(sys.stderr):
        from run_agent import AIAgent

        base_url = os.environ.get(
            "MINIMAX_BASE_URL", "https://api.minimaxi.com/v1"
        )
        api_key = os.environ.get("MINIMAX_API_KEY", "")
        model = os.environ.get("HERMES_MODEL", "MiniMax-M2.7-highspeed")

        _agent = AIAgent(
            base_url=base_url,
            api_key=api_key,
            model=model,
            quiet_mode=True,
            max_iterations=30,
            enabled_toolsets=["research", "web", "code"],
        )
    _agent_ready = True
    return _agent


# ── Schemas ────────────────────────────────────────────────────────────────


class ChatRequest(BaseModel):
    message: str
    session_id: str = ""


# ── Endpoints ──────────────────────────────────────────────────────────────


@app.get("/api/health")
async def health():
    """Health check — returns ready status."""
    return {"ready": _agent_ready}


@app.post("/api/chat")
async def chat(req: ChatRequest):
    """Send a chat message and receive the complete response."""
    try:
        agent = get_agent()
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Agent init failed: {e}")

    if not req.message.strip():
        raise HTTPException(status_code=400, detail="Empty message")

    try:
        response = agent.chat(req.message)
        return {"response": response, "session_id": req.session_id}
    except Exception as e:
        traceback.print_exc(file=sys.stderr)
        raise HTTPException(status_code=500, detail=str(e))


# ── Streaming endpoint ─────────────────────────────────────────────────────


@app.post("/api/chat/stream")
async def chat_stream(req: ChatRequest):
    """Send a chat message and receive streaming SSE response."""
    try:
        agent = get_agent()
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Agent init failed: {e}")

    if not req.message.strip():
        raise HTTPException(status_code=400, detail="Empty message")

    from fastapi.responses import StreamingResponse

    async def event_stream():
        collected_tokens = []

        def on_token(text: str):
            collected_tokens.append(text)
            # Cannot use async in a sync callback, but we're collecting
            # tokens to return at the end

        try:
            agent.chat(req.message, stream_callback=on_token)
            full_text = "".join(collected_tokens)
            yield f"data: {json.dumps({'type': 'token', 'text': full_text})}\n\n"
            yield f"data: {json.dumps({'type': 'done', 'session_id': req.session_id})}\n\n"
        except Exception as e:
            yield f"data: {json.dumps({'type': 'error', 'text': str(e)})}\n\n"

    return StreamingResponse(event_stream(), media_type="text/event-stream")


# ── Main ───────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    port = int(os.environ.get("HERMES_CHAT_PORT", "9120"))
    host = os.environ.get("HERMES_CHAT_HOST", "127.0.0.1")

    # Eager init so the first user request is fast
    print(f"Initializing Hermes AIAgent...", file=sys.stderr)
    try:
        agent = get_agent()
        print(f"Hermes AIAgent ready on {host}:{port}", file=sys.stderr)
    except Exception as e:
        print(f"Hermes AIAgent init failed: {e}", file=sys.stderr)
        sys.exit(1)

    uvicorn.run(
        app,
        host=host,
        port=port,
        log_level="warning",
        access_log=False,
    )
