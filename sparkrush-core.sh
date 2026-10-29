#!/usr/bin/env bash
# SPARKRUSH-CORE — bash orchestrator that dispatches to Rust + Python + XML.
# DGK × PROTON protocol. Honest, executable, no fake benchmarks.
#
# This is the main entry point that "appelle les autres langues" (calls the
# other languages) from a single bash binary, per DGK's spec.
set -euo pipefail

# ---- paths ---------------------------------------------------------------
SR_HOME="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EDITOR_BIN="$SR_HOME/editor/target/release/sparkrush-editor"
EDITOR_SRC="$SR_HOME/editor"
CONFIG_XML="$SR_HOME/config/sparkrush-config.xml"
AGENT_PY="$SR_HOME/agents/optimizer.py"
DESKTOP_FILE="$SR_HOME/desktop/sparkrush-core.desktop"
TESTS_SH="$SR_HOME/tests/sparkrush_edge_tests.sh"

# rustup-installed toolchain
export PATH="$HOME/.cargo/bin:$PATH"

VERSION="1.0.0"

# ---- helpers -------------------------------------------------------------
c_red()    { printf '\033[31m%s\033[0m' "$*"; }
c_green()  { printf '\033[32m%s\033[0m' "$*"; }
c_yellow() { printf '\033[33m%s\033[0m' "$*"; }
c_cyan()   { printf '\033[36m%s\033[0m' "$*"; }
c_bold()   { printf '\033[1m%s\033[0m' "$*"; }

die() { c_red "sparkrush: $*"; echo; exit 1; }
say() { c_cyan "sparkrush:"; printf ' %s\n' "$*"; }
ok()  { c_green "✓"; printf ' %s\n' "$*"; }

require_file() { [[ -f "$1" ]] || die "missing file: $1"; }

# ---- commands ------------------------------------------------------------

cmd_version() {
    echo "sparkrush-core v$VERSION (bash orchestrator)"
    echo "  editor:    $EDITOR_BIN"
    rustc --version 2>/dev/null | sed 's/^/  rustc:    /' || true
    python3 --version 2>&1 | sed 's/^/  python:   /'
}

cmd_help() {
    cat <<EOF
$(c_bold "SPARKRUSH-CORE v$VERSION")  — bash orchestrator → Rust + Python + XML

$(c_bold "USAGE")
  sparkrush-core.sh <command> [args]

$(c_bold "COMMANDS")
  init                  scaffold & verify project layout
  build                 compile the Rust editor (cargo build --release)
  run <file>            view file with syntax highlighting (Rust native)
  metrics <file>        print file metrics (lines, bytes, complexity)
  optimize <file>       invoke python optimizer-agent
  validate              validate XML config + .desktop file
  test                  run the 13-edge test bench
  benchmark             REAL benchmark: rust editor cold-start vs python
  gui                   interactive shell menu (used by .desktop launcher)
  desktop-install       install .desktop file to ~/.local/share/applications
  desktop-uninstall     remove .desktop file
  version               print version + toolchain info
  help                  this message

$(c_bold "EXAMPLES")
  sparkrush-core.sh build
  sparkrush-core.sh run editor/src/main.rs
  sparkrush-core.sh metrics agents/optimizer.py
  sparkrush-core.sh optimize editor/src/main.rs
  sparkrush-core.sh test
  sparkrush-core.sh benchmark

$(c_bold "PROTON NOTE")
  No fake benchmarks. No '4.5x faster than Mistral 4'. Every claim has
  an executable test. See docs/AUDIT.md for the deviation report.
EOF
}

cmd_init() {
    say "checking project layout..."
    local missing=0
    for d in editor/src config agents desktop tests docs; do
        if [[ -d "$SR_HOME/$d" ]]; then ok "dir  $d"; else
            c_red "missing dir: $d"; echo; missing=$((missing+1))
        fi
    done
    for f in sparkrush-core.sh editor/Cargo.toml editor/src/main.rs \
             config/sparkrush-config.xml agents/optimizer.py \
             desktop/sparkrush-core.desktop tests/sparkrush_edge_tests.sh \
             docs/AUDIT.md docs/sparkrush_spec_dgk.md; do
        if [[ -f "$SR_HOME/$f" ]]; then ok "file $f"; else
            c_red "missing file: $f"; echo; missing=$((missing+1))
        fi
    done
    [[ $missing -eq 0 ]] || die "init: $missing missing artifacts"
    ok "init complete — project layout verified"
}

