#!/bin/sh
value="$GIT_PUSH_OPTION_0"
simperby_executable_path="$SIMPERBY_EXECUTABLE_PATH"
simperby_root_path="$SIMPERBY_ROOT_PATH"
branch_name="$(echo "$value" | awk '{print $2}')"

eval "$simperby_executable_path $simperby_root_path after-push $branch_name"
status="$?"
if [ "$status" -ne 0 ]; then
    echo "after-push failed"
	exit 1
fi