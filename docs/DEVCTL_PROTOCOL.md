# rust-ui.devctl/0.1

Opt-in local Windows pipe protocol. CLI semantics and examples live in
[DEVCTL.md](DEVCTL.md); this document defines the wire boundary.

## Frames and ownership

A connection carries one request and one response. Frame: little-endian u32 byte
length, then exactly that many UTF-8 JSON bytes. Maximum 4 MiB each, validated
before allocation. Truncated, malformed, unknown request fields and oversized
frames fail. Overlapped reads/writes have bounded frame deadlines; cancellation
joins native I/O before releasing its buffer/OVERLAPPED. The idle pipe listener
blocks on connection + shutdown events with no periodic timeout.

The endpoint is `\\.\pipe\rust-ui-devctl-<64 hex session id>`, one instance,
`PIPE_REJECT_REMOTE_CLIENTS`. The session file includes protocol, session_id,
independent 64-hex token, owned child pid, endpoint and initial window metadata.
Credentials come from launch environment; they are never part of comparable
snapshots/capture metadata. OS handle teardown removes the pipe; stale receipts
are explicitly removed through cleanup. One serialized command per connection;
the UI handoff queue is bounded to 16 requests. No second UI executor exists.
A request permit serializes queue insertion with the checked Mailbox wake; a
failed wake leaves that request rejected, incapable of later UI execution.
The pipe worker signals the established Mailbox wake authority; all UI mutations
enter the existing guarded Backend turn, followed by reentrant drain.

## Request

```json
{"protocol":"rust-ui.devctl/0.1","token":"<session token>","id":1,"command":"rect","arguments":{"id":"probe.button","timeout_ms":4750}}
```

`id` is a u64 echoed unchanged (scoped to that request/connection). Command string
is bounded to 64 UTF-8 bytes. Arguments are an object (or null for no arguments).
The client sets timeout_ms from its command deadline, bounded to 1..30000 ms.
Requests that expire before UI admission do not execute.

Runtime commands:

| Command | Arguments / result |
|---|---|
| handshake | PID, HWND diagnostic identity, protocol, scale, client_dp/client_px |
| tree / snapshot-layout | Snapshot schema in DEVCTL.md, deterministic retained child order |
| rect | `id`; client-relative logical/physical rectangle + visible/enabled |
| wait-idle | Observed quiescence + presentation barrier; bounded timeout |
| hover / press / release / click | `id`; same native pointer/event/state route |
| focus | `id`, optional `modality: keyboard` |
| key | `key`; documented bounded unmodified subset through backend normalization |
| shutdown | Successful acknowledgement followed by mailbox-woken guarded window destruction |

Launch/cleanup, WGC screenshot/cropping and the comparators are local CLI actions;
they are not filesystem/process/capture commands exposed to a remote endpoint.
No remote transport, unrestricted native message command or arbitrary execution
command exists in the protocol.

## Response

```json
{"protocol":"rust-ui.devctl/0.1","id":1,"ok":true,"result":{"id":"probe.button","coordinate_space":"client-relative","scale_factor":1.25,"rect_dp":{"x":53,"y":83.62109375,"width":69.4609375,"height":36},"rect_px":{"x":66,"y":105,"width":87,"height":45},"visible":true,"enabled":true}}
```

Failure has `ok:false,error:{code,message,details?}` and no result. Success has
result and no error. The CLI returns `{ok,result}` / `{ok,error}` without leaking
the token. An over-budget response is replaced by a small MESSAGE_TOO_LARGE error.
The server waits for client EOF after writing a response; a missing completion
within one second is a transport failure. It does not report shutdown delivery
success after that failure or immediately discard still-buffered response bytes.

Stable codes: INVALID_ARGUMENT, TARGET_NOT_FOUND, TARGET_NOT_VISIBLE,
SESSION_NOT_FOUND, SESSION_CLOSED, TIMEOUT, NOT_SUPPORTED, CAPTURE_FAILED,
COMPARE_MISMATCH (CLI comparison exit class), PROTOCOL_MISMATCH, PROTOCOL_ERROR,
MESSAGE_TOO_LARGE, DUPLICATE_ID, BUSY, IO_ERROR, UI_ERROR.
Timeout `details` includes last UI mailbox/events/reentrant/deferred work,
layout/paint dirtiness, damage, pending paint, pump continuation and deadline flags.
No automatic mutation retries occur. A response deadline is not a transactional
rollback of an operation already executing.

The listener rejects unauthorized/malformed clients before UI admission and
recreates its instance for subsequent clients. Listener creation failure at startup
uses the owned run/bail cleanup contract; a later recreation failure is delivered
through the same UI wake authority as a typed terminal runtime failure. Shutdown
detaches/drains pending replies, signals stop, cancels/joins I/O and joins the worker
before HWND/model resources disappear. Capture belongs to the CLI process, with
its own process-lifetime WinRT initialization and per-capture RAII resources.
