//! Simple greeting CLI example
//!
//! Run with: cargo run -p ralph-cli --example greeter -- <name>
//!
//! Examples:
//!   cargo run -p ralph-cli --example greeter -- Alice
//!   cargo run -p ralph-cli --example greeter -- "Bob Smith"

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: greeter <name>");
        process::exit(1);
    }

    let name = &args[1];
    println!("{}", greet(name));
}

fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet_simple_name() {
        assert_eq!(greet("Alice"), "Hello, Alice!");
    }

    #[test]
    fn test_greet_full_name() {
        assert_eq!(greet("Bob Smith"), "Hello, Bob Smith!");
    }

    #[test]
    fn test_greet_empty_name() {
        assert_eq!(greet(""), "Hello, !");
    }

    #[test]
    fn test_greet_with_special_characters() {
        assert_eq!(greet("José"), "Hello, José!");
    }
}
