#!/usr/bin/env bash
# Resident-memory watchdog for benchmarks.
#
#   LIMIT_MB=4096 TIMEOUT_S=600 POLL_MS=50 scripts/bench-guard.sh <cmd> [args...]
#
# Polls the spawned process tree by walking /proc directly (no subprocesses
# in the hot loop). When combined RSS exceeds LIMIT_MB the tree receives
# SIGKILL; exit code 3 reports the peak. TIMEOUT_S produces exit code 4.
# Normal completion forwards the child status.
set -uo pipefail

LIMIT_MB="${LIMIT_MB:-4096}"
TIMEOUT_S="${TIMEOUT_S:-600}"
POLL_MS="${POLL_MS:-50}"

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <cmd> [args...]" >&2
    exit 64
fi

# Own process group so a group-kill can never reach this guard or its shell.
setsid "$@" &
CHILD=$!
START=$(date +%s)
PEAK_KB=0

declare -a P_PID P_PPID P_RSS

snapshot_procs() {
    P_PID=(); P_PPID=(); P_RSS=()
    local dir pid line rest ppid rss pages
    for dir in /proc/[0-9]*; do
        pid=${dir#/proc/}
        [[ -r $dir/stat && -r $dir/statm ]] || continue
        IFS= read -r line < "$dir/stat" 2>/dev/null || continue
        rest=${line#*) }
        read -r _ ppid _ <<< "$rest"
        read -r _ pages _ < "$dir/statm" 2>/dev/null || continue
        P_PID+=("$pid")
        P_PPID+=("$ppid")
        P_RSS+=("$((pages * 4))")
    done
}

tree_rss_kb() {
    local root=$1 total=0 i n changed p
    n=${#P_PID[@]}
    local -i included=0
    declare -A in_set=()
    in_set[$root]=1
    changed=1
    while ((changed)); do
        changed=0
        for ((i = 0; i < n; i++)); do
            p=${P_PID[i]}
            [[ -n ${in_set[p]:-} ]] && continue
            if [[ -n ${in_set[${P_PPID[i]}]:-} ]]; then
                in_set[p]=1
                ((total += P_RSS[i]))
                included=1
                changed=1
            fi
        done
        # The root itself may have execed into its program; count it once.
        if ((included == 0)); then
            for ((i = 0; i < n; i++)); do
                [[ ${P_PID[i]} == "$root" ]] && ((total += P_RSS[i]))
            done
            break
        fi
    done
    echo "$total"
}

kill_tree() {
    local pgid
    pgid=$(awk '{print $5}' "/proc/$1/stat" 2>/dev/null)
    if [[ -n "${pgid:-}" ]]; then
        kill -KILL "-$pgid" 2>/dev/null
    fi
    kill -KILL "$1" 2>/dev/null
}

while kill -0 "$CHILD" 2>/dev/null; do
    NOW=$(date +%s)
    if ((NOW - START >= TIMEOUT_S)); then
        echo "GUARD: timeout ${TIMEOUT_S}s exceeded (peak ${PEAK_KB} kB)" >&2
        kill_tree "$CHILD"
        wait "$CHILD" 2>/dev/null
        exit 4
    fi
    snapshot_procs
    KB=$(tree_rss_kb "$CHILD")
    ((KB > PEAK_KB)) && PEAK_KB=$KB
    if ((KB > LIMIT_MB * 1024)); then
        echo "GUARD: RSS ${KB} kB exceeded ${LIMIT_MB} MB limit; killed process tree" >&2
        echo "GUARD: peak was ${PEAK_KB} kB" >&2
        kill_tree "$CHILD"
        wait "$CHILD" 2>/dev/null
        exit 3
    fi
    sleep "0.${POLL_MS}"
done

wait "$CHILD"
STATUS=$?
echo "GUARD: finished normally (peak ${PEAK_KB} kB)"
exit "$STATUS"
