# Clipboard files v2

Status: implemented for the Mac Client and Linux X11 Host behind explicit
default-off negotiation. Paired desktop release qualification is pending.

## Scope

Clipboard files v2 carries a platform-neutral file manifest and requested file
ranges between the active Mac Client and the authenticated Linux X11 desktop
worker. It supports regular files and directories in both directions. It does
not carry native pasteboard data, absolute source paths, cut/delete authority,
symlinks, special files, ACLs, extended attributes or data across sessions.

## Capability and authority

The launch feature is `ClipboardFilesFeature = 0x1000000`. The implementation
moved past `0x400000`, which is allocated to desktop sign-out, and requires text
clipboard feature `0x800000` so that both peers share its session epoch,
ownership and revocation machinery.

Negotiation yields one effective mode: `off`, `client-to-host`,
`host-to-client`, or `bidirectional`. Host policy is authoritative. A file
message received before negotiation, in a prohibited direction, at LoginWindow,
or outside the active authenticated desktop worker is rejected.

The feature creates two reliable DataProtocol endpoints on the existing QUIC
connection:

| Endpoint | Purpose | Maximum record |
|---|---|---:|
| 8 | control, manifest and range requests | 64 KiB |
| 10 | file range responses | 256 KiB data plus header |

Control and content are separate from input, Host events and ordinary session
data. Peers that do not negotiate the feature do not register these endpoints.

## Encoding

All integers are unsigned little-endian. Every record begins with this fixed
56-byte header:

| Field | Type | Rule |
|---|---|---|
| magic | 4 bytes | ASCII `PLF2` |
| version | u16 | exactly 2 |
| type | u16 | one known message type |
| flags | u32 | unknown bits rejected |
| payload size | u32 | exact remaining record length |
| offer ID | 16 bytes | cryptographically random, nonzero clipboard offer |
| session epoch | u64 | exact active epoch |
| sequence | u64 | nonzero, strictly increasing per direction |
| reserved | u64 | zero |

Decoders validate the complete header and declared payload before allocating or
reading a variable field. A malformed control record terminates the affected
file subsystem and transfer. It does not reinterpret bytes as another version.

Strings are a u16 byte length followed by scalar-valid UTF-8. They contain no
NUL. A manifest name is exactly one relative component and is subject to the
filesystem rules in the implementation plan.

## Control messages

| Type | Direction | Purpose |
|---|---|---|
| `OfferBegin` | source -> sink | Direction, clipboard generation, top-level count, declared entry count/bytes, policy snapshot |
| `OfferEntry` | source -> sink | One manifest entry |
| `OfferEnd` | source -> sink | Completes the manifest stream |
| `OfferAck` | sink -> source | Accepts the exact manifest and advertises request window/chunk size |
| `Materialize` | sink -> source | Locks a new paste operation for some or all top-level entries |
| `ReadRequest` | sink -> source | Requests one sequential range of a regular file |
| `EntryComplete` | source -> sink | Confirms final size, stable-source check and SHA-256 |
| `TransferReady` | sink -> source | One destination materialization is atomically complete |
| `Revoke` | either | Source clipboard changed; no new materialization may start |
| `Cancel` | either | Stops one offer or materialization with a stable reason code |
| `Error` | either | Rejects a message, offer or materialization with a stable reason code |

`OfferBegin` starts a streaming manifest. Exactly the declared number of
`OfferEntry` records must appear before `OfferEnd`; entries cannot be replaced
or appended after Ack. A sink validates incrementally but accepts only at
`OfferAck`. Rejection discards the entire offer.

`OfferAck` does not by itself pin source files indefinitely. A `Materialize`
record supplies a nonzero 64-bit materialization ID, a bounded list of top-level
entry IDs (or the explicit all-items flag), and locks those sources through one
terminal result. IDs are unique within an offer. Source clipboard replacement
revokes the offer for new materializations but allows already-started ones to
finish. Session loss revokes both the offer and all materializations.

An `OfferEntry` contains:

| Field | Type | Rule |
|---|---|---|
| entry ID | u64 | nonzero and unique in the offer |
| parent ID | u64 | zero for top-level, otherwise an earlier directory |
| kind | u8 | `regular` or `directory` |
| portable mode | u16 | user rwx bits only; all other bits zero |
| size | u64 | zero for directories |
| mtime ns | i64 payload value | signed Unix nanoseconds, range checked |
| name | UTF-8 component | 1..255 bytes |

