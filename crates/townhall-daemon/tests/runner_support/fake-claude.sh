#!/bin/sh
dir=$(dirname "$0")
n=$(( $(cat "$dir/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$dir/count.tmp-$$" && mv "$dir/count.tmp-$$" "$dir/count"
printf '%s\0' "$@" > "$dir/argv.tmp-$n" && mv "$dir/argv.tmp-$n" "$dir/argv-$n"
pwd -P > "$dir/cwd.tmp-$n" && mv "$dir/cwd.tmp-$n" "$dir/cwd-$n"
echo "$$" > "$dir/pid-$n"
env > "$dir/env.tmp-$n" && mv "$dir/env.tmp-$n" "$dir/env-$n"
IFS= read -r first || exit 4
printf '%s\n' "$first" > "$dir/stdin.tmp-$n" && mv "$dir/stdin.tmp-$n" "$dir/stdin-$n"
log() { printf '%s\n' "$1" >> "$dir/stdin-$n"; }
lost() { echo "No conversation found with session ID: $1" >&2; exit 1; }
case "$first" in
  *CRASH*) echo 'warming up' >&2; echo 'boom: claude broke' >&2; exit 3 ;;
  *ALWAYS*) lost any ;;
  *STALE*) case " $* " in *" --resume="*) lost resumed ;; esac ;;
esac
echo '{"type":"system","subtype":"init","session_id":"fake","tools":[]}'
case "$first" in
  *KIDS*|*ESCAPE*)
    case "$first" in
      *ESCAPE*)
        perl -e 'setpgrp(0,0); open F,">",shift; print F $$; close F; exec @ARGV' "$dir/kid-$n" sleep 31 &
        while [ ! -s "$dir/kid-$n" ]; do sleep 0.05; done ;;
      *) sleep 31 & echo "$!" > "$dir/kid-$n" ;;
    esac
    while IFS= read -r line; do log "$line"; done
    exit 0 ;;
  *SLOW*|*HANG*)
    while IFS= read -r line; do
      log "$line"
      case "$line" in
        *'"interrupt"'*)
          case "$first" in *HANG*) continue ;; esac
          echo '{"type":"control_response","response":{"subtype":"success","request_id":"x"}}'
          echo '{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"aborted_streaming"}'
          cat >> "$dir/stdin-$n"
          exit 0 ;;
      esac
    done
    exit 0 ;;
  *ERRAFTER*)
    echo '{"type":"result","subtype":"error_during_execution","is_error":true,"result":"No conversation found with session ID: z"}'
    cat >> "$dir/stdin-$n"
    exit 1 ;;
  *TOOL*)
    echo '{"type":"control_request","request_id":"req-edit","request":{"subtype":"can_use_tool","tool_name":"Edit","input":{"file_path":"/x","old_string":"a","new_string":"b"},"tool_use_id":"toolu_1"}}'
    IFS= read -r answer; log "$answer"
    echo '{"type":"control_request","request_id":"req-read","request":{"subtype":"can_use_tool","tool_name":"Read","input":{"file_path":"/x"},"tool_use_id":"toolu_2"}}'
    IFS= read -r answer; log "$answer" ;;
esac
say='on it.'
case "$first" in
  *MENTION*) say='the app logs say: No conversation found with session ID 42'; echo "$say" >&2 ;;
esac
echo '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"on"}}}'
echo '{"type":"assistant","message":{"model":"claude-test","role":"assistant","content":[{"type":"text","text":"'"$say"'"}],"usage":{"input_tokens":1200,"cache_creation_input_tokens":300,"cache_read_input_tokens":500,"output_tokens":20}}}'
denials='[]'
case "$first" in
  *DENIAL*) denials='[{"tool_name":"Bash","tool_use_id":"t1","tool_input":{"command":"rm -rf /"}},{"tool_name":"Edit","tool_use_id":"t2","tool_input":{"file_path":"/x"}}]' ;;
esac
echo '{"type":"result","subtype":"success","is_error":false,"result":"'"$say"'","total_cost_usd":0.0123,"permission_denials":'"$denials"',"usage":{"input_tokens":5000,"output_tokens":40},"modelUsage":{"claude-test":{"contextWindow":200000}}}'
cat >> "$dir/stdin-$n"
