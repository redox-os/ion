#!./testshell

echo 'Test 13 (variant of Test 1):'
echo 'On SIGINT, cat should exit (and be restarted by the shell loop)'
echo 'and the Text "I am a trap" should be printed'

set -x
trap 'echo I am a trap' 2
while : ; do cat ; echo -n $? ; done
