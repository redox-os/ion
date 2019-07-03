#! /bin/sh

# Don't ask what is the right thing here...

IFS="   :"
var="bla:fasel:blubb:"
for i in foo:bla:fasel:blubb: ; do
    echo val: "'"$i"'"
done
echo
for i in foo:$var ; do
    echo val: "'"$i"'"
done

