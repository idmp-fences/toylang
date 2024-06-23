for ($i = 0; $i -le 50; $i++) {
    $fences = python helper.py "./results/random-d4-gurobi.csv" "programs/random-2-10-2-depth-4/random-2-10-2-$i.msgpack"
    for ($j = 0; $j -le 4; $j++) {
        Write-Host "Running Program $i ($($j+1)/5)"
        python solvers/run_alns.py "programs/random-2-10-2-depth-4/random-2-10-2-$i.msgpack" --select roulette-wheel -q --until-objective $fences --max-runtime 600 >> output.txt
    }
}