docatcher() {
    echo 'Trigger some async actions, shell should not exit'
    echo 'Then exit catcher with C-d'
    if [ ! -f ./catcher ]; then
	    make catcher
    fi
    ./catcher
}

endless() {
    while : ; do foo=a; done
}
