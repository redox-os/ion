#! ./testshell

#echo Caller is pid $$
./test23b.sh
echo Survived, should not happen. Exit code: $?

