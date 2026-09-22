@AGENTS.md
@docs/bde/agents.md

## Claude Code specifics
- MemPalace tools are `mcp__mempalace__mempalace_*` (project `.mcp.json`, enabled in `.claude/settings.json`;
  read tools and diary_write are pre-allowed, add_drawer/kg_add/kg_invalidate ask, delete/mine/sync are not allowed). If they're missing, the forward on
  127.0.0.1:8765 is down: use `scripts/agent/memory` or carry on and backfill.
- Subagents and workflow agents inherit this file and the MCP connection. Tell them to use `scripts/agent/graph` to
  orient, and to return facts rather than writing memory themselves.
- `HANDOFF.md` is long and upstream's; read the sections you need, not the whole file.
