#!/bin/sh
eval "count=\$GIT_PUSH_OPTION_COUNT"
eval "simperby_path=\$SIMPERBY_PATH"

if [ "$count" != 0 ]
then
	i=0
	while [ "$i" -lt "$count" ]
	do
		eval "value=\$GIT_PUSH_OPTION_$i"
		case "$value" in
		reject)
			cd ${simperby_path}
			result=$(./simperby_false.sh)
			if [ "$result" = false ]
			then exit 1
			fi
			;;
        *)
			cd ${simperby_path}
			result=$(./simperby_true.sh)
			if [ "$result" = false ]
			then exit 1
			fi
            ;;
		esac
		i=$((i + 1))
	done
else 
	echo "There is no push option."
	exit 1		
fi