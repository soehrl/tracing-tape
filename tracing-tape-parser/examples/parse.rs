use tracing_tape_parser::Tape;

fn main() {
    let now = std::time::Instant::now();
    let file = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    println!("Read file in {:?}", now.elapsed());

    let now = std::time::Instant::now();
    let tape = Tape::parse(&file);
    println!("Parsed tape in {:?}", now.elapsed());

    println!("Duration: {:?}", tape.timestamp_range());
}
