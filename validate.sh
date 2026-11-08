#!/usr/bin/env bash
#
# End-to-end validation of simtap using standard Linux tape tooling
# (mt, dd, tar). Starts a fresh drive, exercises the st contract as seen
# through real userspace tools, and verifies data survives an eject/insert
# cycle.
#
# Faithfulness notes baked in here (all confirmed against a live mount):
#   * /dev/st0 rewinds on close, so tape position does NOT persist across
#     separate mt/dd invocations. Each tool run starts at BOT. Therefore this
#     script does single-file round-trips and only checks that spacing
#     commands *succeed* (rc=0); the full file-mark state machine is covered
#     by the Rust unit tests (cargo test).
#   * Any close after a write auto-writes a WEOF then rewinds, so "mt status"
#     after a dd/tar write reports BOT, not EOF.
#   * A tape is non-seekable, but FUSE cannot make a regular file non-seekable
#     for SEEK_SET/CUR (the kernel handles those locally); simtap instead
#     honours the kernel file offset as the tape position so tools that lseek
#     (e.g. `tar -tf`) stay in sync.
#
# Usage: ./validate.sh [--no-build]
#
set -u

ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN="$ROOT/target/release/simtap"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/simtap-validate.XXXXXX")"
LIB="$WORK/lib"
MNT="$WORK/mnt"
TAPE="$MNT/tape"
NST="$MNT/nst"
EXP="$ROOT/tar_multivolume.exp"
SERVER_PID=""
FAILURES=0

pass() { echo "PASS: $*"; }
fail() { echo "FAIL: $*"; FAILURES=$((FAILURES + 1)); }

# check <description> <cmd...>  -> passes if the command exits 0
check() {
    local desc="$1"; shift
    if "$@" >/dev/null 2>&1; then pass "$desc"; else fail "$desc"; fi
}

# check_fails <description> <cmd...>  -> passes if the command exits non-zero
check_fails() {
    local desc="$1"; shift
    if "$@" >/dev/null 2>&1; then fail "$desc (expected failure, but it succeeded)"; else pass "$desc"; fi
}

# check_grep <description> <pattern> <cmd...>  -> passes if combined output matches
check_grep() {
    local desc="$1" pattern="$2"; shift 2
    local out
    out="$("$@" 2>&1)"
    if printf '%s\n' "$out" | grep -q "$pattern"; then
        pass "$desc"
    else
        fail "$desc (wanted /$pattern/, got: $(printf '%s' "$out" | tr '\n' ' '))"
    fi
}

cleanup() {
    fusermount3 -u "$MNT" >/dev/null 2>&1 || true
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" >/dev/null 2>&1 || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    [ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"
}
trap cleanup EXIT

# ---------- setup ----------

if [ "${1:-}" != "--no-build" ]; then
    if ! cargo build --release --manifest-path "$ROOT/Cargo.toml" >/dev/null 2>&1; then
        echo "FAIL: cargo build --release"
        exit 1
    fi
fi
if [ ! -x "$BIN" ]; then
    echo "FAIL: binary not found at $BIN (run without --no-build)"
    exit 1
fi

mkdir -p "$LIB" "$MNT"

"$BIN" serve --library "$LIB" --mount "$MNT" --insert valtape >"$WORK/serve.log" 2>&1 &
SERVER_PID=$!

up=0
for _ in $(seq 1 100); do
    if [ -f "$TAPE" ]; then up=1; break; fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then break; fi
    sleep 0.1
done
if [ "$up" -ne 1 ]; then
    fail "drive came up (serve.log: $(tail -3 "$WORK/serve.log" 2>/dev/null | tr '\n' ' '))"
    exit 1
fi
pass "drive mounted with 'valtape' inserted"

# ---------- mt basics (fresh cartridge, at BOT) ----------

check_grep "mt status works (MTIOCGET through FUSE)" "BOT ONLINE" mt -f "$TAPE" status
check_grep "fresh tape at BOT, file 0 block 0" "File number=0, block number=0" mt -f "$TAPE" status
# A real st drive reports immediate-report mode (GMT_IM_REP_EN) by default: the
# driver's do_async_writes and the negotiated drive-buffering bit are both on
# out of the box (st.c:4020-4023,4497; st_options.h ST_ASYNC_WRITES=1).
check_grep "immediate-report mode on by default (GMT_IM_REP_EN)" "IM_REP_EN" mt -f "$TAPE" status
check_grep "mt tell reports block 0" "At block 0" mt -f "$TAPE" tell
check "mt rewind (MTREW)" mt -f "$TAPE" rewind
check_grep "simtap status reports loaded" "loaded=true" "$BIN" status --library "$LIB"