cmd_build() {
    say "compiling Rust editor (cargo build --release)..."
    require_file "$EDITOR_SRC/Cargo.toml"
    ( cd "$EDITOR_SRC" && cargo build --release ) || die "cargo build failed"
    [[ -x "$EDITOR_BIN" ]] || die "binary not produced: $EDITOR_BIN"
    ok "editor built → $EDITOR_BIN"
    "$EDITOR_BIN" --version
}

ensure_editor_built() {
    if [[ ! -x "$EDITOR_BIN" ]]; then
        say "editor not built — building now..."
        cmd_build
    fi
}

cmd_run() {
    local f="${1:-}"
    [[ -n "$f" ]] || die "run: missing file argument"
    [[ -f "$f" ]] || die "run: file not found: $f"
    ensure_editor_built
    "$EDITOR_BIN" "$f"
}

cmd_metrics() {
    local f="${1:-}"
    [[ -n "$f" ]] || die "metrics: missing file argument"
    [[ -f "$f" ]] || die "metrics: file not found: $f"
    ensure_editor_built
    "$EDITOR_BIN" "$f" --metrics
}

cmd_optimize() {
    local f="${1:-}"
    [[ -n "$f" ]] || die "optimize: missing file argument"
    [[ -f "$f" ]] || die "optimize: file not found: $f"
    require_file "$AGENT_PY"
    say "invoking python optimizer-agent on: $f"
    python3 "$AGENT_PY" "$f"
    ok "optimizer done — see ${f}.opt for cleaned output"
}

cmd_validate() {
    say "validating XML config..."
    require_file "$CONFIG_XML"
    python3 - <<PY || die "xml config is not well-formed"
import xml.etree.ElementTree as ET
import sys
tree = ET.parse("$CONFIG_XML")
root = tree.getroot()
assert root.tag == "sparkrush", f"root tag is {root.tag}, expected 'sparkrush'"
attrs = root.attrib
print(f"  xml: well-formed, root=<{root.tag} version={attrs.get('version','?')}>")
# print declared family (DGK x GLM, 2026-10-29)
fam = root.find("./family")
if fam is not None:
    members = fam.findall("./member")
    print(f"  family: declared-by={fam.get('declared-by','?')} date={fam.get('date','?')} ({len(members)} members)")
    for m in members:
        print(f"    - {m.get('id')} [{m.get('role')}]: {m.get('note','')}")
# report declared features honestly
feats = root.findall("./features/feature")
impl = sum(1 for f in feats if f.get("status") == "implemented")
notimpl = sum(1 for f in feats if f.get("status") == "not-implemented")
print(f"  features: {impl} implemented, {notimpl} honestly declared not-implemented")
for f in feats:
    if f.get("status") != "implemented":
        print(f"    - {f.get('name')}: {f.get('status')} ({f.get('reason','')})")
PY
    ok "xml valid"

    say "validating .desktop file..."
    require_file "$DESKTOP_FILE"
    python3 - <<PY || die ".desktop file is malformed"
import configparser
p = configparser.ConfigParser(interpolation=None)
p.optionxform = str  # preserve case for Desktop Entry keys
with open("$DESKTOP_FILE") as fh:
    p.read_string(fh.read())
sec = p["Desktop Entry"]
required = ["Type","Name","Exec","Terminal","Categories"]
missing = [k for k in required if k not in sec]
assert not missing, f"missing keys: {missing}"
print(f"  desktop: Name={sec['Name']} Type={sec['Type']} Terminal={sec['Terminal']}")
PY
    ok "desktop valid"
}

cmd_test() {
    require_file "$TESTS_SH"
    bash "$TESTS_SH"
}

