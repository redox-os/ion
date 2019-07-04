#! ./testshell

traph ()
{
	trap 2
	kill -2 $$
	echo 'Survived!'
}

echo 'You should be able to kill this script with just one SIGINT'
cat
cat
cat
cat
cat

