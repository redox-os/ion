#! ./testshell

traph ()
{
	trap 2
	kill -2 $$
	echo 'Error, survived!'
	sleep 1
	echo 'Survived even longer.'
}
trap traph 2

#echo called is pid $$
echo 'You should be able to kill this script with just one SIGINT'
cat
cat
cat
cat
cat

