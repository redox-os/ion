#! ./testshell

echo "You should be able to end this script by SIGQUIT, but not by SIGINT"
trap : 2
trap 'echo SIGQUIT ; exit 1' 3
./hardguy
