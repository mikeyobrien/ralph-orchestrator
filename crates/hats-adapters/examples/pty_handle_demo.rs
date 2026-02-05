//! Demo: PtyHandle loopback test
//!
//! Creates a PtyHandle with loopback (output_tx sends to input_rx),
//! sends "test\n", and verifies round-trip.

use hats_adapters::PtyHandle;
use tokio::sync::{mpsc, watch};

#[tokio::main]
async fn main() {
    println!("PtyHandle Demo: Loopback Test");
    println!("==============================\n");

    // Create channels for loopback
    let (output_tx, output_rx) = mpsc::unbounded_channel();
    let (input_tx, mut input_rx) = mpsc::unbounded_channel();
    let (control_tx, mut _control_rx) = mpsc::unbounded_channel();
    let (_terminated_tx, terminated_rx) = watch::channel(false);

    // Create PtyHandle
    let mut handle = PtyHandle {
        output_rx,
        input_tx: input_tx.clone(),
        control_tx,
        terminated_rx,
    };

    // Spawn loopback task: forward input to output
    tokio::spawn(async move {
        while let Some(data) = input_rx.recv().await {
            let _ = output_tx.send(data);
        }
    });

    // Send test data
    let test_data = b"test\n".to_vec();
    println!("Sent: {}", String::from_utf8_lossy(&test_data).trim());
    handle.input_tx.send(test_data).unwrap();

    // Receive and verify
    if let Some(received) = handle.output_rx.recv().await {
        println!("Received: {}", String::from_utf8_lossy(&received).trim());
        println!("\n✓ Round-trip successful!");
    } else {
        println!("\n✗ No data received");
    }
}
