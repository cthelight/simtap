# simtap

A standalone, **faithful emulation of a Linux `st`-driver SCSI tape drive**, exposed to
userspace over **FUSE**. It lets you drive a virtual tape drive with the ordinary tools —
`mt`, `dd`, `tar` — unmodified, on a machine with no tape hardware.

---

## Premise

Testing anything that talks to a tape drive is painful without hardware. The behavior that
matters — filemarks, end-of-medium, multi-volume `tar`, rewind-on-close, offline/load, the
exact `mt status` flags — is precisely the behavior that is hard to reproduce or unit-test.

On Linux, a SCSI tape drive is presented to userspace by the kernel `st` driver through two
character devices and a small set of ioctls:

| Device | Behavior on `close()` |
|--------|----------------------|
| `/dev/st0`  | **rewinds** the tape |
| `/dev/nst0` | leaves the head where it is ("non-rewinding") |

Both expose the same `mtioctl(2)` surface (`MTIOCTOP`, `MTIOCGET`, `MTIOCPOS`) and the same
`MT_*` operation codes (`MTREW`, `MTFSF`, `MTWEOF`, `MTEOM`, …) and `GMT_*` status bits that
`mt` and `dd`/`tar` rely on.

**simtap reimplements that exact contract in userspace.** It mounts a FUSE directory that
contains the two device files (`tape` and `nst`). Every read, write, and ioctl is dispatched
to a shared drive state that mirrors the `st` driver's per-partition bookkeeping, so the
status output, error codes, and filemark/EOD state machine match what a real `st`-driven SCSI
tape would produce. The medium itself is just a pair of ordinary files, so data **persists
across eject/insert** and runs are reproducible.

It is a general-purpose testing rig for anything that drives tapes through the `st`
contract — backup and archival tools, `tar --multi-volume` EOM/roll flows, custom
`mt`/`dd` pipelines — on a hardware-less box.

### What "faithful" means here

- `st` rewinds on close, `nst` does not (driver `rew_at_close`).
- Closing after a write auto-writes a filemark (`st_flush`), including the `dd` dup-fd pattern.
- Reading across a filemark is two steps (data, then a 0-byte mark completion), exactly like
  the driver's `ST_FM_HIT` / `ST_FM` transition.
- Reading past the last filemark settles through the blank-tape state machine and then fails
  with `-EIO`, like a real drive.
- A write past the medium's capacity fails with `-ENOSPC` and flags end-of-data.
- `mt offline` / `rewoffl` / `eject` take the medium offline (`GMT_DR_OPEN`); only
  `mt load` brings it back, at BOT.
- `MTIOCGET` reports the same `GMT_*` bits a real drive reports — `BOT`/`EOF`/`EOT`/`EOD`,
  `WR_PROT`, `ONLINE`/`DR_OPEN`, density codes, and `IM_REP_EN` (on by default, as on hardware).
- `MTSETDRVBUFFER` and the `MT_ST_*` options that ride on it require `CAP_SYS_ADMIN`; the
  emulator proxies that capability as `euid == 0`.

