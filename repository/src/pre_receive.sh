#!/bin/sh
eval "count=\$GIT_PUSH_OPTION_COUNT"

if [ "$count" != "0" ]
then
	i=0
	while [ "$i" -lt "$count" ]
	do
		eval "value=\$GIT_PUSH_OPTION_$i"
		case "$value" in
		accept=*)
			cd ${value#*=}
			result=$(./simperby_true.sh)
			if [ "$result" = false ]
			then exit 1
			fi
            ;;
		reject=*)
			cd ${value#*=}
			result=$(./simperby_false.sh)
			if [ "$result" = false ]
			then exit 1
			fi
			;;
        *)
			echo "echo from the pre-receive-hook: ${value}" >&2
			;;
		esac
		i=$((i + 1))
	done
else 
	echo "There is no push options."
	exit 1		
fi