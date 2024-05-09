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

The full grammar can be found at [parser/src/toy.pest](parser/src/toy.pest).

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

