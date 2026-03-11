use std::{env, fs, path::Path};

use daktronics_allsport_5000::{
    sports::{basketball::BasketballSport, Sport},
    RTDState,
};
use tokio_serial::SerialPortBuilderExt;

#[cfg(unix)]
const DEFAULT_TTY: &str = "/dev/ttyUSB0";
#[cfg(windows)]
const DEFAULT_TTY: &str = "COM1";

const DEFAULT_OUTPUT_PATH: &str = "./basketball-live.json";

/// Main function to dump basketball data as JSON.
///
/// This function initializes a serial port connection to the AllSport 5000
/// controller, reads the basketball game data, and writes the latest snapshot
/// into a JSON file each update.
#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    // Get the command line arguments and determine the TTY path.
    let mut args = env::args();
    let tty_path = args.nth(1).unwrap_or_else(|| DEFAULT_TTY.into());
    let output_path = args.next().unwrap_or_else(|| DEFAULT_OUTPUT_PATH.into());

    // Create the serial port. Note the baud rate and parity.
    // allow because cargo gets suspicious on Windows
    #[allow(unused_mut)]
    let mut port = tokio_serial::new(tty_path, 19200)
        .parity(tokio_serial::Parity::None)
        .open_native_async()?;

    #[cfg(unix)]
    port.set_exclusive(false)
        .expect("Unable to set serial port exclusive to false");

    // Initialize the BasketballSport with the RTDState from the serial stream.
    let mut sport = BasketballSport::new(RTDState::from_serial_stream(port, true).unwrap());

    // Continuously update and write the basketball game data as JSON.
    let mut update_result = Ok(false);
    while let Ok(_) = update_result {
        let snapshot = serde_json::to_string_pretty(&sport).expect("couldn't read data");
        persist_snapshot(&output_path, &snapshot).expect("couldn't write snapshot");
        println!("Updated {}", output_path);

        update_result = sport.rtd_state().update_async().await;
    }
    if let Err(e) = update_result {
        eprintln!("{:?}", e);
    }
    Ok(())
}

fn persist_snapshot(output_path: &str, snapshot: &str) -> std::io::Result<()> {
    let path = Path::new(output_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, snapshot)
}
