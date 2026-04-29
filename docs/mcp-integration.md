# MCP Integration — Use Attune as a Knowledge Source from Any MCP Client

> Status: stable for v0.6 (W4 deliverable, 2026-04-27).
> Approach: thin stdio shim wrapping Attune's existing REST API. Per strategy decision
> we do not bundle our own MCP server crate — we wrap with the smallest possible adapter
> so any MCP-compatible client (Claude Desktop, Cursor, Continue, Cherry Studio, LobeChat,
> open-webui, or your own scripts) can call Attune.

## What you get

Once configured, your MCP client can:

- `attune_search(query: str, top_k: int = 10)` — hybrid (BM25 + vector) search across your
  Attune vault. Returns ranked items with title, snippet, citations.
- `attune_get_item(id: str)` — fetch the full content of one item.
- `attune_chat(prompt: str)` — RAG-grounded chat answer with citations from your vault.

All calls hit your **local** Attune server (default `http://localhost:18900`). Your vault
stays on your machine; the MCP client only sees the search/chat results.

## Quick start (Claude Desktop)

1. Make sure Attune server is running and your vault is unlocked:
   ```bash
   cd rust && cargo run --bin attune-server
   curl http://localhost:18900/api/v1/status   # should return unlocked: true
   ```

2. Drop `tools/attune_mcp_shim.py` (shipped in this repo) somewhere stable — for example
   `~/.local/share/attune/attune_mcp_shim.py`.

3. Edit `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or
   `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

   ```json
   {
     "mcpServers": {
       "attune": {
         "command": "python3",
         "args": ["/absolute/path/to/attune_mcp_shim.py"],
         "env": {
           "ATTUNE_BASE_URL": "http://localhost:18900",
           "ATTUNE_API_TOKEN": "your-token-here"
         }
       }
     }
   }
   ```

4. Restart Claude Desktop. In a new chat, ask: `@attune search "rust ownership"` —
   you should see your vault hits flow into the answer.

## Quick start (Cursor)

Cursor reads MCP from `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "attune": {
      "command": "python3",
      "args": ["/absolute/path/to/attune_mcp_shim.py"]
    }
  }
}
```

Then in Cursor: `@attune` triggers the tool list; pick `attune_search`.

## Quick start (Continue / Cherry Studio / LobeChat / open-webui)

These clients all support the MCP stdio transport with the same JSON shape — just point
`command + args` at `attune_mcp_shim.py`. See each client's docs for the exact config file
location.

## How the shim works

`tools/attune_mcp_shim.py` is ~120 lines of Python. It speaks MCP over stdio (`stdin/stdout`
JSON-RPC 2.0) and translates each tool call into a `requests.get/post` against your local
Attune REST API. No persistent state, no caching, no server logic — Attune does all the
work.

This means:
- ✅ Zero Rust dependency added — shim works with any Python ≥3.9
- ✅ Hot-swap MCP protocol versions by editing one file
- ✅ Add new tools (e.g. `attune_create_item`) by adding ~10 lines per tool
- ⚠️ One Python process per MCP client connection (negligible for personal use)

## Configuration reference

| Env var | Default | Purpose |
|---------|---------|---------|
| `ATTUNE_BASE_URL` | `http://localhost:18900` | Where the server listens |
| `ATTUNE_API_TOKEN` | _(empty)_ | If you set a server token, paste it here |
| `ATTUNE_TIMEOUT_SEC` | `30` | Per-call HTTP timeout |
| `ATTUNE_DEBUG` | _(unset)_ | Set to `1` for verbose stderr logs |

## Authentication

The shim sends `Authorization: Bearer $ATTUNE_API_TOKEN` on every request. If your server
runs without auth (development), leave the env var empty.

## Privacy boundary

Per Attune's "private AI knowledge companion" promise:
- Your vault never leaves your machine — even when used via MCP, the MCP client only sees
  the **results** of your search/chat, not the raw vault.
- The shim does not log queries (set `ATTUNE_DEBUG=1` if you want them on stderr for
  debugging — never in production).
- All Attune-side encryption (DEK + AES-256-GCM) still applies; the shim is just a translator.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| `Connection refused` | Server not running | `cd rust && cargo run --bin attune-server` |
| `403 vault locked or unavailable` | Vault not unlocked | Open Attune desktop or call `/api/v1/vault/unlock` |
| MCP client doesn't show "attune" | Bad path in config | Use absolute path; check JSON syntax |
| Empty results | Vault empty / wrong query | Call `/api/v1/items` directly to verify content |

## Versioning policy

- v0.6: search / get_item / chat tools (this document).
- v0.7: add `attune_list_recent`, `attune_create_note` for write operations.
- The shim's exposed tool names are stable across minor versions; renames will be batched
  with major version bumps and listed in `RELEASE.md`.

## Why this design (not a Rust MCP crate)

Per the strategy plan v4: "C2 用 gpt-researcher 现成 MCP server，不要自研" — the value
is in Attune's vault + RAG quality, not in re-implementing the MCP protocol. A 120-line
Python shim:
- Lets the MCP ecosystem evolve without rebuilding Attune
- Keeps Attune's binary lean (no extra Rust crate, no transitive deps)
- Demonstrates that any HTTP backend can become MCP-callable in an hour

If you need a Rust-native MCP server (e.g. for embedded deployments where Python is
unavailable), file an issue — `attune-mcp-server` is on the v0.8 roadmap.

## References

- Strategy plan v4 (W4 C2 decision): see `docs/v0.6-release-readiness.md`
- MCP protocol spec: <https://modelcontextprotocol.io/>
- gpt-researcher MCP example: <https://github.com/assafelovic/gpt-researcher>
- Attune REST API: see `rust/RELEASE.md` for endpoint inventory
