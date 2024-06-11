import json
import sys

from ilp import ILPSolver, AbstractEventGraph, CriticalCycle


def main():
    input_json = json.load(sys.stdin)
    aeg_data = input_json["aeg"]
    ccs_data = input_json["critical_cycles"]

    aeg = AbstractEventGraph(aeg_data['nodes'], aeg_data['edges'])
    critical_cycles = [CriticalCycle(cc['cycle'], cc['potential_fences'], aeg) for cc in ccs_data]

    print("AEG:", aeg)
    print("Critical Cycles:", critical_cycles)

    ILPSolver(aeg, critical_cycles).fence_placement()

    print("AEG:", aeg)

if __name__ == "__main__":
    main()
