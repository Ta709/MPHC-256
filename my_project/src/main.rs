use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use rand::{distributions::Alphanumeric, Rng};
use std::time::{SystemTime, UNIX_EPOCH};
use std::process;
use base64::{encode};

const ROUNDS: usize = 256; // Number of rounds
const PRIME: u128 = 0x100000001b3; // A large prime number

/// Generate a dynamic pepper using system-specific values
fn generate_dynamic_pepper() -> String {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let process_id = process::id();
    format!("{:x}-{:x}", time, process_id)
}

/// Modular non-linear transformation
fn modular_non_linear_transform(value: u128) -> u128 {
    let rotated = value.rotate_right(23);
    (value ^ rotated).wrapping_mul(PRIME) % 0xffffffffffffffff
}

/// Perform a single round of hashing (SIMD-style)
fn hash_round(
    state: &mut [u128; 4],
    mixed_input: u128,
    round: usize,
) {
    for i in 0..4 {
        state[i] ^= mixed_input; // XOR with input
        state[i] = state[i].wrapping_add(PRIME); // Add large prime
        state[i] = state[i].rotate_left((round % 16 + i * 8) as u32); // Rotate bits
        state[i] = modular_non_linear_transform(state[i]); // Apply non-linear transform
        state[i] ^= state[(i + 1) % 4]; // Cross-mix state
    }
}

/// Advanced 256-bit hash function
fn advanced_hash(input: &str, salt: &str, pepper: &str) -> [u8; 32] {
    // Initialize 256-bit state (4 x 64-bit chunks)
    let mut state: [u128; 4] = [
        0x6a09e667f3bcc908,
        0xbb67ae8584caa73b,
        0x3c6ef372fe94f82b,
        0xa54ff53a5f1d36f1,
    ];

    // Combine input, salt, and pepper
    let combined_input = format!("{}{}{}", pepper, input, salt);

    // Split the combined input into chunks for parallel processing
    let mut threads = vec![];
    for round in 0..ROUNDS {
        let mut local_state = state.clone();
        for byte in combined_input.bytes() {
            let dynamic_salt = (round as u128).wrapping_add(PRIME) ^ (local_state[round % 4] & 0xff);
            let mixed_byte = u128::from(byte).wrapping_add(dynamic_salt);

            // Spawn threads for each state chunk
            let handle = thread::spawn(move || {
                let mut chunk_state = local_state;
                hash_round(&mut chunk_state, mixed_byte, round);
                chunk_state
            });

            threads.push(handle);
        }
    }

    // Collect results from threads and update state
    for handle in threads {
        if let Ok(thread_state) = handle.join() {
            for i in 0..4 {
                state[i] ^= thread_state[i];
            }
        }
    }

    // Convert 4 x u128 chunks to a 256-bit hash
    let mut final_hash = [0u8; 32];
    for (i, chunk) in state.iter().enumerate() {
        // Use only the least significant 8 bytes of each u128
        final_hash[i * 8..(i + 1) * 8].copy_from_slice(&chunk.to_le_bytes()[..8]);
    }

    final_hash
}

/// Generate a random salt
fn generate_salt() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(67)
        .map(char::from)
        .collect()
}

/// Convert hash to binary string
fn hash_to_binary_string(hash: &[u8; 32]) -> String {
    hash.iter()
        .map(|byte| format!("{:08b}", byte))
        .collect::<Vec<String>>()
        .concat()
}

fn main() -> io::Result<()> {
    // Define the folder and log file paths
    let folder_path = "logs";
    let log_file_path = format!("{}/hash_log.txt", folder_path);

    // Create the folder if it doesn't exist
    if !Path::new(folder_path).exists() {
        fs::create_dir(folder_path)?;
    }

    // Open the log file in append mode, creating it if it doesn't exist
    let mut log_file = File::options()
        .create(true)
        .append(true)
        .open(&log_file_path)?;

    println!("Enter strings to hash (type exit to quit):");

    loop {
        // Read user input
        let mut input_string = String::new();
        io::stdin().read_line(&mut input_string)?;
        let input_string = input_string.trim(); // Remove trailing newline

        // Exit the loop if the user types 'exit'
        if input_string.eq_ignore_ascii_case("exit") {
            break;
        }

        // Generate a random salt and dynamic pepper
        let salt = generate_salt();
        let pepper = generate_dynamic_pepper();

        // Generate the advanced 256-bit hash
        let hash_result = advanced_hash(input_string, &salt, &pepper);

        // Convert hash to binary string
        let binary_hash = hash_to_binary_string(&hash_result);

        // Encode the hash result in Base64
        let base64_hash = encode(&hash_result);

        // Log the input, salt, pepper, and hash details to the file
        writeln!(
            log_file,
            "Input: '{}'\nSalt: '{}'\nPepper: '{}'\nBinary Hash: {}\nBase64 Hash: {}\n",
            input_string, salt, pepper, binary_hash, base64_hash
        )?;

        // Add an empty line between log entries for better readability
        writeln!(log_file, "")?;

        // Print only the Base64 hash in the console
        println!("(Base64): {}", base64_hash);
    }

    println!("Exiting. Log saved to '{}'.", log_file_path);

    Ok(())
}
