#!./testshell

echo 'Test 2: You should not be able to exit `cat` with SIGINT.'
echo '        SIGQUIT should abort `cat` (with coredump) while'
echo '        the shell should continue and call `cat` again.'
echo '        SIGTERM should exit the whole script.'

set -x
trap '' 2
while : ; do cat ; echo -n $? ; done
