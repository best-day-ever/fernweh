# Desktop collaboration admission

The Linux Host advertises feature `0x20000000` for collaboration. An authenticated
guest joins the current owner's desktop after that owner explicitly grants
view or interaction permission, or an explicit Host policy authorizes it.
Normal PAM/GSSAPI authentication and host access
policy still apply independently to every guest. A broker route is reachability,
never desktop authorization.

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

## Automatic joining for authorized workstation users

Set `collaboration_require_approval = false` in `[collaboration]` to allow every
user who passes the Host's existing authentication and workstation access policy
to join the current unlocked desktop with interaction. The default is `true`.
This policy needs no group membership and never accepts a client claim of access.
A broker route alone is insufficient: the guest must have their own authenticated
Host session before requesting or claiming a grant.

Automatic grants use the same bounded, desktop-bound, one-use path described
below. Owner denial, downgrade, revocation and helper closure do not block them.
The participant indicator describes authorized access without identifying ordinary
users as administrators. Locks and all other desktop lifecycle revocations remain.
A policy change back to requiring approval is checked at status and launch;
already active streams end through the ordinary desktop/disconnect lifecycle.
The existing `support: true` wire field denotes an owner-approval exemption under
either automatic joining or the optional support group; it does not assert an
administrator identity. There are at most four automatically authorized guests.

## Optional administrator support override

An administrator may configure `collaboration_support_group` in the Host's
`[collaboration]` section. It defaults to empty (disabled). The Host resolves
membership through NSS/SSSD using the authenticated account UID. Missing groups,
failed lookups and nonmembers do not authorize support. Client JSON, usernames
and feature flags cannot confer this role. PAM/GSSAPI and workstation access
policy still apply; this is solely an exception to owner consent.

Members receive a desktop-bound interaction grant without waiting for approval.
The Host rechecks membership on guest status and at launch. The grant is still
single-use and expires after two minutes if unused. Owner deny/revoke/downgrade
controls cannot change a support grant. The local indicator labels it as
authorized access and exposes no permission buttons for it. A missing or
closed helper revokes ordinary guests but does not block support. The Host keeps
trying to start the indicator; it also logs successful support joins by UID,
without grant secrets. Input remains suspended while owner security controls
are open, including during other guests' pending consent.

Support remains bound to the unlocked desktop: lock, logout, generation change,
primary stream termination and guest cancellation revoke it and release held
input. Guest clipboard, files, pen, devices, microphone and camera remain disabled.
The owner's capture and layout stay authoritative. Four separate support slots
prevent ordinary pending/active guests from exhausting support admission; support
is also bounded to four concurrent identities. Capture/transport resources and
matching encoder requirements still apply.

Membership changes affect new requests and launches through refreshed NSS/SSSD
state; an already streaming support grant is not continuously reauthenticated.
To end it immediately, disconnect the support stream or lock/end the desktop.

The wire negotiation is unchanged (`0x20000000`). Request creation retains its
`pending` acknowledgement for existing clients; the next status response returns
`control` with the existing grant/capture fields. Owner/helper participant records
add `support: true` for approval-exempt grants; absent/false means ordinary consent.

## Host HTTPS API

All routes require the guest's own authenticated Host session or the authenticated
desktop owner's session. Responses must not be cached. Request identifiers and
grants are not logged.

| Route | Purpose |
| --- | --- |
| `POST /plank/collaboration/request` | Request admission under Host policy; returns an opaque `request_id` and `state`. |
| `GET /plank/collaboration/status?request_id=…` | The requesting guest polls its own request; approved replies include permission, a launch grant and current owner stream settings. |
| `GET /plank/collaboration/status` | Owner-only participant and pending-request status. |
| `POST /plank/collaboration/decision` | Owner-only decision for a request. |
| `POST /plank/collaboration/revoke` | Owner-only revocation. |
| `POST /plank/collaboration/cancel` | Cancel the caller's own request or guest participation. |

An approved guest supplies `plankCollaboration=<grant>` to `/launch`. The Host
revalidates the authenticated guest, owner, desktop and permission before creating
any guest media endpoint. A feature bit alone or a client-supplied permission is
never sufficient. Reconnecting an ordinary guest requires another approval; support
requires a fresh policy check.

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
desktop, replacing its worker, or losing the consent helper revokes ordinary
sharing. Support is exempt only from helper loss; all desktop lifecycle
revocations still apply.
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
