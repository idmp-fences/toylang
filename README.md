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

## Build 

For a more optimized executable, run the following build command with the environment variable `RUSTFLAGS="-C target-cpu=native"` (done by default in `.cargo/config.toml`):

```
cargo build --release -p toy
```

The executable will be built in `./target/release/toy`

## Compile and run a .toy program

```
./toy.exe run test.toy
```

## Generate the AEG and critical cycles for a .toy program

```
./toy.exe find-cycles test.toy
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
First set up a local python environment by running:
```
pipenv install
```

To run an experiment use the command:
./run_experiment.sh <program_path> <ALNS|ILP>
If the file does not have run permissions, simply tun:
chmod +x ./run_experimens.sh

To run multiple experiments and save results to a file, run:
./run_all.sh
If the file does not have run permissions, simply tun:
chmod +x ./run_all.sh
To modify the specifications of the experiments, simply modify the filenames and run_types arrays in the script