#!./testshell

echo 'Test 1: See whether child can work on SIGINT and SIGQUIT without'
echo '        terminating the shell around it. See if the shell is'
echo '        interruptable afterwards'

if [ $ZSH_VERSION ] ; then
    source lib.sh
else
    . ./lib.sh
fi

docatcher
echo 'Now try to exit shell loop with C-c, C-\ or SIGTERM'
endless
