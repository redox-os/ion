#! ./testshell

traph ()
{
	trap 2
	kill -2 $$
	echo 'Error, survived!'
	sleep 1
	echo 'Survived even longer.'
}
echo 'You should be able to end the script with just one SIGINT'
trap traph 2
./hardguy

