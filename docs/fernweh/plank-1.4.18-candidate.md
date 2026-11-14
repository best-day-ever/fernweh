# PLANK 1.4.18 candidate — unfinished

Do not merge this branch wholesale. Current private main and its integration
ledger record the focused repairs already accepted. This branch preserves the
larger upstream merge for continued compatibility work, rather than replacing
working Fernweh paths during that work.

Before promotion:

- Bring current main's independently qualified input cleanup, pen checks and
  focused backports into the candidate without duplicating their tests.
- Reconcile the Qt frontend with saved monitor selection, primary/crops,
  physical/virtual backing and screen-change Apply/Keep behavior.
- Wire file/text clipboard paste and completion through the Qt owner, plus
  participant views and permission changes. Existing SDL event callbacks are
  insufficient for the Qt runtime.
- Preserve and build the Windows frontend. Keeping old files alone does not
  establish compatible decoder interfaces or a working runtime.
- Reconcile optional metrics defaults, startup validation and configuration
  plumbing; complete the Host installer filesystem gates.
- Keep exact profile selection and Fernweh's pacing cap while qualifying the
  Native10/dependency changes with native source-first FEC, encrypted burst and
  unchanged loss/performance thresholds.
- Recheck both optional-broker Client profiles and all relevant platform gates.
  The local C ABI is version15 in this candidate, because Fernweh retains setup
  admission/file-clipboard fields absent from upstream's version14 layout.

Investigation evidence: the combined input library passes8 tests; the Rust1.99.1
candidate passes88 unit,6 wire and11 encrypted functional tests. A Mac Qt developer
candidate compiled with the new dependencies. Its application-flow and Windows
parity are still unresolved. The native Linux candidate dependency set was
prepared, but its full build stopped at an offline crate-cache miss. None of
these results qualify the whole candidate or authorize a deployment.

The inherited README/review receipt at this branch's older base may describe
superseded preparation state. Read current main's ledger for accepted changes.
