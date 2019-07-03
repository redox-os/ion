#!./testshell

echo 'Test 3: A background job is being started, then the shell loops.'
echo '        You should be able to break the shell loop with SIGINT.'
echo '        This goes wrong if the shell blocks signals when'
echo '        starting any child. It should do so only for foreground'
echo '        jobs.'
echo '        Make sure you type SIGINT before wc completes'

. ./lib.sh

echo Starting job
tar cvf - --xz ../target/debug/ion | tar xvfJ - | wc &
echo 'Now try to break this loop'
endless
