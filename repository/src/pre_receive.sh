#!/bin/sh
eval "count=\$GIT_PUSH_OPTION_COUNT"
eval "simperby_path=\$SIMPERBY_PATH"

if [ $count = 1 ]
then
	eval "value=\$GIT_PUSH_OPTION_0"
	cd ${simperby_path}
	result=$(./simperby_cli_example.sh $value)
	if [ $result = false ]
	then exit 1
	fi
else 
	echo "The number of push option is not 1."
	exit 1		
fi