See [Limitations](#limitations) for what is intentionally *not* modeled.

---

## Building

Requires a Rust toolchain and FUSE (`/dev/fuse`). For the end-to-end checks you also want
`mt` (mt-st), `dd`, `tar`, and `expect`.

```sh
cd simtap
cargo build --release
# → target/release/simtap
```

---

## Usage

One `simtap serve` process is one virtual drive. It mounts a FUSE directory exposing
`tape` (≈ `/dev/st0`) and `nst` (≈ `/dev/nst0`), and opens a control socket that the other
subcommands use to change the medium.

```
simtap serve --library DIR --mount DIR [--capacity BYTES] [--insert LABEL]
simtap insert LABEL --library DIR
simtap eject      --library DIR
simtap status     --library DIR
simtap list       --library DIR
```

### Start a drive

```sh
simtap serve --library /tmp/mylib --mount /tmp/mytape --insert TAPE001
# simtap: inserted cartridge 'TAPE001'
# simtap: serving tape at /tmp/mytape (socket: "/tmp/mylib/.simtap.sock");
#         unmount with fusermount3 -u /tmp/mytape or Ctrl-C
```

- `--library DIR` — directory holding the cartridge files (created if missing).
- `--mount DIR` — FUSE mount point for the drive (created if missing).
- `--capacity BYTES` — capacity for newly created cartridges (default **256 MiB**).
- `--insert LABEL` — insert a cartridge on startup (created blank if it does not exist).

`serve` runs in the foreground and blocks on the mount. Stop it with **Ctrl-C** (or
`fusermount3 -u /tmp/mytape`).

### Use it with the normal tools

In another shell, the mounted files behave like a tape drive:

```sh
$ ls /tmp/mytape
nst  tape

$ mt -f /tmp/mytape/tape status
SCSI 2 tape drive:
File number=0, block number=0, partition=0.
Tape block size 0 bytes. Density code 0x0 (default).
Soft error count since last status=0
General status bits on (41010000):
 BOT ONLINE IM_REP_EN

$ head -c 2048 /dev/urandom > f.dat
$ dd if=f.dat of=/tmp/mytape/tape bs=512 count=4 status=none
$ dd if=/tmp/mytape/tape of=f.out bs=512 count=4 status=none
$ cmp f.dat f.out          # identical

$ mt -f /tmp/mytape/nst tell
At block 0.
```

Because `/dev/st0`-style `tape` rewinds on close, each `mt`/`dd`/`tar` invocation starts from
the beginning of tape. The `nst` device preserves the head position across invocations, which
is what lets you append several archives to one cartridge.

### Change the medium

Cartridges are labeled files in the library. New labels are created blank automatically.

```sh
$ simtap eject --library /tmp/mylib
ok
$ simtap insert TAPE002 --library /tmp/mylib
ok
$ simtap status --library /tmp/mylib
label=TAPE002 loaded=true fileno=0 blkno=0 pos=0 datalen=0 marks=0 ...
$ simtap list --library /tmp/mylib
TAPE001
TAPE002
```

Data persists across eject/insert, so a cartridge can be ejected, set aside, and reinserted
later with its contents and filemarks intact.

### Multi-volume `tar` (spanning cartridges)

`tar --multi-volume` prompts for the next volume when the current one is full. The helper
`tar_multivolume.exp` (expect) watches for that prompt, swaps cartridges via the control
socket, and answers it — so a single archive can span as many cartridges as it needs.

```
Usage:
  tar_multivolume.exp <BIN> <LIB> <vols> <tar-args...>

  <BIN>   simtap binary
  <LIB>   library directory
  <vols>  comma-separated labels for volumes #2, #3, ... (the first volume must
          already be inserted; tar counts it as volume #1)
  <tar-args>  the tar command and its arguments
```

Example: write a `tar` archive starting on the already-inserted `span1`, spilling onto
`span2` and `span3` as each fills. Start the drive with small cartridges (200 KiB) so a
big archive spans several of them; the script inserts `span2`/`span3` automatically as
tar prompts for the next volume:

```sh
simtap serve --library /tmp/mylib --mount /tmp/mytape --capacity 204800 --insert span1 &

# Write a ~450 KB archive across span1 -> span2 -> span3.
expect tar_multivolume.exp target/release/simtap /tmp/mylib span2,span3 \
     tar -cf /tmp/mytape/nst -M -b 20 -C src .
```

Reading back is the same idea — re-insert the first volume and let the script load the rest:

```sh
simtap eject   --library /tmp/mylib
simtap insert  span1 --library /tmp/mylib
expect tar_multivolume.exp target/release/simtap /tmp/mylib span2,span3 \
     tar -x -M -b 20 -f /tmp/mytape/nst -C /tmp/restore
```

---

## Command reference

| Command | Purpose |
|---------|---------|
| `serve --library D --mount M [--capacity N] [--insert L]` | Start the drive, mount it, and serve the control socket. |
| `insert LABEL --library D` | Insert (create-if-new) cartridge `LABEL` into a running drive. Prints `ok`. |
| `eject --library D` | Eject the current cartridge (saving it) and empty the drive. Prints `ok`. |
| `status --library D` | Print a one-line summary of the live drive (label, position, filemarks, flags). |
| `list --library D` | List cartridge labels present in the library, one per line. |

---

## How it works

**One process = one drive.** `serve` owns a `CartridgeLibrary` (the media store) and a single
shared `TapeState` (the drive's head + medium) behind one `Arc<Mutex<_>>`. The FUSE worker
threads and the control-server thread all observe that same state, so `tape` and `nst` are
two views of one physical head on one medium.

- `src/tape.rs` — the engine. The tape is a byte stream (the `.tap` file) plus a sorted list
  of file-mark byte offsets (`marks`). It implements the `st` EOF state machine, every
  `MTIOCTOP` operation, and builds the exact `MTIOCGET` (`struct mtget`) and `MTIOCPOS`
  (`struct mtpos`) payloads.
- `src/cartridge.rs` — the on-disk store and insert/eject/persistence.
- `src/fs.rs` — the FUSE filesystem. Exposes `tape`/`nst`, dispatches I/O and ioctls to the
  shared state, and implements the `st` vs `nst` rewind-on-close behavior.
- `src/control.rs` — the control plane (Unix socket, JSON protocol).
- `src/main.rs` — the CLI.

### The non-seekable trick

A real tape is non-seekable, but a FUSE regular file cannot stop the kernel from handling
`SEEK_SET`/`SEEK_CUR` locally. simtap sidesteps this by **serving data at the kernel's file
offset** (head = `open_base + offset`). For `st`, `open_base` is always 0 (rewound on close),
so it is simply the offset; for `nst`, `open_base` is wherever the head was left, so a fresh
open appends mid-tape instead of overwriting at BOT. This keeps `tar -tf` and friends — which
`lseek` — in sync with the tape position. `SEEK_HOLE`/`SEEK_DATA` (the only `lseek`s the
kernel forwards) return `ESPIPE`, matching the driver's `noop_llseek`.

### On-disk layout (per library)

```
<library>/
  <label>.tap     raw tape bytes
  <label>.meta    JSON: { write_prot, capacity, density, block_size, marks: [byte offsets] }
  .simtap.sock    control-plane Unix socket (while the drive is running)
```

`marks` is persisted so filemarks survive eject/insert.

### Control-plane protocol

Newline-delimited JSON over `<library>/.simtap.sock`. Request `{"op": ...}` → response
`{"ok": bool, "error"?: str, "status"?: str}`:

- `{"op":"insert","label":"TAPE001"}` → `{"ok":true}`
- `{"op":"eject"}` → `{"ok":true}`
- `{"op":"status"}` → `{"ok":true,"status":"label=... loaded=true ..."}`

---

## Supported `MTIOCTOP` operations

| Op (code) | Behavior |
|-----------|----------|
| `MTREW`(6), `MTRETEN`(9) | Rewind to BOT. |
| `MTFSF`(1), `MTBSF`(2), `MTFSFM`(11), `MTBSFM`(10) | Space forward/backward over filemarks. |
| `MTFSR`(3), `MTBSR`(4) | Space forward/backward by records. |
| `MTWEOF`(5), `MTWEOFI`(35) | Write filemark(s). |
| `MTWSM`(27) | Write filemark and set the set-mark flag. |
| `MTEOM`(12) | Seek to end of data. |
| `MTERASE`(13) | Erase the tape from the head. |
| `MTNOP`(8) | No-op. |
| `MTSETBLK`(20) | Set the (fixed) record/block size. |
| `MTSETDENSITY`(21) | Set the density code. |
| `MTSEEK`(22) | Absolute seek to a block. |
| `MTFSS`(25), `MTBSS`(26) | Space forward/backward to the set-mark. |
| `MTLOCK`(28), `MTUNLOCK`(29) | Accepted (no-op). |
| `MTSETDRVBUFFER`(24) | Drive-buffering / `MT_ST_*` write-options; requires `euid == 0`. |
| `MTOFFL`(7), `MTUNLOAD`(31) | Take the medium offline (`GMT_DR_OPEN`). |
| `MTLOAD`(30) | Reload the medium, at BOT. |
| `MTCOMPRESSION`(32) | Accepted (no-op; no real compression is performed). |
| `MTSETPART`(33), `MTMKPART`(34) | Not supported (`EINVAL`). |
| `MTRESET`(0), `MTRAS1/2/3`(14–16) | Not supported (`ENOSYS`). |

`MTIOCGET` (48-byte `struct mtget`) and `MTIOCPOS` (8-byte `struct mtpos`) are both served.

---

## Testing

```sh
cargo test --lib     # unit tests: EOF state machine, spacing, marks, status bits, EOD, offline
./validate.sh        # end-to-end via mt/dd/tar against a live mount (builds first)
./validate.sh --no-build
```

`validate.sh` mounts a fresh drive and exercises the `st` contract through the real tools.
Highlights:

- **Basics** — `mt status`, `mt tell`, `mt rewind`, immediate `IM_REP_EN` flag, read/write via `dd`.
- **Persistence** — data survives an eject/insert cycle.
- **A** — multiple `tar` archives appended on one `nst` tape, recovered by `rewind`+`fsf`.
- **B** — back-to-back `dd` on `nst` (the dup-fd `EBUSY` regression).
- **C** — multi-volume `tar -M` across three small-capacity volumes (split-record
  reassembly, per-volume multi-volume headers), driven by `tar_multivolume.exp`.
- **D** — N archives, forward *and* backward navigation, byte-perfect, exact file numbers.
- **E** — a multi-file tape survives eject/insert.
- **F** — `st` and `nst` are two views of one physical head (one close rewinds for everyone).
- **G** — four archives across three tapes; recover archives 3 and 4 by **spacing over** the
  preceding archives with `mt fsf`, without re-reading them.
- **H** — `fsf` from a just-hit filemark settles cleanly (no phantom 0-byte read).
- **I** — filling the tape to capacity (`ENOSPC`), with the partial write intact.
- **J** — medium-change ops (`offline`/`rewoffl`/`load`) and the `drvbuffer` permission gate.

---

## Limitations

Faithful to the *contract* userspace sees, but not a full physical model. Deliberately:

- **No real compression.** `MTCOMPRESSION` is accepted as a no-op.
- **Soft-error count is always 0.** The emulator performs no physical reads that can produce
  recoverable errors, so the driver's soft-error accounting has nothing to count.
- **No physical-fault state.** `ST_NOT_READY` (drive present but not ready) is a hardware
  fault condition that does not apply to a functioning emulator.
- **No partitions.** `MTSETPART`/`MTMKPART` return `EINVAL`; the drive is single-partition.
- **`MTRESET`/`MTRAS*`** return `ENOSYS`.
- **One drive, one medium at a time.** A cartridge is in use until ejected.
- **Not truly non-seekable** for `SEEK_SET`/`SEEK_CUR` (the kernel handles those locally); the
  kernel-offset mapping above is what keeps seeking tools consistent.

---

## Repository layout

```
simtap/
  Cargo.toml          standalone crate (its own workspace)
  src/
    main.rs           CLI (serve/insert/eject/status/list)
    lib.rs            module root
    tape.rs           drive engine: EOF state machine, MT ops, status payloads, unit tests
    cartridge.rs      cartridge store, insert/eject, persistence
    fs.rs             FUSE filesystem (tape/nst)
    control.rs        control-plane Unix socket + JSON protocol
  tar_multivolume.exp expect helper that swaps cartridges for `tar -M`
  validate.sh         end-to-end validation script
```
