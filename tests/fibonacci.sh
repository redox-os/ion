fib () {
    if test $1 -le 1; then
        echo $1
    else
        output=1
        previous=0
        index=2
        end=$(($i+1))

        while test $index -ne $end; do
            temp=$output
            output=$((output + $previous))
            previous=$temp
            index=$(($index + 1))
        done
        echo $output
    fi
}

i=1
while test $i -ne 200; do
    fib $i
    i=$((i+1))
done
