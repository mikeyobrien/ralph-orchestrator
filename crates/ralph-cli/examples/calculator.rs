//! Simple calculator CLI example
//!
//! Run with: cargo run -p ralph-cli --example calculator -- <operation> <a> <b>
//!
//! Examples:
//!   cargo run -p ralph-cli --example calculator -- add 5 3
//!   cargo run -p ralph-cli --example calculator -- subtract 10 4
//!   cargo run -p ralph-cli --example calculator -- multiply 6 7
//!   cargo run -p ralph-cli --example calculator -- divide 20 4

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: calculator <operation> <a> <b>");
        eprintln!("Operations: add, subtract, multiply, divide");
        process::exit(1);
    }

    let operation = &args[1];
    let a: f64 = match args[2].parse() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("Error: '{}' is not a valid number", args[2]);
            process::exit(1);
        }
    };
    let b: f64 = match args[3].parse() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("Error: '{}' is not a valid number", args[3]);
            process::exit(1);
        }
    };

    let result = match operation.as_str() {
        "add" => add(a, b),
        "subtract" => subtract(a, b),
        "multiply" => multiply(a, b),
        "divide" => match divide(a, b) {
            Some(r) => r,
            None => {
                eprintln!("Error: division by zero");
                process::exit(1);
            }
        },
        _ => {
            eprintln!("Error: unknown operation '{}'", operation);
            eprintln!("Valid operations: add, subtract, multiply, divide");
            process::exit(1);
        }
    };

    println!("{}", result);
}

fn add(a: f64, b: f64) -> f64 {
    a + b
}

fn subtract(a: f64, b: f64) -> f64 {
    a - b
}

fn multiply(a: f64, b: f64) -> f64 {
    a * b
}

fn divide(a: f64, b: f64) -> Option<f64> {
    if b == 0.0 { None } else { Some(a / b) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2.0, 3.0), 5.0);
        assert_eq!(add(-1.0, 1.0), 0.0);
        assert_eq!(add(0.0, 0.0), 0.0);
        assert_eq!(add(1.5, 2.5), 4.0);
    }

    #[test]
    fn test_subtract() {
        assert_eq!(subtract(5.0, 3.0), 2.0);
        assert_eq!(subtract(3.0, 5.0), -2.0);
        assert_eq!(subtract(0.0, 0.0), 0.0);
        assert_eq!(subtract(10.5, 0.5), 10.0);
    }

    #[test]
    fn test_multiply() {
        assert_eq!(multiply(2.0, 3.0), 6.0);
        assert_eq!(multiply(-2.0, 3.0), -6.0);
        assert_eq!(multiply(0.0, 100.0), 0.0);
        assert_eq!(multiply(1.5, 2.0), 3.0);
    }

    #[test]
    fn test_divide() {
        assert_eq!(divide(6.0, 2.0), Some(3.0));
        assert_eq!(divide(5.0, 2.0), Some(2.5));
        assert_eq!(divide(0.0, 5.0), Some(0.0));
        assert_eq!(divide(10.0, 0.0), None);
    }

    #[test]
    fn test_divide_by_zero() {
        assert!(divide(1.0, 0.0).is_none());
        assert!(divide(0.0, 0.0).is_none());
        assert!(divide(-5.0, 0.0).is_none());
    }
}
