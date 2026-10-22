# Consented desktop collaboration

The Linux Host advertises feature `0x20000000` for collaboration. An authenticated
guest joins the current owner's desktop only after that owner explicitly grants
view or interaction permission. Normal PAM/GSSAPI authentication and host access
policy still apply independently to every guest. A broker route is reachability,
never desktop consent.

## Identity and consent

The Host binds each request to the authenticated guest and current desktop
identity. Approval grants access to that desktop only. It cannot unlock a screen,
sign another user out, log into GDM, survive a desktop change, or confer the
owner's operating-system identity on the guest. Pending requests expire; silence
is refusal. Session grants and transport tokens are separate opaque secrets.

An owner sees the request in their existing desktop, including when that desktop
is streamed remotely. The owner chooses Decline, View only, or Allow interaction.
The Host's participant window remains available during sharing to revoke a
guest or change their permission. Guest input must be inhibited while the consent
or participant control UI is being used, so an existing controller cannot approve
another controller or change its own permission.

## Host HTTPS API

All routes require the guest's own authenticated Host session or the authenticated
desktop owner's session. Responses must not be cached. Request identifiers and
grants are not logged.

| Route | Purpose |
| --- | --- |
| `POST /plank/collaboration/request` | Request owner consent; returns an opaque `request_id` and `state`. |
| `GET /plank/collaboration/status?request_id=…` | The requesting guest polls its own request; approved replies include permission, a launch grant and current owner stream settings. |
| `GET /plank/collaboration/status` | Owner-only participant and pending-request status. |
| `POST /plank/collaboration/decision` | Owner-only decision for a request. |
| `POST /plank/collaboration/revoke` | Owner-only revocation. |
| `POST /plank/collaboration/cancel` | Cancel the caller's own request or guest participation. |

An approved guest supplies `plankCollaboration=<grant>` to `/launch`. The Host
revalidates the authenticated guest, owner, desktop and permission before creating
any guest media endpoint. A feature bit alone or a client-supplied permission is
never sufficient. Reconnecting a guest requires another approval.

## Display ownership

**The primary user's display layout is authoritative.** Joining, leaving,
resizing a guest window, or changing guest permissions must never modeset the
Host, replace its display lease, choose another primary output, or restore the
guest's bookmark layout on disconnect. Guest display-configuration requests are
not applied. Guests inherit the owner's existing capture geometry and arrange
their view locally. Encoding remains subject to the ordinary exact-format
negotiation; collaboration cannot silently substitute a video profile.

## Permissions and lifetime

View-only sessions cannot inject keyboard, pointer, pen or raw HID input. The
Host enforces this at input consumption, independently of client controls.
Interaction adds desktop keyboard, pointer and text input. Pen, clipboard, file
transfer, raw USB/HID, microphone and camera forwarding are unavailable to guests
in either mode; ordinary owner entitlements remain unchanged.

The reliable Host-to-Client `PLD1` control type 13 carries one network-order
`u32`: `0` revoked, `1` view-only, `2` interaction. It is valid only on a guest
connection negotiating `0x20000000`. The Host sends the effective permission
when it changes, including temporary view-only suspension while owner consent
controls are open. The Client updates its input gate and role indicator; the
Host's current permission remains the authority for every input packet.

Every guest has an independent authenticated transport connection. Dropping a
guest must not stop the owner or another guest, release the owner's display
lease, or lock their desktop. Ending the primary stream, locking/logging out the
desktop, replacing its worker, or losing the consent helper revokes sharing.
Queued input is rejected after revocation, and held guest keys/buttons are
released. A guest never keeps an otherwise abandoned desktop unlocked.

## Broker routing

The existing broker APIs remain sufficient. Busy hosts remain connectable to
assigned users, who authenticate to the Host and request consent there. Each WAN
user receives their own relay lease, including users behind the same public NAT.
Guest logout/lease expiry must close only that guest's route. The Host's shared
UDP listener dispatches connections by the unpredictable per-session token;
listener reuse must not depend on random `SO_REUSEPORT` routing.

## Acceptance

Behavioral tests must cover denial/expiry, wrong identity, stale desktop,
single-use grants, revocation, guest input gating and owner layout inheritance.
Transport loopbacks must cover concurrent clients on the same host port and peer
isolation when a guest stops. Broker tests cover distinct leases, direct grants,
HBAC and owner survival after guest logout. Real desktop acceptance additionally
requires the actual consent UI, two simultaneous clients, view-only input denial,
permission changes, owner disconnect/lock, and unchanged monitor geometry before
and after guest join/leave. Unit tests and package builds alone do not prove those
hardware/UI outcomes.
