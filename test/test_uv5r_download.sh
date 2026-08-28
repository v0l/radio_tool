#!/bin/bash
# Downloads a codeplug from a simulated UV-5R on a PTY and checks the result
# matches what the simulator was serving.
#
# Usage: test_uv5r_download.sh <radio_tool binary> <test source dir>
set -u

RADIO_TOOL="$1"
SRC_DIR="$2"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

python3 -u "$SRC_DIR/fake_uv5r.py" > "$TMP_DIR/port.txt" 2> "$TMP_DIR/sim.err" &
SIM_PID=$!

# wait for the simulator to publish its pty name
for _ in $(seq 1 50); do
    [ -s "$TMP_DIR/port.txt" ] && break
    sleep 0.1
done

PORT=$(head -1 "$TMP_DIR/port.txt")
if [ -z "$PORT" ]; then
    echo "FAIL: simulator did not start"
    cat "$TMP_DIR/sim.err"
    exit 1
fi

"$RADIO_TOOL" --port "$PORT" --radio UV5R --read-codeplug -o "$TMP_DIR/uv5r.img" > "$TMP_DIR/dl.log" 2>&1
RC=$?
# the simulator sits waiting on the pty master, it never sees an EOF
kill $SIM_PID 2>/dev/null
wait $SIM_PID 2>/dev/null

if [ $RC -ne 0 ]; then
    echo "FAIL: download failed"
    cat "$TMP_DIR/dl.log" "$TMP_DIR/sim.err"
    exit 1
fi

# 8 byte ident + 0x1800 main + 0x140 aux
SIZE=$(wc -c < "$TMP_DIR/uv5r.img")
if [ "$SIZE" -ne 6472 ]; then
    echo "FAIL: expected a 6472 byte image, got $SIZE"
    exit 1
fi

INFO=$("$RADIO_TOOL" --codeplug-info -i "$TMP_DIR/uv5r.img" 2>&1)
echo "$INFO"

check() {
    if ! echo "$INFO" | grep -q "$1"; then
        echo "FAIL: expected '$1' in codeplug info"
        exit 1
    fi
}

check "Baofeng UV-5R Codeplug"
check "HN5RV01"
check "HELLO / RADIO"
check "SIMPLEX     145.50000  145.50000"
check "RPT1        145.62500  145.02500"
check "PMR1        446.00625  446.00625"
check "3 of 128 channels used"

echo "PASS"
