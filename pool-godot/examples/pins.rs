//! Prints the values behind the E4 pins. Run with `cargo run -p pool-godot --example pins`.
use pool_core::config::TableConfig;
use pool_core::hash::Hasher;
use pool_core::math::Vec3;
use pool_core::state::GameState;

fn rack(table: &TableConfig) -> (Vec<Vec3>, Vec<i32>) {
    let radius = table.ball.radius;
    let mut positions = vec![Vec3::new(0.25, 0.5625, radius)];
    let mut numbers = vec![0];
    for row in 0..4 {
        for column in 0..4 {
            if row == 0 && column == 0 {
                continue;
            }
            positions.push(Vec3::new(
                0.5 + 0.0625 * column as f64,
                0.5 + 0.0625 * row as f64,
                radius,
            ));
            numbers.push(numbers.len() as i32);
        }
    }
    (positions, numbers)
}

fn main() {
    let table = TableConfig::default();
    let (mut positions, mut numbers) = rack(&table);
    let mut rotations: Vec<Vec3> = Vec::new();
    let shots = [
        (8.0, 90.0, 0.0, 0.0, 0.0),
        (6.5, 105.0, 0.0, 0.006, 0.004),
        (7.5, 78.0, 8.0, -0.005, -0.003),
    ];
    for (speed, aim, elevation, cx, cy) in shots {
        let (state, trace) = pool_godot::bridge::simulate_from_intent_f64(
            &table,
            speed,
            aim,
            elevation,
            cx,
            cy,
            &pool_godot::bridge::triples_to_flat(&positions),
            &numbers,
            &pool_godot::bridge::triples_to_flat(&rotations),
        )
        .expect("shot runs");
        println!(
            "hash {} duration {:?} events {}",
            pool_core::hash_state_hex(&state),
            trace.duration,
            trace.events.len()
        );
        let mut path = Hasher::new();
        for t in [0.0, trace.duration * 0.5, trace.duration] {
            for p in trace.sample_positions(t) {
                path.write_f64(p.x);
                path.write_f64(p.y);
                path.write_f64(p.z);
            }
        }
        println!("path {:016x}", path.finish());
        let final_rotations = trace.sample_rotation(trace.duration);
        let mut spin = Hasher::new();
        for r in &final_rotations {
            spin.write_f64(r.x);
            spin.write_f64(r.y);
            spin.write_f64(r.z);
        }
        println!("rotations {:016x}", spin.finish());
        let mut next_positions = Vec::new();
        let mut next_numbers = Vec::new();
        let mut next_rotations = Vec::new();
        let GameState { balls, .. } = state;
        for (i, ball) in balls.iter().enumerate() {
            if ball.is_pocketed || ball.is_off_table {
                continue;
            }
            let mut p = ball.position;
            p.z = table.ball.radius;
            next_positions.push(p);
            next_numbers.push(ball.number as i32);
            next_rotations.push(final_rotations[i]);
        }
        if !next_numbers.contains(&0) {
            next_positions.insert(0, positions[0]);
            next_numbers.insert(0, 0);
            next_rotations.insert(0, Vec3::ZERO);
        }
        positions = next_positions;
        numbers = next_numbers;
        rotations = next_rotations;
    }
    println!("canary {}", pool_intent::canary_hex());
}
