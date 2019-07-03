#!./testshell

trap 'echo I am a trap' 2

echo 'Try to break wait using SIGINT before wc completes'
echo 'After you break wait, it should print "I am a trap"'
echo 'and then "Going on"'
echo 'wc &'
wc /dev/zero &
p=$!
echo wait
wait
echo "Going on"
kill $p