# ---------- dd write a single file ----------

head -c 5120 /dev/urandom >"$WORK/f1.dat"
check "write file 1 (10 x 512B) via dd" dd if="$WORK/f1.dat" of="$TAPE" bs=512 count=10 status=none
check_grep "after dd close: auto WEOF + rewind -> BOT" "BOT ONLINE" mt -f "$TAPE" status
check_grep "file mark written (back to file 0)" "File number=0, block number=0" mt -f "$TAPE" status

# ---------- immediate read-back ----------

dd if="$TAPE" of="$WORK/f1.out" bs=512 count=10 status=none
check "read back file 1, content identical" cmp -s "$WORK/f1.dat" "$WORK/f1.out"

# ---------- eject -> no medium ----------

check "simtap eject" "$BIN" eject --library "$LIB"
check_grep "ejected drive: loaded=false" "loaded=false" "$BIN" status --library "$LIB"
check_fails "dd read with no medium fails" dd if="$TAPE" of="$WORK/noeject.out" bs=512 count=1
check_grep "mt status after eject shows DR_OPEN" "DR_OPEN" mt -f "$TAPE" status

# ---------- re-insert -> data persisted ----------

check "re-insert 'valtape'" "$BIN" insert valtape --library "$LIB"
check_grep "re-inserted tape: loaded=true" "loaded=true" "$BIN" status --library "$LIB"
dd if="$TAPE" of="$WORK/f1b.out" bs=512 count=10 status=none
check "file 1 intact after eject/insert cycle" cmp -s "$WORK/f1.dat" "$WORK/f1b.out"

# ---------- insert blank cartridge ----------

check "insert blank 'second' cartridge" "$BIN" insert second --library "$LIB"
check_grep "blank cartridge at BOT" "BOT ONLINE" mt -f "$TAPE" status
dd if="$TAPE" of="$WORK/empty.out" bs=512 count=1 status=none
check "blank cartridge reads zero bytes" test ! -s "$WORK/empty.out"

# ---------- spacing commands + tar (overwrites from BOT) ----------

check "re-insert 'valtape'" "$BIN" insert valtape --library "$LIB"

# Each spacing op runs in its own open (starts at BOT); we only require success.
# These may rewrite file marks (e.g. `weof` at BOT creates an empty file), so
# they run only after the persistence read-back, just before the tar overwrite.
check "mt fsf 1 succeeds from BOT" mt -f "$TAPE" fsf 1
check "mt weof succeeds from BOT" mt -f "$TAPE" weof
check "mt rewind succeeds" mt -f "$TAPE" rewind
check_grep "tell still block 0 (rewind-on-close)" "At block 0" mt -f "$TAPE" tell
check_fails "bsf from BOT fails with EIO (faithful non-seekable)" mt -f "$TAPE" bsf 1

mkdir -p "$WORK/src"
echo "hello simtap" >"$WORK/src/a.txt"
head -c 1048576 /dev/urandom >"$WORK/src/big.bin"

check "tar -cf writes archive to tape" tar -cf "$TAPE" -C "$WORK/src" .
check_grep "after tar close: auto WEOF -> BOT" "BOT ONLINE" mt -f "$TAPE" status
check "tar -tf lists archive from tape" tar -tf "$TAPE"
mkdir -p "$WORK/dst"
check "tar -xf extracts from tape" tar -xf "$TAPE" -C "$WORK/dst"
check "extracted big.bin identical" cmp -s "$WORK/src/big.bin" "$WORK/dst/./big.bin"
check "extracted a.txt identical" cmp -s "$WORK/src/a.txt" "$WORK/dst/./a.txt"

# ---------- A: multiple archives on one nst tape ----------
# nst does not rewind on close, so successive writes append to the same
# cartridge. Two tar archives land back-to-back; each is recovered by
# rewind + fsf. (The st device can't do this: it rewinds on every close.)

