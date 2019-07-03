#! ./testshell

echo "This script should be breakable by SIGINT if you run a shell with"
echo "asynchrnous traps enabled. Examples: FreeBSD's sh with switch -T"
echo "from April, 1999 or FreeBSD's sh between September 1998 and March"
echo "1999. SIGQUIT should do nothing"
trap : 3
trap 'echo SIGINT ; exit 1' 2
./hardguy
