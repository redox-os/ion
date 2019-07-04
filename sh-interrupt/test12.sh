#!./testshell

echo 'Test 12 (Variant of test10):'
echo 'This script should not be killable by SIGINT or SIGQUIT'
(while :; do ./catcher ; done)
