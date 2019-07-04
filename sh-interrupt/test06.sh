#!./testshell

echo 'Test 2: You should not be able to exit `cat` with SIGQUIT.'
echo '        SIGINT and SIGTERM should exit the whole script.'

set -x
trap '' 3
while : ; do cat ; echo -n $? ; done