cmd_benchmark() {
    say "REAL benchmark — rust editor cold-start vs python cold-start"
    ensure_editor_built

    # warm cache once (filesystem cache)
    "$EDITOR_BIN" --version >/dev/null 2>&1 || true
    python3 -c "print('warm')" >/dev/null 2>&1

    local rust_runs=5 py_runs=5
    local rust_total_us=0 py_total_us=0

    # Portable timing using python3 + time.perf_counter_ns (no /usr/bin/time dependency)
    for i in $(seq 1 $rust_runs); do
        local us
        us=$(python3 -c "
import subprocess, time, sys
t0 = time.perf_counter_ns()
subprocess.run([sys.argv[1], '--version'], capture_output=True)
t1 = time.perf_counter_ns()
print(int((t1-t0)//1000))
" "$EDITOR_BIN")
        rust_total_us=$((rust_total_us + us))
    done
    for i in $(seq 1 $py_runs); do
        local us
        us=$(python3 -c "
import subprocess, time
t0 = time.perf_counter_ns()
subprocess.run(['python3', '-c', \"print('x')\"], capture_output=True)
t1 = time.perf_counter_ns()
print(int((t1-t0)//1000))
")
        py_total_us=$((py_total_us + us))
    done

    local rust_avg=$((rust_total_us / rust_runs))
    local py_avg=$((py_total_us / py_runs))
    local ratio
    ratio=$(awk -v r="$rust_avg" -v p="$py_avg" 'BEGIN{ if(r>0) printf "%.1f", p/r; else print "n/a" }')

    echo
    echo "  rust editor cold-start (--version):  avg ${rust_avg} µs  (n=$rust_runs)"
    echo "  python3 cold-start (print('x')):     avg ${py_avg} µs  (n=$py_runs)"
    echo "  ratio (python/rust):                 ${ratio}x"
    echo
    say "honest reading: rust is ${ratio}x faster than python on cold-start."
    say "this is NOT a comparison to Mistral 4 (which is not installed here)."
    ok "benchmark done"
}

cmd_gui() {
    cat <<EOF
$(c_bold "SPARKRUSH-CORE v$VERSION")  — bureau, pas chrome

  1) build         compiler l'éditeur Rust
  2) run <file>    voir un fichier avec coloration
  3) metrics       métriques d'un fichier
  4) optimize      agent Python regex
  5) validate      valider XML + .desktop
  6) test          banc de 13 tests
  7) benchmark     benchmark réel rust vs python
  8) help          aide complète
  q) quitter

EOF
    while true; do
        read -r -p "sparkrush> " choice
        case "$choice" in
            1) cmd_build ;;
            2) read -r -p "file? " f; cmd_run "$f" ;;
            3) read -r -p "file? " f; cmd_metrics "$f" ;;
            4) read -r -p "file? " f; cmd_optimize "$f" ;;
            5) cmd_validate ;;
            6) cmd_test ;;
            7) cmd_benchmark ;;
            8) cmd_help ;;
            q|Q|quit|exit) echo "bye"; break ;;
            "") ;;
            *) c_red "unknown: $choice"; echo ;;
        esac
    done
}

cmd_desktop_install() {
    require_file "$DESKTOP_FILE"
    local dest_dir="$HOME/.local/share/applications"
    mkdir -p "$dest_dir"
    # patch the Exec= line to point to the absolute path of this script
    local patched="$dest_dir/sparkrush-core.desktop"
    sed -e "s|__SR_HOME__|$SR_HOME|g" "$DESKTOP_FILE" > "$patched"
    chmod +x "$patched"
    ok "desktop file installed: $patched"
    say "SparkRush Core should now appear in your GNOME app launcher."
    say "if it doesn't, run: update-desktop-database ~/.local/share/applications"
}

cmd_desktop_uninstall() {
    local f="$HOME/.local/share/applications/sparkrush-core.desktop"
    if [[ -f "$f" ]]; then rm -f "$f"; ok "removed: $f"; else
        say "no desktop file to remove"; fi
}

# ---- dispatch ------------------------------------------------------------
main() {
    local cmd="${1:-help}"
    shift || true
    case "$cmd" in
        init)              cmd_init "$@" ;;
        build)             cmd_build "$@" ;;
        run)               cmd_run "$@" ;;
        metrics)           cmd_metrics "$@" ;;
        optimize)          cmd_optimize "$@" ;;
        validate)          cmd_validate "$@" ;;
        test)              cmd_test "$@" ;;
        benchmark)         cmd_benchmark "$@" ;;
        gui)               cmd_gui "$@" ;;
        desktop-install)   cmd_desktop_install "$@" ;;
        desktop-uninstall) cmd_desktop_uninstall "$@" ;;
        version|--version) cmd_version "$@" ;;
        help|--help|-h)    cmd_help "$@" ;;
        *) die "unknown command: $cmd  (try: sparkrush-core.sh help)" ;;
    esac
}

main "$@"
