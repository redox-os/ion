#!./testshell 

echo 'Test 5: The SIGINT/SIGQUIT-catching program is being run'
echo '     A: The shell should not exit on signals while this program runs.'
echo '     B: After you exited it via C-d, you should be able to end this'
echo '        script with its 3 subhells with just one signal'
echo '        script with just one SIGINT or SIGQUIT'

if [ $ZSH_VERSION ] ; then
    source lib.sh
else
    . ./lib.sh
fi

(
    (
	docatcher
	echo "Now try to exit with one SIGINT"
	endless
    )
    endless
)
endless
