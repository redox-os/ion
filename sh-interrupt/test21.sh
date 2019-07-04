#! ./testshell

traph ()
{
	echo 'Survived!'
}
trap traph 2

echo 'You should need 5 SIGINT to end this script'
cat
cat
cat
cat
cat

