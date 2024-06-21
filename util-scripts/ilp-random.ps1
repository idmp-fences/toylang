for ($i = 31; $i -le 50; $i++) {
    python solvers/run_ilp.py "programs/random-2-10-2-depth-4/random-2-10-2-$i.msgpack" -q --solver 'cbc' >> output.jsonl
}