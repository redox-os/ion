#! ./testshell

echo "This script is the same as the last, but it enables the asynchronous"
echo "trap switch (-T) in FreeBSD's sh from April 1999."
echo "Other shells should not exit on SIGINT or SIGQUIT."
set -T
trap : 3
trap 'echo SIGINT ; exit 1' 2
./hardguy
