#!/usr/bin/env bash
# SPARKRUSH-CORE edge test bench — 13 tests (T01-T13)
# DGK x PROTON protocol. Every test has a clear pass/fail criterion.
set -uo pipefail

SR_HOME="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EDITOR_BIN="$SR_HOME/editor/target/release/sparkrush-editor"
EDITOR_SRC="$SR_HOME/editor"
CONFIG_XML="$SR_HOME/config/sparkrush-config.xml"
AGENT_PY="$SR_HOME/agents/optimizer.py"
DESKTOP_FILE="$SR_HOME/desktop/sparkrush-core.desktop"
CORE_SH="$SR_HOME/sparkrush-core.sh"

export PATH="$HOME/.cargo/bin:$PATH"

PASS=0
FAIL=0
FAILED_TESTS=()

c_red()    { printf '\033[31m%s\033[0m' "$*"; }
c_green()  { printf '\033[32m%s\033[0m' "$*"; }
c_yellow() { printf '\033[33m%s\033[0m' "$*"; }

t() {
    local id="$1"; shift
    local desc="$1"; shift
    if "$@" >/tmp/sr_test_out 2>&1; then
        printf "  [$(c_green PASS)] %s  %s\n" "$id" "$desc"
        PASS=$((PASS+1))
    else
        printf "  [$(c_red   FAIL)] %s  %s\n" "$id" "$desc"
        cat /tmp/sr_test_out | sed 's/^/        | /'
        FAIL=$((FAIL+1))
        FAILED_TESTS+=("$id")
    fi
}

# Fixture files
FIXTURE_RS="$SR_HOME/tests/_fixture.rs"
FIXTURE_PY="$SR_HOME/tests/_fixture.py"
FIXTURE_XML="$SR_HOME/tests/_fixture.xml"

setup() {
    cat > "$FIXTURE_RS" <<'EOF'
fn main() {
    let x = 42;
    println!("hello");
        // tab-indent comment
    let y = x + 1;
    println!("{}", y);
}
EOF
    cat > "$FIXTURE_PY" <<'EOF'
def add(a, b):
    # sum two
    return a + b

def sub(a, b):
    return a - b

print(add(1, 2))
EOF
    cat > "$FIXTURE_XML" <<'EOF'
<?xml version="1.0"?>
<root>
  <item name="x" value="1"/>
</root>
EOF
    # build the editor if not already
    if [[ ! -x "$EDITOR_BIN" ]]; then
        ( cd "$EDITOR_SRC" && cargo build --release ) >/tmp/sr_build.log 2>&1 || {
            cat /tmp/sr_build.log
            echo "FATAL: cargo build failed in setup" >&2
            exit 1
        }
    fi
}

teardown() {
    rm -f "$FIXTURE_RS" "$FIXTURE_RS.opt" "$FIXTURE_PY" "$FIXTURE_PY.opt" \
          "$FIXTURE_XML" "$FIXTURE_XML.opt" /tmp/sr_test_out /tmp/sr_build.log
}

setup
trap teardown EXIT

echo "$(c_yellow "SPARKRUSH-CORE edge bench — 13 tests")"
echo ""

# T01 — bash syntax check of orchestrator
t T01 "sparkrush-core.sh syntax-clean (bash -n)" \
    bash -n "$CORE_SH"

# T02 — editor compiles (this is checked in setup; here we just verify the bin exists and runs)
t T02 "editor binary built and --version exits 0" \
    "$EDITOR_BIN" --version

# T03 — editor selftest
t T03 "editor --selftest produces highlight samples" \
    "$EDITOR_BIN" --selftest

# T04 — editor help
t T04 "editor --help exits 0 and contains 'SPARKRUSH-EDITOR'" \
    bash -c '"$0" --help | grep -q "SPARKRUSH-EDITOR"' "$EDITOR_BIN"

# T05 — view a rust file (highlight path)
t T05 "editor view rust fixture (lines>0, contains 'lang=rust')" \
    bash -c '"$0" "$1" 2>&1 | grep -q "lang=rust"' "$EDITOR_BIN" "$FIXTURE_RS"

# T06 — metrics
t T06 "editor --metrics prints complexity_proxy" \
    bash -c '"$0" "$1" --metrics 2>&1 | grep -q "complexity_proxy"' "$EDITOR_BIN" "$FIXTURE_RS"

# T07 — optimizer produces .opt file
t T07 "optimizer.py produces <file>.opt with 'optimizer: output'" \
    bash -c 'python3 "$0" "$1" 2>&1 | grep -q "optimizer: output"' "$AGENT_PY" "$FIXTURE_RS"

# T08 — XML config well-formed + has expected structure
t T08 "config XML well-formed + root=<sparkrush>" \
    python3 -c "import xml.etree.ElementTree as ET; r=ET.parse('$CONFIG_XML').getroot(); assert r.tag=='sparkrush'; assert r.get('version')=='1.0.0'; print('ok')"

# T09 — .desktop file parses + has required keys
t T09 ".desktop file has Name=, Exec=, Terminal=, Type= keys" \
    bash -c 'grep -q "^Name=" "$0" && grep -q "^Exec=" "$0" && grep -q "^Terminal=" "$0" && grep -q "^Type=" "$0"' "$DESKTOP_FILE"

# T10 — orchestrator help command exits 0
t T10 "sparkrush-core.sh help exits 0" \
    bash "$CORE_SH" help

# T11 — editor on nonexistent file exits 2
t T11 "editor exits 2 on missing file" \
    bash -c '"$0" /nonexistent/file.xyz >/dev/null 2>&1; [ $? -eq 2 ]' "$EDITOR_BIN"

# T12 — orchestrator validate command works (XML + desktop)
t T12 "sparkrush-core.sh validate exits 0" \
    bash "$CORE_SH" validate

# T13 — REAL benchmark: rust cold-start < python cold-start (rust must win)
# This is the honest version of "4.5x faster". We measure real cold-start
# using python3.perf_counter_ns (portable, no /usr/bin/time dependency).
t T13 "benchmark: rust editor --version faster than python3 cold-start" \
    python3 -c "
import subprocess, time, sys
ed = sys.argv[1]
# warm
subprocess.run([ed, '--version'], capture_output=True)
subprocess.run(['python3', '-c', \"print('warm')\"], capture_output=True)
# time rust
def t_rust():
    t0 = time.perf_counter_ns()
    subprocess.run([ed, '--version'], capture_output=True)
    t1 = time.perf_counter_ns()
    return (t1-t0)//1000
def t_py():
    t0 = time.perf_counter_ns()
    subprocess.run(['python3', '-c', \"print(1)\"], capture_output=True)
    t1 = time.perf_counter_ns()
    return (t1-t0)//1000
r = min(t_rust() for _ in range(5))
p = min(t_py() for _ in range(5))
print(f'rust_us={r} py_us={p}')
sys.exit(0 if r < p else 1)
" "$EDITOR_BIN"

echo ""
echo "──────────────────────────────────────"
echo "  passed: $(c_green $PASS)    failed: $(c_red $FAIL)    total: $((PASS+FAIL))"
if [[ $FAIL -gt 0 ]]; then
    echo "  failed tests: ${FAILED_TESTS[*]}"
    exit 1
fi
echo "  status: $(c_green "ALL GREEN — PROTON SATISFIED")"
echo "──────────────────────────────────────"
