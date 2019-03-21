#!/bin/sh

git checkout origin/master
cargo bench
cargo build --release
PREV_SIZE=$(ls -al target/release/ion | cut -d' ' -f5)
git reset --hard HEAD
git checkout -
cargo bench
cargo build --release
SIZE=$(ls -al target/release/ion | cut -d' ' -f5)

# if lower_bound*upper_bound > 0, then we consider the benchmark "changed"
NOISE=0.05
JQ_FILTER="if .Median.confidence_interval.lower_bound > $NOISE or .Median.confidence_interval.upper_bound < -$NOISE then .Median.point_estimate else \"\" end"

total=0
total_worse=0
result=""

for suite in ./target/criterion/*; do
    name=$(echo $suite | cut -d'/' -f 4)
    worse=0
    tests=0

    testcases=""

    for test in $suite/*/*/change/estimates.json; do
        estimate=$(cat "$test" | jq -r "$JQ_FILTER" -c)
        case "$estimate" in
            -*)
                inner="<failure message=\"Performance Regressed\" type=\"WARNING\">\
                    Performance regressed by $estimate in $test\
                </failure>"
                worse=$((worse+1))
            ;;
        esac
        testcases="$testcases<testcase id=\"$(echo "$test" | cut -d'/' -f 6)\" name=\"$(echo "$test" | cut -d'/' -f 6)\">$inner</testcase>"
        tests=$((tests+1))
    done

    result="$result<testsuite id=\"$name\" name=\"$name\" tests=\"$tests\" failures=\"$worse\">$testcases</testsuite>"

    total_worse=$((total_worse + worse))
    total=$((total + tests))
done

binary=$(test $(echo "$PREV_SIZE * 105 / 100" | bc) -ge $SIZE; echo $?)
result="$result\
<testsuite id=\"size\" name=\"Binary size\" tests=\"1\" failures=\"$binary\">\
<testcase id=\"size\" name=\"Binary size\">"

total=$((total + 1))
if [ ! "$binary" -eq "0" ]; then
    result="$result\
    <failure message=\"Binary size increased\" type=\"WARNING\">\
        Binary size increased from $PREV_SIZE to $SIZE.\
    </failure>"
    total_worse=$((total_worse + 1))
fi

result="$result</testcase></testsuite>"

result="<?xml version=\"1.0\" encoding=\"UTF-8\" ?>
<testsuites id=\"$(date +%s)\" name=\"Performances\" tests=\"$total\" failures=\"$total_worse\">
$result
</testsuites>"

echo $result > target/report.xml

exit $(test "$total_worse" -eq "0"; echo $?)
