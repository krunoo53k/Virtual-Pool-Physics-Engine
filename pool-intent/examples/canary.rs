//! Prints the canary and the model version for the handshake and the docs.
fn main() {
    println!("model {}", pool_core::MODEL_VERSION);
    println!("canary {}", pool_intent::canary_hex());
}