Although the wire slot for mtime is signed, it is decoded as an explicit signed
64-bit value rather than through the unsigned record helpers. Receivers reject
timestamps outside the destination filesystem's supported range instead of
wrapping them.

## Content messages

`ReadRequest` names a live materialization, one regular entry, an offset and a
length from 1 through 256 KiB. Requests for an entry outside that
materialization, directories, data beyond the declared size, overlapping
ranges, a second range before the previous range for that entry completes, or
more than the negotiated outstanding window are rejected. v2 requests ranges
sequentially for each file and permits at most two files in progress.

Its fixed payload is `materialization ID: u64`, `entry ID: u64`, `offset: u64`,
`length: u32`, `reserved: u32`. Reserved is zero. A `Data` payload has the same
prefix, replacing requested length with exact data length, followed by those
bytes. `EntryComplete` contains `materialization ID: u64`, `entry ID: u64`,
`size: u64` and 32 SHA-256 bytes.

Endpoint 10 returns `Data` records with the same offer ID, materialization ID,
epoch, entry ID, offset and exact bytes. No Data record is valid before
`Materialize` and a matching `ReadRequest`. The aggregate
accepted-but-not-written content is bounded to 8 MiB. Flow control cannot be
implemented by unbounded application queues.

After the last byte, the source rechecks stable file metadata and sends
`EntryComplete` with the declared size and SHA-256 calculated over the served
byte sequence. The sink compares its own byte count and digest, fsyncs the
partial file, applies safe metadata, and atomically renames it. A mismatch or
source change fails the whole transfer. Empty files receive `EntryComplete`
without a range request.

Directories are created privately during staging and become visible only with
the materialization root's final atomic rename. `TransferReady` is sent only
after all requested entries are complete and any required native clipboard
publication has succeeded. An AppKit promise that writes directly to the
application-provided destination needs no second clipboard publication.

## State machine

```text
NO_OFFER
  -> OFFERING (OfferBegin, OfferEntry*, OfferEnd)
  -> OFFER_AVAILABLE (OfferAck)

Each paste under OFFER_AVAILABLE:
  -> MATERIALIZING (Materialize)
  -> TRANSFERRING (ReadRequest <-> Data, EntryComplete)*
  -> COMMITTING
  -> READY (TransferReady)

Offer -> REVOKED (Revoke, source clipboard replacement)
Materialization -> CANCELLED (Cancel, policy/session loss)
Materialization -> FAILED (Error, malformed data, I/O/integrity failure)
```

Only one transition is valid for each incoming message. Terminal
materializations ignore duplicate terminal messages and reject other late traffic.
Materialization IDs are never reusable within an offer, offer IDs are never
reusable within a process, and a new authenticated session starts with empty
offer, materialization and sequence state.

## Clipboard generations and leases

Clipboard generation is local to the offering peer and independent in each
direction. A newer generation revokes the older offer for new materializations.
Already-started materializations may finish. Equal file names or bytes do not
suppress a new clipboard generation because ownership changed.

The source registry is a lease, not a transferable path capability. It maps
entry IDs to source handles and stable metadata only inside the authenticated
worker. The peer cannot provide a path to read. `Revoke` or source clipboard
replacement prevents new `Materialize` records. Each accepted `Materialize`
pins only its selected entries through one terminal result; later clipboard
replacement does not abort it. Session loss always closes the registry and all
materializations. Already completed destination files follow the destination's
bounded staging cleanup policy; incomplete files are deleted.

## Errors

Stable reason classes are:

- `unsupported`
- `policy-denied`
- `offer-replaced`
- `limit-exceeded`
- `invalid-manifest`
- `unsupported-entry-type`
- `source-unavailable`
- `source-changed`
- `destination-unavailable`
- `no-space`
- `integrity-failed`
- `cancelled-by-user`
- `session-ended`
- `protocol-error`
- `internal-error`

Wire errors carry a reason class and optional numeric detail, never a source or
destination path. User-facing platform code may associate an error with an
already validated manifest entry and display that entry's name locally.

## Compatibility

- A v1-only peer continues text sync and never registers endpoints 8 or 10.
- A v2 peer with file policy `off` behaves identically to v1.
- File records are never tunneled through v1 clipboard chunks.
- Unknown v2 message types, flags or entry kinds are rejected, not skipped.
- Adding symlinks, metadata streams, compression, cross-session resume or a new
  platform requires separately negotiated capability bits or a later version.
