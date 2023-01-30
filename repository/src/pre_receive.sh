#!/bin/sh
eval "count=\$GIT_PUSH_OPTION_COUNT"
eval "simperby_path=\$SIMPERBY_PATH"
eval "value=\$GIT_PUSH_OPTION_0"

read oldRev newRev refname
branch="$(echo $refname | awk '{split($0,a,"/"); print a[3]}')"

if [ $count != 1 ]
then
	echo "The number of push option is not 1"
	exit 1
fi

result=$($simperby_path $value $branch)
if [ $result = false ]
then exit 1
fi