for ($i = 30; $i -le 30; $i++) {
    $fences = python helper.py "random-d4-gurobi.csv" "programs/random-2-10-2-depth-4/random-2-10-2-$i.msgpack"
    for ($j = 0; $j -le 2; $j++) {
        python solvers/run_alns.py "programs/random-2-10-2-depth-4/random-2-10-2-$i.msgpack" --select roulette-wheel -q  --until-objective $fences --max-runtime 600 >> output.txt
    }
}