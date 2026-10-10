# Sessions

`2026-09-09` · ideation one-pager · not yet a spec

## Problem Statement

How might we let a desk full of unrelated drawings become several named,
durable bodies of work — without asking the user to remember to file anything,
and without the agent putting a snapshot in the wrong drawer?

## Context

Today every delivery lands in one in-memory list on the bus. Two consequences,
both named as the real pain:

- **The history panel is one undifferentiated stream.** Yesterday's wireframe
  sits next to this morning's whiteboard photo with nothing between them.
- **Everything is lost on restart.** `SketchBus` holds `Vec<Delivery>` and
  never trims it, so the images live in RAM for exactly as long as the app
  runs, then vanish. (Two bugs for the price of one: that list is also an
  unbounded memory leak in a long-running session.)

`FUTURE.md` already anticipates this under _Cross-session history_. This
one-pager is that item growing up.

## On the word "session"

The chosen term is **session**, not project — "project" collides with the
Norday sense of a client engagement. The word is already in use in three
places, and they have to be told apart:

1. **This run of the app.** What the agent-facing strings mean today: "List
   every delivery sent this session", "No deliveries yet this session".
2. **The MCP protocol session.** `rmcp`'s `LocalSessionManager`, one per
   connected host. A library type, internal, never shown to a user.
3. **The new meaning:** a named, durable body of desk work.

Meaning 1 is the only real conflict, and it resolves in an unusually happy
way. Once deliveries persist and `list_deliveries` scopes to the current
session, "every delivery sent this session" stays literally true — it just
starts meaning something more useful. The tool descriptions sharpen rather
than change. Meaning 2 stays internal and needs no rename; meaning 3 takes
over the user-facing vocabulary.

The one hard consequence: the dead prototype directory in the app data dir is
_itself_ called `sessions/` (a Gemini cost ledger and generated frames from
the removed pipeline). That is now a direct path collision, not just clutter.
It has to go before anything writes there.

## Three directions considered

### A. Persist everything, group later

No session concept. Every delivery is written to disk; the history panel
groups by date and searches the OCR text that is already captured. Solves
loss-on-restart entirely and clutter partially, with zero new vocabulary.

- **User value:** high on persistence, medium on clutter. Date is a weak proxy
  for "a piece of work" — a single afternoon holds three topics.
- **Feasibility:** the cheapest. `Delivery` already derives `Serialize` and
  only `jpeg` is `#[serde(skip)]`, so the sidecar is close to free.
- **Differentiation:** none. It is table stakes, not a feature.

### B. One current session, agent-named, with adoption (recommended)

There is _always_ exactly one current session — never a mode the user can
forget to enter. It opens itself on the first delivery with a provisional name
("Tue 9 Sep, 14:20"). The agent calls `start_session("Kiosk redesign")`, which
does one of two things:

- the current session is still **provisional** → it is _named_, adopting the
  deliveries already in it;
- the current session is already **named and non-empty** → a new one opens.

That single rule is what makes the two orders of work both correct. Sketch
first and explain later, and the drawings you already made get adopted into
the name. Open the chat first and draw after, and the session is named before
anything arrives. A gap of several hours closes a provisional session so
adoption can never reach back into yesterday.

- **User value:** high. It is the only direction that removes clutter _and_
  requires no filing discipline from the user.
