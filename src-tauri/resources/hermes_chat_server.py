#!/usr/bin/env python3
"""
Hermes Chat HTTP Server — persistent Hermes AIAgent as a local HTTP service.

Starts on port 9120, exposes a chat endpoint that the Tauri frontend calls.
AIAgent stays alive in memory between requests — no cold start latency.

Endpoints:
    GET  /               — chat web UI page
    POST /api/chat       — send a message, get complete response
    POST /api/chat/stream — send a message, get SSE streaming response
    GET  /api/health     — health check (returns ready status)
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
from fastapi.responses import HTMLResponse, StreamingResponse
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
            request_overrides={"enable_search": True},
        )
    _agent_ready = True
    return _agent


# ── Schemas ────────────────────────────────────────────────────────────────


class ChatRequest(BaseModel):
    message: str
    session_id: str = ""


# ── Chat Web Page ──────────────────────────────────────────────────────────


CHAT_HTML = r"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Hermes Chat</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#1a1a2e;color:#e0e0e0;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;height:100vh;display:flex;flex-direction:column}
#messages{flex:1;overflow-y:auto;padding:20px;display:flex;flex-direction:column;gap:16px;scroll-behavior:smooth}
.msg{max-width:85%;padding:12px 16px;border-radius:12px;line-height:1.6;font-size:14px;white-space:pre-wrap;word-break:break-word}
.msg.user{background:#2d2d5e;align-self:flex-end;border-bottom-right-radius:4px}
.msg.assistant{background:#2a2a3e;align-self:flex-start;border-bottom-left-radius:4px}
.msg .label{font-size:11px;color:#888;margin-bottom:4px;font-weight:600}
.msg .content{color:#e0e0e0}
.msg .content code{background:#1a1a2e;padding:2px 6px;border-radius:4px;font-size:13px;color:#7ec8e3}
.msg .content pre{background:#111;padding:12px;border-radius:8px;overflow-x:auto;margin:8px 0}
.msg .content pre code{background:transparent;padding:0}
.msg .content strong{color:#f0f0f0}
.typing{color:#888;font-size:13px;padding:8px 16px;align-self:flex-start;animation:pulse 1.5s infinite}
@keyframes pulse{0%,100%{opacity:.4}50%{opacity:1}}
#input-area{display:flex;gap:8px;padding:16px 20px;background:#16162a;border-top:1px solid #2a2a4e}
#input{flex:1;background:#2a2a3e;border:1px solid #3a3a5e;border-radius:8px;padding:12px 16px;color:#e0e0e0;font-size:14px;resize:none;outline:none;min-height:48px;max-height:120px;font-family:inherit}
#input:focus{border-color:#6c63ff;box-shadow:0 0 0 2px rgba(108,99,255,.2)}
#send{background:#6c63ff;color:#fff;border:none;border-radius:8px;padding:0 24px;font-size:14px;cursor:pointer;font-weight:600;transition:background .2s}
#send:hover{background:#5b52e0}
#send:disabled{background:#3a3a5e;cursor:not-allowed}
.error{color:#ff6b6b;font-size:13px;padding:8px 16px;align-self:center}
</style>
</head>
<body>
<div id="messages"></div>
<div id="input-area">
<textarea id="input" placeholder="给 Hermes 发送消息..." rows="1"></textarea>
<button id="send">发送</button>
</div>
<script>
const messages=document.getElementById('messages');
const input=document.getElementById('input');
const sendBtn=document.getElementById('send');

function addMessage(role,text){
  const div=document.createElement('div');
  div.className='msg '+role;
  const label=document.createElement('div');
  label.className='label';
  label.textContent=role==='user'?'你':'Hermes';
  const content=document.createElement('div');
  content.className='content';
  content.textContent=text;
  div.appendChild(label);
  div.appendChild(content);
  messages.appendChild(div);
  messages.scrollTop=messages.scrollHeight;
}

function showTyping(){const d=document.createElement('div');d.className='typing';d.id='typing';d.textContent='Hermes 正在思考...';messages.appendChild(d);messages.scrollTop=messages.scrollHeight}
function hideTyping(){const t=document.getElementById('typing');if(t)t.remove()}
function showError(msg){const d=document.createElement('div');d.className='error';d.textContent=msg;messages.appendChild(d);messages.scrollTop=messages.scrollHeight}

function updateLastMessage(text){
  const msgs=messages.querySelectorAll('.msg.assistant');
  if(msgs.length>0){
    const content=msgs[msgs.length-1].querySelector('.content');
    if(content)content.textContent=text;
    messages.scrollTop=messages.scrollHeight;
  }
}

async function send(){
  const text=input.value.trim();
  if(!text||sendBtn.disabled)return;
  input.value='';
  input.style.height='auto';
  addMessage('user',text);
  showTyping();
  sendBtn.disabled=true;
  let full='';
  try{
    const resp=await fetch('/api/chat/stream',{
      method:'POST',
      headers:{'Content-Type':'application/json'},
      body:JSON.stringify({message:text,session_id:''})
    });
    if(!resp.ok){const e=await resp.text();throw new Error(e)}
    const reader=resp.body.getReader();
    const decoder=new TextDecoder();
    let buf='';
    while(true){
      const{done,value}=await reader.read();
      if(done)break;
      buf+=decoder.decode(value,{stream:true});
      const lines=buf.split('\n');
      buf=lines.pop()||'';
      for(const line of lines){
        if(!line.startsWith('data: '))continue;
        try{
          const data=JSON.parse(line.slice(6));
          if(data.type==='token'){full=data.text;hideTyping();addMessage('assistant','');updateLastMessage(full)}
          else if(data.type==='done'){hideTyping()}
          else if(data.type==='error'){hideTyping();showError(data.text)}
        }catch(e){}
      }
    }
  }catch(e){hideTyping();showError('请求失败: '+e.message)}
  finally{sendBtn.disabled=false;input.focus()}
}

sendBtn.addEventListener('click',send);
input.addEventListener('keydown',e=>{if(e.key==='Enter'&&!e.shiftKey){e.preventDefault();send()}});
input.addEventListener('input',()=>{input.style.height='auto';input.style.height=Math.min(input.scrollHeight,120)+'px'});
input.focus();
addMessage('assistant','你好！我是 Hermes AI 助手。你可以问我任何问题，我会搜索网络获取最新信息。');
</script>
</body>
</html>"""


@app.get("/")
async def chat_page():
    """Serve the chat web UI."""
    return HTMLResponse(CHAT_HTML)


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

    async def event_stream():
        collected_tokens = []

        def on_token(text: str):
            collected_tokens.append(text)

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