check "insert 'multiarc' cartridge" "$BIN" insert multiarc --library "$LIB"
mkdir -p "$WORK/arc1" "$WORK/arc2"
echo "alpha archive" >"$WORK/arc1/a.txt"
head -c 61440 /dev/urandom >"$WORK/arc2/b.bin"

check "tar -cf archive 1 to nst" tar -cf "$NST" -C "$WORK/arc1" a.txt
check "tar -cf archive 2 to nst (appends, no rewind)" tar -cf "$NST" -C "$WORK/arc2" b.bin

check "rewind before reading archive 1" mt -f "$NST" rewind
check_grep "archive 1 (from BOT) lists a.txt" "^a\.txt$" tar -tf "$NST"
check "rewind before reading archive 2" mt -f "$NST" rewind
check "fsf 1 to archive 2" mt -f "$NST" fsf 1
check_grep "archive 2 (after fsf 1) lists b.bin" "^b\.bin$" tar -tf "$NST"

# ---------- B: back-to-back dd on nst (the EBUSY regression) ----------
# GNU dd dups its output fd, so one dd emits OPEN, FLUSH, WRITE, FLUSH,
# RELEASE. Two dds back-to-back used to leak the drive as busy (EBUSY).

check "insert 'multidd' cartridge" "$BIN" insert multidd --library "$LIB"
head -c 8192 /dev/urandom >"$WORK/d1.dat"
head -c 8192 /dev/urandom >"$WORK/d2.dat"
check "dd file 1 to nst" dd if="$WORK/d1.dat" of="$NST" bs=8192 count=1 status=none
check "dd file 2 to nst (back-to-back, no EBUSY)" dd if="$WORK/d2.dat" of="$NST" bs=8192 count=1 status=none
check "rewind before reading d1" mt -f "$NST" rewind
dd if="$NST" of="$WORK/d1.out" bs=8192 count=1 status=none
check "d1 intact after back-to-back writes" cmp -s "$WORK/d1.dat" "$WORK/d1.out"
check "rewind before reading d2" mt -f "$NST" rewind
check "fsf 1 to d2" mt -f "$NST" fsf 1
dd if="$NST" of="$WORK/d2.out" bs=8192 count=1 status=none
check "d2 intact after back-to-back writes" cmp -s "$WORK/d2.dat" "$WORK/d2.out"

