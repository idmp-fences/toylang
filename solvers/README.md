# ILP and ALNS solvers

## Usage

### ALNS

```
> python .\solvers\run_alns.py --help
usage: run_alns.py [-h] [--initial-state-gen {hot-edges,first-edges,ilp}] [--select {random,roulette-wheel,roulette-wheel-segmented}]
                   [--accept {hill-climbing,late-acceptance-hill-climbing,simulated-annealing}] [--max-runtime MAX_RUNTIME] [--until-objective UNTIL_OBJECTIVE]
                   file_path

ALNS Configuration

positional arguments:
  file_path             Path to the JSON file to load

options:
  -h, --help            show this help message and exit
  --initial-state-gen {hot-edges,first-edges,ilp}
                        Initial state generation method
  --select {random,roulette-wheel,roulette-wheel-segmented}
                        Select method
  --accept {hill-climbing,late-acceptance-hill-climbing,simulated-annealing}
                        Accept method
  --max-runtime MAX_RUNTIME
                        Max runtime of ALNS
  --until-objective UNTIL_OBJECTIVE
                        Run ALNS until this objective is reached
```

### ILP