- **Feasibility:** medium. Needs a persistence layer (direction A's work), one
  mutating MCP tool, and session management UI. The bus doc comment holds a
  useful invariant — _the MCP server never touches settings or the filesystem_
  — so writing belongs on the bus side and the server keeps only reading.
- **Differentiation:** real. The agent naming the user's physical work,
  retroactively, is a thing a folder tree cannot do.

### C. Sessions as agent-visible memory

B plus `list_sessions` and `search_deliveries(query, session?)`, so Claude can
reach back into last week's session and pull the drawing out by its recognised
text. This is where sessions stop being a filing cabinet and start paying off.

- **User value:** potentially the highest of the three, but only _after_ a
  corpus exists. On day one there is nothing to search.
- **Feasibility:** low extra cost on top of B, provided the storage layout is
  designed for it now.
- **Verdict:** correct destination, wrong starting point. Build B so that C is
  a later addition and not a migration.

## Recommended Direction

**B, built on A's spine, with the storage layout chosen so C is additive.**

Concretely: land persistence first, because B without it is pointless and A
without B is unsatisfying. One directory per session under the app data dir,
one file per snapshot, one JSON sidecar per delivery derived from the struct
that already serializes. Then the always-a-current-session rule, then the
`start_session` tool, then the management UI.

The single most important design decision is the adoption rule. It converts
the mode error ("the agent forgot to open a session") from a silent
mis-filing into a recoverable, expected case.

## Key Assumptions to Validate

- [ ] **Claude actually calls `start_session`.** Nothing forces it. Test: run
      a week of real work with the tool description as drafted, and count the
      deliveries that end up in a provisional session. If most do, the tool
      description is wrong or the auto-naming has to get smarter.
- [ ] **Adoption matches intent.** When work is sketched before the chat
      opens, does naming the provisional session capture the right set? Test
      against the last two weeks of your own desk work.
- [ ] **Writing desk photos to disk is acceptable.** This is the assumption
      that could kill the feature, and it is not technical. Today the camera
      watches your desk and the images are _ephemeral_. Persisting them is a
      material change to the app's privacy posture, for an app whose whole
      pitch is that nothing leaves the machine. Validate that colleagues are
      comfortable, that the storage location is visible in the UI, and that
      delete means the bytes are gone.
- [ ] **Disk growth stays boring.** Snapshots are capped at 1280 px and
      encoded at quality 0.75, so roughly 200 KB each. A heavy day of 50
      deliveries at two images each is about 20 MB, so a few hundred MB per
      month. Tolerable, but only if the size is visible and deletion is easy.

## MVP Scope

**In:**

- Deliveries persist: one folder per session, JPEG per snapshot, JSON sidecar
  per delivery from the existing `Serialize` impl.
- Exactly one current session at all times; auto-opened and provisionally
  named on the first delivery.
- `start_session(name)` over MCP, with the adopt-or-open rule and a time-gap
  fence that closes a stale provisional session.
- App UI: switch, rename, delete a session. Delete asks for confirmation and
  really removes the files. History filters to the current session.
- Visible storage location and total size.
- Remove the dead prototype `sessions/` directory before writing to that path,
  since the name now collides directly.
- Re-read the four agent-facing "this session" strings in `mcp_server.rs` and
  the two bus comments so they carry the new meaning unambiguously.

**Out of the MVP:** everything in the next section.

## Not Doing (and Why)

- **Cross-session search and `list_sessions`.** The payoff of direction C, but
  it needs a corpus to be worth anything. Design the layout for it, ship it
  second.
- **Moving individual deliveries between sessions.** The adoption rule should
  make mis-filing rare. If it turns out not to be, this is the first thing to
  add — do not build the escape hatch before knowing the trap exists.
- **Retention or auto-pruning.** Show the size, let the user delete. An
  automatic deleter of the user's own drawings needs more trust than a v1 has
  earned.
- **Export, sharing, sync between machines, encryption at rest.** Each is a
  separate privacy posture with its own decisions. A plain inspectable folder
  keeps the door open to all of them.
- **Per-session camera settings.** Cameras are a property of the desk, not of
  the work.
- **A session concept in the MCP App card.** The card shows one delivery; it
  has no reason to know about filing.

## Open Questions

- What closes a provisional session — a fixed time gap, the app quitting, or
  the MCP client disconnecting? The protocol session is available via
  `LocalSessionManager`, which makes per-conversation boundaries cheap, but a
  reconnect must not fragment one piece of work into three sessions.
- `start_session` would be the first mutating MCP tool. Every existing tool is
  `read_only_hint = true`. Does the agent get to _delete_ a session too, or is
  destruction strictly the user's? (Instinct: strictly the user's.)
- What happens to an open collection when a session boundary lands mid-way?
- Do session names have to be unique, and what happens on a repeat name —
  reopen the existing session, or make a second one?
