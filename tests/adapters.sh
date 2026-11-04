#!/bin/sh
# Integration test: isolated tmux server, real Kakoune, no user configuration.
set -eu
root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd)
PATH="$root/target/debug:$root/adapters:$PATH"
export PATH
socket=sumi-test-$$
session=sumi-kak-test-$$
work=$(mktemp -d)
cleanup() { tmux -L "$socket" kill-server 2>/dev/null || :; rm -rf "$work"; }
trap cleanup EXIT HUP INT TERM
wait_screen() {
    count=0
    until tmux -L "$socket" capture-pane -p -t check | rg -q "$1"; do
        count=$((count+1))
        [ "$count" -lt 100 ] || { tmux -L "$socket" capture-pane -p -t check; exit 1; }
        sleep 0.05
    done
}
kak_cmd() { printf '%s\n' "evaluate-commands -client client0 %{ $1 }" | kak -p "$session"; }
tmux -L "$socket" new-session -d -s check -x 120 -y 30 "kak -n -s $session -e 'source $root/adapters/sumi.kak'"
sleep 0.1
tmux -L "$socket" send-keys -t check i C-k
wait_screen 'SKK on'
tmux -L "$socket" send-keys -t check -l Kanji
tmux -L "$socket" send-keys -t check Space Enter
tmux -L "$socket" send-keys -t check -l kana
tmux -L "$socket" send-keys -t check Escape
wait_screen 'SKK off'
kak_cmd "write! $work/actual"
sleep 0.1
printf '漢字かな\n' > "$work/expected"
cmp "$work/expected" "$work/actual"
# Two selections share one engine; confirmed output is inserted at each selection.
kak_cmd 'edit -scratch multiple; execute-keys iA<ret>B<esc>; select 1.1,1.1 2.1,2.1'
sleep 0.1
tmux -L "$socket" send-keys -t check i C-k
wait_screen 'SKK on'
tmux -L "$socket" send-keys -t check -l ka
tmux -L "$socket" send-keys -t check Escape
wait_screen 'SKK off'
kak_cmd "write! $work/multiple"
sleep 0.1
[ "$(rg -o 'か' "$work/multiple" | wc -l)" -ge 2 ]
# Standalone TUI on an actual PTY, including output-only stdout and cancellation.
tmux -L "$socket" new-window -t check -n tui "skk tui > '$work/tui'"
sleep 0.1
tmux -L "$socket" send-keys -t check:tui -l Kanji
tmux -L "$socket" send-keys -t check:tui Space Enter C-d
sleep 0.2
printf '漢字' > "$work/expected"
cmp "$work/expected" "$work/tui"
tmux -L "$socket" new-window -t check -n cancel "skk tui > '$work/cancel'"
sleep 0.1
tmux -L "$socket" send-keys -t check:cancel -l ka
tmux -L "$socket" send-keys -t check:cancel Escape
sleep 0.2
[ ! -s "$work/cancel" ]
# tmux buffer transport targets a stable pane ID, not the active pane.
tmux -L "$socket" new-window -d -t check -n receiver 'cat'
pane=$(tmux -L "$socket" display-message -p -t check:receiver '#{pane_id}')
tmux -L "$socket" load-buffer -b sumi-test "$work/expected"
tmux -L "$socket" paste-buffer -d -b sumi-test -t "$pane"
sleep 0.1
tmux -L "$socket" capture-pane -p -t "$pane" | rg -q '漢字'
# Closing the editor while SKK is active must terminate and clean up the bridge.
tmux -L "$socket" select-window -t check:0
tmux -L "$socket" send-keys -t check:0 i C-k
wait_screen 'SKK on'
kak_cmd "echo -to-file $work/directory %opt{skk_directory}"
sleep 0.1
directory=$(cat "$work/directory")
[ -d "$directory" ]
kak_cmd 'quit!'
count=0
while [ -d "$directory" ]; do
    count=$((count+1))
    [ "$count" -lt 100 ] || { echo 'Kakoune bridge leaked' >&2; exit 1; }
    sleep 0.05
done
printf 'Kakoune conversion, burst/exit, multiple selections, TUI and tmux transport: OK\n'