# ---------- C: multi-volume tar -M across small-capacity volumes ----------
# Three 204800-byte (20-record) volumes; a ~450KB archive overflows the first
# two. tar -M prompts, the expect driver swaps cartridges, and the result must
# be a valid multi-volume archive (vol1 byte-identical to a plain tar's prefix,
# vol2/vol3 correctly split, full extraction byte-identical).
if command -v expect >/dev/null 2>&1; then
    for v in mv1 mv2 mv3; do
        : >"$LIB/$v.tap"
        printf '{"write_prot":false,"capacity":204800,"density":0,"block_size":0,"marks":[]}' >"$LIB/$v.meta"
    done
    mkdir -p "$WORK/mvsrc"
    echo "hello multivolume" >"$WORK/mvsrc/a.txt"
    head -c 450000 /dev/urandom >"$WORK/mvsrc/b.bin"
    tar -cf "$WORK/plain_ref.tar" -C "$WORK/mvsrc" .

    check "insert 'mv1' for multi-volume write" "$BIN" insert mv1 --library "$LIB"
    check "tar -M write spans three volumes" expect "$EXP" "$BIN" "$LIB" mv2,mv3 tar -cf "$NST" -M -b 20 -C "$WORK/mvsrc" .

    for v in 1 2; do
        if [ "$(stat -c%s "$LIB/mv$v.tap" 2>/dev/null)" = "204800" ]; then
            pass "vol$v filled to capacity (204800)"
        else
            fail "vol$v filled to capacity (204800) (got $(stat -c%s "$LIB/mv$v.tap" 2>/dev/null))"
        fi
    done
    v3=$(stat -c%s "$LIB/mv3.tap" 2>/dev/null || echo 0)
    if [ "$v3" -gt 0 ] && [ "$v3" -lt 204800 ]; then
        pass "vol3 is a partial final volume ($v3 bytes)"
    else
        fail "vol3 is a partial final volume (got $v3)"
    fi
    head -c 204800 "$WORK/plain_ref.tar" >"$WORK/vol1_prefix.bin"
    check "vol1 byte-identical to plain tar prefix" cmp -s "$WORK/vol1_prefix.bin" "$LIB/mv1.tap"

    # Continuation volumes must begin with a GNU multi-volume header:
    # typeflag 'M' (GNUTYPE_MULTIVOL in GNU tar >= 1.32; 'G' in older tars)
    # naming the split file, with size = bytes of that file still to write.
    # A full continuation volume holds exactly 204800 - 512 data bytes, so
    # size(vol2) - size(vol3) must be exactly 204288. (Spec-level, verified
    # against the GNU tar 1.35 source; independent of our emulator.)
    for v in 2 3; do
        tf=$(dd if="$LIB/mv$v.tap" bs=1 skip=156 count=1 status=none)
        if [ "$tf" = "M" ] || [ "$tf" = "G" ]; then
            pass "vol$v starts with a multi-volume header (typeflag '$tf')"
        else
            fail "vol$v starts with a multi-volume header (typeflag got '$tf')"
        fi
        if head -c 100 "$LIB/mv$v.tap" | grep -aq "\./b\.bin"; then
            pass "vol$v multi-volume header names the split file"
        else
            fail "vol$v multi-volume header names the split file"
        fi
    done
    mvsz() { dd if="$1" bs=1 skip=124 count=12 status=none | tr -cd '0-7'; }
    m2=$(( 8#$(mvsz "$LIB/mv2.tap") ))
    m3=$(( 8#$(mvsz "$LIB/mv3.tap") ))
    if [ $(( m2 - m3 )) -eq 204288 ]; then
        pass "vol2 holds a full 204288 data bytes (size fields consistent: $m2 - $m3)"
    else
        fail "vol2 holds a full 204288 data bytes (size fields give $(( m2 - m3 )): $m2 - $m3)"
    fi

    check "insert 'mv1' for multi-volume read-back" "$BIN" insert mv1 --library "$LIB"
    mkdir -p "$WORK/mvdst"
    check "tar -M read extracts across three volumes" expect "$EXP" "$BIN" "$LIB" mv2,mv3 tar -x -M -b 20 -f "$NST" -C "$WORK/mvdst"
    check "multi-volume a.txt identical" cmp -s "$WORK/mvsrc/a.txt" "$WORK/mvdst/./a.txt"
    check "multi-volume b.bin identical" cmp -s "$WORK/mvsrc/b.bin" "$WORK/mvdst/./b.bin"
else
    echo "SKIP: section C (multi-volume tar -M) — expect not installed"
fi

# ---------- D: N archives, forward+backward navigation, byte-perfect ----------
# N files on one nst tape, each separated by an auto filemark. Recover every
# file by forward (rewind+fsf) AND backward (fsf to last, then bsf) navigation,
# byte-identical, and assert mt reports the exact file number.
check "insert 'many' cartridge" "$BIN" insert many --library "$LIB"
N=4
mkdir -p "$WORK/many"
for i in $(seq 1 $N); do
    head -c $((i * 4096)) /dev/urandom >"$WORK/many/f$i.dat"
done
for i in $(seq 1 $N); do
    check "write f$i to nst" dd if="$WORK/many/f$i.dat" of="$NST" bs=4096 count=$i status=none
done
# Exact filemark byte positions: files are i*4096 bytes, so file i (0-based)
# starts at 4096*(1+...+i) = 4096*i*(i+1)/2 bytes = 4*i*(i+1) 512-byte blocks.
for i in $(seq 0 $((N - 1))); do
    mt -f "$NST" rewind
    [ "$i" -gt 0 ] && mt -f "$NST" fsf $i
    check_grep "forward fsf $i -> File number=$i" "File number=$i" mt -f "$NST" status
    b=$(( 4 * i * (i + 1) ))
    check_grep "forward fsf $i -> exact byte offset (At block $b)" "At block $b" mt -f "$NST" tell
    dd if="$NST" of="$WORK/many/fwd_$i.out" bs=4096 count=$((i + 1)) status=none
    check "forward fsf $i recovers f$((i + 1)) byte-perfect" cmp -s "$WORK/many/f$((i + 1)).dat" "$WORK/many/fwd_$i.out"
done
mt -f "$NST" rewind
mt -f "$NST" fsf $((N - 1))
for i in $(seq $N -1 1); do
    dd if="$NST" of="$WORK/many/bwd_$i.out" bs=4096 count=$i status=none
    check "backward bsf to f$i recovers it byte-perfect" cmp -s "$WORK/many/f$i.dat" "$WORK/many/bwd_$i.out"
    b=$(( 4 * i * (i + 1) ))
    check_grep "after f$i: head on its exact filemark (At block $b)" "At block $b" mt -f "$NST" tell
    [ "$i" -gt 1 ] && mt -f "$NST" bsf 1
done

# ---------- E: multi-file tape survives eject/insert ----------
# All N files and their filemarks must persist across an eject/insert cycle.
check "eject 'many'" "$BIN" eject --library "$LIB"
check_grep "ejected 'many' (loaded=false)" "loaded=false" "$BIN" status --library "$LIB"
check "re-insert 'many'" "$BIN" insert many --library "$LIB"
for i in $(seq 0 $((N - 1))); do
    mt -f "$NST" rewind
    [ "$i" -gt 0 ] && mt -f "$NST" fsf $i
    dd if="$NST" of="$WORK/many/per_$i.out" bs=4096 count=$((i + 1)) status=none
    check "after eject/insert, f$((i + 1)) intact" cmp -s "$WORK/many/f$((i + 1)).dat" "$WORK/many/per_$i.out"
done

# ---------- F: st and nst are two views of one physical head ----------
# Whatever device the data went in through, /dev/st0 and /dev/nst0 observe
# the same cartridge state (one head, one medium), and an st close rewinds
# the shared head for everyone.
check "insert 'cross' cartridge" "$BIN" insert cross --library "$LIB"
mkdir -p "$WORK/cross"
head -c 8192  /dev/urandom >"$WORK/cross/x1.dat"
head -c 16384 /dev/urandom >"$WORK/cross/x2.dat"
check "write x1 via nst" dd if="$WORK/cross/x1.dat" of="$NST" bs=8192 count=1 status=none
check "write x2 via nst" dd if="$WORK/cross/x2.dat" of="$NST" bs=8192 count=2 status=none
# Head sits at EOT after x2's auto-WEOF; st must report that same state.
# One status only: the st close of a second invocation would rewind first.
out=$(mt -f "$TAPE" status 2>&1)
if printf '%s\n' "$out" | grep -q "File number=2, block number=0" && \
   printf '%s\n' "$out" | grep -q "EOF"; then
    pass "st sees nst-written files at EOT (File number=2, EOF flag)"
else
    fail "st sees nst-written files at EOT (got: $(printf '%s' "$out" | tr '\n' ' '))"
fi
# That st open/close rewound the shared head: nst now starts at BOT.
check_grep "nst sees BOT after st close (shared head)" "BOT ONLINE" mt -f "$NST" status
# nst-written data reads back byte-perfect through the st device.
dd if="$TAPE" of="$WORK/cross/x1_st.out" bs=8192 count=1 status=none
check "x1 read via st byte-perfect (nst-written)" cmp -s "$WORK/cross/x1.dat" "$WORK/cross/x1_st.out"
# dd skip maps to the kernel file offset, which simtap serves as the tape
# position (see header note), so x2 is reachable through st without rewinding.
dd if="$TAPE" of="$WORK/cross/x2_st.out" bs=8192 count=2 skip=1 status=none
check "x2 read via st byte-perfect (nst-written)" cmp -s "$WORK/cross/x2.dat" "$WORK/cross/x2_st.out"
# Reverse direction: the files still read back byte-perfect via nst after st
# usage (the st closes left the head rewound, so start over from BOT).
dd if="$NST" of="$WORK/cross/x1_nst.out" bs=8192 count=1 status=none
check "x1 read via nst byte-perfect (after st use)" cmp -s "$WORK/cross/x1.dat" "$WORK/cross/x1_nst.out"
mt -f "$NST" rewind
mt -f "$NST" fsf 1
dd if="$NST" of="$WORK/cross/x2_nst.out" bs=8192 count=2 status=none
check "x2 read via nst byte-perfect (after st use)" cmp -s "$WORK/cross/x2.dat" "$WORK/cross/x2_nst.out"

# ---------- G: four archives across three tapes; recover 3 & 4 without re-reading 1 & 2 ----------
# The headline multi-tape scenario:
#   * archives 1 and 2 live entirely on tape 1;
#   * archive 3 starts on tape 1 and spans onto tape 2;
#   * archive 4 starts on tape 2 and spans onto tape 3.
# We must recover archive 3 from tapes 1+2 and archive 4 from tapes 2+3, and —
# the crux — reach each archive by SPACING over the preceding archives with
# `mt fsf` (a positional filemark-space, no data read), not by re-reading them.
#
#   span1: [A1][WM][A2][WM][A3 part1]              <- full  (A3 is file 2)
#   span2: [A3 part2][WM][A4 part1]                <- full  (A4 is file 1)
#   span3: [A4 part2][WM]                          <- partial
#
# Each archive is a single-file tar written with -b 20 (10240-byte records), so
# archive boundaries land on record boundaries and the two leading archives
# leave span1 aligned; `mt fsf` then lands exactly on the next archive's start.
if command -v expect >/dev/null 2>&1; then
    for v in span1 span2 span3; do
        : >"$LIB/$v.tap"
        printf '{"write_prot":false,"capacity":204800,"density":0,"block_size":0,"marks":[]}' >"$LIB/$v.meta"
    done
    mkdir -p "$WORK/g1" "$WORK/g2" "$WORK/g3" "$WORK/g4"
    head -c 51200  /dev/urandom >"$WORK/g1/a1.bin"   # A1 (fits span1)
    head -c 51200  /dev/urandom >"$WORK/g2/a2.bin"   # A2 (fits span1)
    head -c 153600 /dev/urandom >"$WORK/g3/a3.bin"   # A3 (spans span1 -> span2)
    head -c 163840 /dev/urandom >"$WORK/g4/a4.bin"   # A4 (spans span2 -> span3)

    check "insert 'span1' for the archive write" "$BIN" insert span1 --library "$LIB"
    check "write archive 1 to span1" tar -cf "$NST" -b 20 -C "$WORK/g1" a1.bin
    check "write archive 2 to span1 (appends, no rewind)" tar -cf "$NST" -b 20 -C "$WORK/g2" a2.bin
    check "write archive 3 (spans span1 -> span2)" expect "$EXP" "$BIN" "$LIB" span2 tar -cf "$NST" -M -b 20 -C "$WORK/g3" a3.bin
    check "write archive 4 (spans span2 -> span3)" expect "$EXP" "$BIN" "$LIB" span3 tar -cf "$NST" -M -b 20 -C "$WORK/g4" a4.bin

    # The spanning must fill span1 and span2 to capacity and leave span3 partial.
    for v in 1 2; do
        if [ "$(stat -c%s "$LIB/span$v.tap" 2>/dev/null)" = "204800" ]; then
            pass "span$v filled to capacity (204800)"
        else
            fail "span$v filled to capacity (204800) (got $(stat -c%s "$LIB/span$v.tap" 2>/dev/null))"
        fi
    done
    s3=$(stat -c%s "$LIB/span3.tap" 2>/dev/null || echo 0)
    if [ "$s3" -gt 0 ] && [ "$s3" -lt 204800 ]; then
        pass "span3 is a partial final volume ($s3 bytes)"
    else
        fail "span3 is a partial final volume (got $s3)"
    fi

    # --- recover archive 3 from span1+span2, spacing over A1+A2 with fsf ---
    check "insert 'span1' for archive 3 recovery" "$BIN" insert span1 --library "$LIB"
    check "fsf 2 spaces over A1+A2 to file 2 (A3 start)" mt -f "$NST" fsf 2
    check_grep "head at A3 start: File number=2" "File number=2" mt -f "$NST" status
    mkdir -p "$WORK/dst3"
    check "tar -M recovers archive 3 across span1 -> span2" expect "$EXP" "$BIN" "$LIB" span2 tar -x -M -b 20 -f "$NST" -C "$WORK/dst3"
    check "archive 3 extracted byte-perfect" cmp -s "$WORK/g3/a3.bin" "$WORK/dst3/a3.bin"

    # --- recover archive 4 from span2+span3, spacing over A3 with fsf ---
    check "insert 'span2' for archive 4 recovery" "$BIN" insert span2 --library "$LIB"
    check "fsf 1 spaces over A3 to file 1 (A4 start)" mt -f "$NST" fsf 1
    check_grep "head at A4 start: File number=1" "File number=1" mt -f "$NST" status
    mkdir -p "$WORK/dst4"
    check "tar -M recovers archive 4 across span2 -> span3" expect "$EXP" "$BIN" "$LIB" span3 tar -x -M -b 20 -f "$NST" -C "$WORK/dst4"
    check "archive 4 extracted byte-perfect" cmp -s "$WORK/g4/a4.bin" "$WORK/dst4/a4.bin"

    # --- integrity: the spanning writes did not corrupt archives 1 & 2 on span1 ---
    # (This deliberately DOES read archive 1 from BOT; it is a separate
    #  integrity check, not part of the A3/A4 recovery above.)
    check "insert 'span1' for archive 1 integrity check" "$BIN" insert span1 --library "$LIB"
    mkdir -p "$WORK/dst1"
    check_grep "archive 1 (from BOT) still lists a1.bin" "^a1\.bin$" tar -tf "$NST"
    mt -f "$NST" rewind
    check "archive 1 extracted byte-perfect after all spanning" tar -xf "$NST" -C "$WORK/dst1"
    check "archive 1 a1.bin identical (intact after spanning writes)" cmp -s "$WORK/g1/a1.bin" "$WORK/dst1/a1.bin"
else
    echo "SKIP: section G (archive spanning across tapes) — expect not installed"
fi

# ---------- H: fsf from a just-hit filemark (st.c settle regression) ----------
# A real st drive settles after a successful MTFSF: the head is at the mark
# (start of the next file), block 0, eof=ST_FM (st.c:3030,3055) -- the next
# read returns data, never a spurious 0-byte filemark read. If a READ stops on
# a mark (ST_FM_HIT) and the next command is `mt fsf 1`, the mark the head
# sits on counts as the first mark crossed (st.c:3726-3737), so the spacing is
# fully consumed and the drive must settle exactly as after a plain MTFSF.
# Regression: the old code left eof=ST_FM_HIT here, so the following read
# returned 0 bytes before the file's data (phantom filemark completion).
check "insert 'fsfgap' cartridge" "$BIN" insert fsfgap --library "$LIB"
mkdir -p "$WORK/h"
head -c 5120 /dev/urandom >"$WORK/h/h1.dat"   # file 0
head -c 4096 /dev/urandom >"$WORK/h/h2.dat"   # file 1
head -c 6144 /dev/urandom >"$WORK/h/h3.dat"   # file 2
check "write h1 to nst" dd if="$WORK/h/h1.dat" of="$NST" bs=1024 count=5 status=none
check "write h2 to nst" dd if="$WORK/h/h2.dat" of="$NST" bs=1024 count=4 status=none
check "write h3 to nst" dd if="$WORK/h/h3.dat" of="$NST" bs=1024 count=6 status=none
mt -f "$NST" rewind
# Read file 0 fully: the head stops ON its filemark (ST_FM_HIT, fileno=0).
dd if="$NST" of="$WORK/h/h1.read" bs=1024 count=5 status=none
check "file 0 read to its mark byte-perfect" cmp -s "$WORK/h/h1.dat" "$WORK/h/h1.read"
# The regression itself: fsf 1 from a just-hit mark must land clean on file 1.
check "fsf 1 from just-hit mark" mt -f "$NST" fsf 1
check_grep "settled at file 1 start: File number=1, block number=0" "File number=1, block number=0" mt -f "$NST" status
check_grep "settled at exact byte offset (At block 10)" "At block 10" mt -f "$NST" tell
dd if="$NST" of="$WORK/h/h2.read" bs=1024 count=4 status=none
check "file 1 after fsf-from-mark byte-perfect (no phantom 0-byte read)" cmp -s "$WORK/h/h2.dat" "$WORK/h/h2.read"

# ---------- I: filling the tape to capacity (ENOSPC) ----------
# A real drive returns -ENOSPC ("No space left on device") the moment a write
# would run past the medium (st.c:1716-1732), which also flags end-of-data so
# `mt status` would show EOD while still open. On close the drive auto-writes
# the filemark and rewinds (st_flush:1407-1484), clearing that transient flag,
# so the drive reports BOT again. The partial file that did fit stays intact.
: >"$LIB/full.tap"
printf '{"write_prot":false,"capacity":8192,"density":0,"block_size":0,"marks":[]}' >"$LIB/full.meta"
check "insert 'full' cartridge (8 KiB capacity)" "$BIN" insert full --library "$LIB"
head -c 32768 /dev/urandom >"$WORK/full.dat"
# 16 x 512B blocks fit in 8192 bytes; the 17th write overflows -> ENOSPC.
check_grep "dd filling the tape fails with ENOSPC" "No space left" dd if="$WORK/full.dat" of="$TAPE" bs=512 count=64 status=none
check_grep "after the fill: auto WEOF + rewind -> BOT (transient EOD cleared)" "BOT ONLINE" mt -f "$TAPE" status
dd if="$TAPE" of="$WORK/full.out" bs=512 count=16 status=none
head -c 8192 "$WORK/full.dat" >"$WORK/full.expected"
check "the 8192 bytes that fit are intact" cmp -s "$WORK/full.expected" "$WORK/full.out"

# ---------- J: medium-change ops (offline/eject/load) + drvbuffer permission gate ----------
# A real st drive supports SCSI medium-change: `mt offline`/`rewoffl`/`eject`
# take the medium offline (ready=ST_NO_TAPE) -- I/O then fails with ENOMEDIUM
# and MTIOCGET reports DR_OPEN instead of ONLINE; only `mt load` brings the
# medium back, at BOT (st.c:2635-2647,2767,3969-4017). MTSETDRVBUFFER (and the
# MT_ST_* options riding on it) additionally require CAP_SYS_ADMIN
# (st.c:3711-3716); the emulator proxies that capability as euid==0.
check "insert 'medchg' cartridge" "$BIN" insert medchg --library "$LIB"
mkdir -p "$WORK/j"
head -c 5120 /dev/urandom >"$WORK/j/j1.dat"
check "write j1 to nst" dd if="$WORK/j/j1.dat" of="$NST" bs=512 count=10 status=none
check "mt offline (MTOFFL)" mt -f "$TAPE" offline
check_grep "offline drive reports DR_OPEN" "DR_OPEN" mt -f "$TAPE" status
check_fails "dd read while offline fails with ENOMEDIUM" dd if="$TAPE" of="$WORK/j/off.out" bs=512 count=1
check "mt load (MTLOAD) restores the medium at BOT" mt -f "$TAPE" load 1
check_grep "loaded drive back at BOT ONLINE" "BOT ONLINE" mt -f "$TAPE" status
dd if="$TAPE" of="$WORK/j/j1.out" bs=512 count=10 status=none
check "j1 intact after offline/load cycle" cmp -s "$WORK/j/j1.dat" "$WORK/j/j1.out"
# `rewoffl` is the second path to offline (rewind + offline); from ONLINE it
# takes the medium offline the same way, and `load` restores it again.
check "mt rewoffl takes the medium offline" mt -f "$TAPE" rewoffl
check_grep "rewoffl drive reports DR_OPEN" "DR_OPEN" mt -f "$TAPE" status
check "mt load restores the medium after rewoffl" mt -f "$TAPE" load 1
# MTSETDRVBUFFER needs CAP_SYS_ADMIN: root (euid 0) always holds it, a
# non-root caller gets EPERM. The drive is ONLINE here (the gate only applies
# to an online medium; offline would mask it with ENOMEDIUM).
if [ "$(id -u)" -eq 0 ]; then
    check "mt drvbuffer 0 (root: CAP_SYS_ADMIN held)" mt -f "$TAPE" drvbuffer 0
else
    check_fails "mt drvbuffer 0 (non-root: EPERM, no CAP_SYS_ADMIN)" mt -f "$TAPE" drvbuffer 0
fi

# ---------- library bookkeeping ----------

check_grep "list shows 'valtape'" "^valtape$" "$BIN" list --library "$LIB"
check_grep "list shows 'second'" "^second$" "$BIN" list --library "$LIB"
check "cartridge data file on disk" test -s "$LIB/valtape.tap"
check "cartridge meta file on disk" test -s "$LIB/valtape.meta"
check "simtap status command works" "$BIN" status --library "$LIB"

# ---------- summary ----------

echo
if [ "$FAILURES" -eq 0 ]; then
    echo "ALL TESTS PASSED"
else
    echo "$FAILURES TEST(S) FAILED"
    KEEP_WORK=1
    echo "work dir kept for inspection: $WORK"
    exit 1
fi
