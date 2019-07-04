#! ./testshell



trap 'echo trap ; trap 2 ; kill -2 $$' 2

#echo called is pid $$
echo 'test23, variant of 20, differs in that the trap handler is defined'
echo '        without a shell function'
echo 'You should be able to kill this script with just one SIGINT'
cat
cat
cat
cat
cat

