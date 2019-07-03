#! ./testshell

echo "On SIGINT, this script should print a line and NIT exit, on SIGQUIT, it should exit"
trap 'echo This should not just display something, not exit' 2
trap 'echo SIGQUIT ; exit 1' 3
#trap : 3
./hardguy

