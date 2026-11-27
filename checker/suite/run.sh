#!/bin/sh
# Runs every case in checker/suite against the checker, each in a fresh copy of
# the repo, and fails unless every verdict is what the case expects.
# Usage (from the repo root): scripts/suite.sh, which runs this in the build image.
# expect: line 1 stage (or ACCEPTED), line 2 what the attack is, optional
# line 3 text the checker output must contain (alternatives split by " | "),
# optional line 4 a metric check, `<metric> <awk comparison>`, e.g. `visual_change > 0`.
# SUITE_ONLY=pt_ runs a subset.
# files/ is laid over a fresh copy of the repo; an optional `changed` file
# replaces the generated `A<tab>path` list, for cases that edit or delete.
set -u
repo=$(pwd)
cargo build -q --release --locked --offline -p checker || exit 2
export CARGO_TARGET_DIR="$repo/target/suite"
# A pristine copy stands in for main, so the ratchet has something to compare with.
base=$(mktemp -d)
tar -C "$repo" --exclude=./target --exclude=./.git --exclude=./checker/suite -cf - . | tar -C "$base" -xf -
pass=0; fail=0
for dir in checker/suite/${SUITE_ONLY:-}*/; do
  name=$(basename "$dir")
  want=$(head -n 1 "$dir/expect")
  because=$(sed -n 3p "$dir/expect")
  work=$(mktemp -d)
  tar -C "$repo" --exclude=./target --exclude=./.git --exclude=./checker/suite -cf - . | tar -C "$work" -xf -
  if [ -f "$dir/changed" ]; then
    cp "$dir/changed" "$work/changed.txt"
  else
    (cd "$dir/files" && find . -type f -o -type l | sed 's|^\./|A	|') > "$work/changed.txt"
  fi
  cp -a "$dir/files/." "$work/"
  (cd "$work" && "$repo/target/release/checker" --changed changed.txt --base "$base" --out verdict.json > out.txt 2>&1)
  got=$(sed -n 's/.*"verdict":"\([A-Z]*\)","stage":"\([a-z]*\)".*/\1 \2/p' "$work/verdict.json")
  case "$want" in
    ACCEPTED) ok=$([ "${got%% *}" = ACCEPTED ] && echo y) ;;
    *) ok=$([ "$got" = "REJECTED $want" ] && echo y) ;;
  esac
  reason=$(grep -m 1 '^  - ' "$work/out.txt" | cut -c 5- | head -c 100)
  # Rejected for the reason the case is about, not by accident. Alternatives are
  # separated by " | " where timing decides which rule fires first.
  if [ -n "$because" ]; then
    hit=""
    rest="$because"
    while [ -n "$rest" ]; do
      alt=${rest%% | *}
      grep -qF -- "$alt" "$work/out.txt" && hit=y
      [ "$alt" = "$rest" ] && rest="" || rest=${rest#* | }
    done
    [ -n "$hit" ] || ok=""
  fi
  check=$(sed -n 4p "$dir/expect")
  if [ -n "$check" ]; then
    v=$(sed -n "s/.*\"${check%% *}\":\"\([0-9.]*\)\".*/\1/p" "$work/verdict.json")
    [ -n "$v" ] && awk "BEGIN { exit !($v ${check#* }) }" || ok=""
    reason="${check%% *} = ${v:-none}"
  fi
  if [ "${ok:-}" = y ]; then
    pass=$((pass + 1)); printf 'ok    %-24s %-18s %s\n' "$name" "$got" "$reason"
  else
    fail=$((fail + 1)); printf 'FAIL  %-24s want %s, got %s\n' "$name" "$want${check:+, $check}" "$got${check:+, $reason}"; sed 's/^/      /' "$work/out.txt" | head -20
  fi
  rm -rf "$work"
done
rm -rf "$base"
echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
