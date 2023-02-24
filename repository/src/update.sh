#!/bin/sh
simperby_executable_path="$SIMPERBY_EXECUTABLE_PATH"
simperby_root_path="$SIMPERBY_ROOT_PATH"
refname="$1"
oldrev="$2"
newrev="$3"

eval "$simperby_executable_path $simperby_root_path notify-push $newrev"
status="$?"
if [ "$status" -ne 0 ]; then 
    echo "notify-push failed"
	exit 1
fi