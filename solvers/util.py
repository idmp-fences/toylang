import json
from typing import List

import msgpack

from aeg import AbstractEventGraph, CriticalCycle


def load_aeg(file_path: str) -> (AbstractEventGraph, List[CriticalCycle]):
    if file_path.endswith(".json"):
        with open(file_path, 'r') as file:
            data = json.load(file)
    elif file_path.endswith(".msgpack"):
        with open(file_path, 'rb') as file:
            data = msgpack.unpackb(file.read())
    else:
        raise ValueError("Cannot parse this file type")

    aeg_data = data["aeg"]
    ccs_data = data["critical_cycles"]
    aeg = AbstractEventGraph(aeg_data['nodes'], aeg_data['edges'])
    critical_cycles = [CriticalCycle(cc['cycle'], cc['potential_fences'], aeg) for cc in ccs_data]

    return aeg, critical_cycles
