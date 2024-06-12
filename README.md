# ***Toy***: a language for *Optimizing Fence Placement in TSO*

```rust
let x: u32 = 0;
let y: u32 = 0;
thread t1 {
    x = 1;
    Fence(WR);
    let a: u32 = x;
}
thread t2 {
    y = 1;
    Fence(WR);
    let b: u32 = x;
}
final {
    assert( !( t1.a == 0 && t2.b == 0 ) );
}
```

[Pest](https://pest.rs/) is used to parse the *toy* language.
The full grammar can be found at [parser/src/toy.pest](parser/src/toy.pest).

## Compile and run a .toy program

```
cargo run -p toy -- test.toy
```

## Documentation

```
cargo doc --open --no-deps
```

## Testing

```
cargo test
```

## Linting

```
cargo clippy
```

or, with pedantic lints

```
cargo clippy -- -W clippy::pedantic
```

## Experiments
To run an experiment use the command:
./run_experiment.sh <program_path> <ALNS|ILP>
If the file does not have run permissions, simply tun:
chmod +x ./run_experimens.sh

To run multiple experiments and save results to a file, run:
./run_all.sh
If the file does not have run permissions, simply tun:
chmod +x ./run_all.sh
To modify the specifications of the experiments, simply modify the filenames and run_types arrays in the script