#!./testshell

echo 'Test 11 (variant of 9):'
echo 'Try to break wait using SIGINT before wc completes' 
echo 'After you break wait, it should NOT print "Going on"'

echo 'wc &'
gzip < /kernel | wc &
p=$!
echo wait
wait
echo "Going on"
kill $p
