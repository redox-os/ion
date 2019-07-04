#!./testshell 

echo 'Test 4: Three shells all loop. You should be able to terminate this'
echo '        script with just one SIGINT or SIGQUIT'

if [ $ZSH_VERSION ] ; then
    source lib.sh
else
    . ./lib.sh
fi

(
    (
	endless
    )
    endless
)
endless
