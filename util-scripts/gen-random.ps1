for ($i = 9; $i -le 50; $i++) {
    cargo run -p generator random 2 10 2 --max-depth=4 -o "programs/random/random-2-10-2-$i.toy"
    ./target/release/toy.exe find-cycles "programs/random/random-2-10-2-$i.toy" -f=message-pack -o "programs/random/random-2-10-2-$i.msgpack"
}