#!/bin/sh
push_option_count="$GIT_PUSH_OPTION_COUNT"
value="$GIT_PUSH_OPTION_0"
simperby_executable_path="$SIMPERBY_EXECUTABLE_PATH"
simperby_root_path="$SIMPERBY_ROOT_PATH"

if [ "$push_option_count" -ne 1 ]; then
	echo "The number of push options is not 1"
	exit 1
fi

count="$(echo "$value" | awk '{print NF}')"
if [ "$count" -ne 5 ]; then
    echo "The number of arguments to Cli is not 5"
    exit 1
fi

commit="$(echo "$value" | awk '{print $1}')"
branch_name="$(echo "$value" | awk '{print $2}')"
timestamp="$(echo "$value" | awk '{print $3}')"
signature="$(echo "$value" | awk '{print $4}')"
signer="$(echo "$value" | awk '{print $5}')"

eval "$simperby_executable_path $simperby_root_path check-push $value"
status="$?"
if [ "$status" -ne 0 ]; then
	echo "check-push failed"
	exit 1
fi