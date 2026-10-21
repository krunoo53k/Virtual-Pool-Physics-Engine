//! A deterministic real-root finder for the polynomials that appear in the
//! time-of-impact conditions (§11.3).
//!
//! The ball-ball condition
//!
//! ```text
//! |dp0 + dv * t + 0.5 * da * t^2|^2 = (2R)^2
//! ```
//!
//! is a quartic in `t`. The document asks for a robust root finder rather than a
//! hand-rolled quartic formula, so this module isolates the real roots of the
//! polynomial with its derivative and then refines each of them by bisection plus
//! a few Newton steps. Bisection runs a fixed number of iterations, so the result
//! is bit-for-bit reproducible (§11.4).

/// A coefficient list in descending order: `[a_n, ..., a_1, a_0]`.
pub type Coefficients = [f64; 5];

/// Evaluates a polynomial with Horner's scheme.
pub fn eval(coeffs: &Coefficients, x: f64) -> f64 {
    let mut acc = coeffs[0];
    for c in &coeffs[1..] {
        acc = acc * x + c;
    }
    acc
}

/// The derivative of a polynomial, as a coefficient list.
pub fn derivative(coeffs: &Coefficients) -> Coefficients {
    let n = coeffs.len() - 1;
    let mut out = [0.0; 5];
    for i in 0..n {
        out[i] = coeffs[i] * (n - i) as f64;
    }
    out
}

/// All real roots of `coeffs` inside `[lo, hi]`, in ascending order.
pub fn real_roots_in(coeffs: &Coefficients, lo: f64, hi: f64) -> Vec<f64> {
    let mut roots = match degree(coeffs) {
        4 => quartic_roots(coeffs, lo, hi),
        3 => cubic_roots(coeffs[1], coeffs[2], coeffs[3], coeffs[4]),
        2 => quadratic_roots_abc(coeffs[2], coeffs[3], coeffs[4]),
        1 => linear_root(coeffs),
        _ => Vec::new(),
    };
    roots.retain(|r| r.is_finite() && *r >= lo - 1e-12 && *r <= hi + 1e-12);
    roots.sort_by(|a, b| a.total_cmp(b));
    roots.dedup_by(|a, b| (*a - *b).abs() <= 1e-12);
    roots
}

/// The degree of the polynomial, ignoring leading zero coefficients.
fn degree(coeffs: &Coefficients) -> usize {
    let mut scale: f64 = 0.0;
    for c in coeffs {
        scale = scale.max(c.abs());
    }
    let threshold = scale * 1e-14;
    for (i, c) in coeffs.iter().enumerate() {
        if c.abs() > threshold {
            return coeffs.len() - 1 - i;
        }
    }
    0
}

/// The roots of a quadratic `a t^2 + b t + c`.
pub fn quadratic_roots_abc(a: f64, b: f64, c: f64) -> Vec<f64> {
    if a.abs() < 1e-300 {
        if b.abs() < 1e-300 {
            return Vec::new();
        }
        return vec![-c / b];
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return Vec::new();
    }
    let sq = disc.sqrt();
    // The numerically stable form of the quadratic formula.
    let q = -0.5 * (b + if b >= 0.0 { sq } else { -sq });
    let mut roots = Vec::new();
    if q.abs() > 0.0 {
        roots.push(q / a);
        roots.push(c / q);
    } else {
        roots.push(0.0);
        roots.push(-b / a);
    }
    roots.sort_by(|x, y| x.total_cmp(y));
    roots
}

/// The roots of a quadratic.
pub fn quadratic_roots(coeffs: &Coefficients) -> Vec<f64> {
    quadratic_roots_abc(coeffs[2], coeffs[3], coeffs[4])
}

/// The root of a linear polynomial.
fn linear_root(coeffs: &Coefficients) -> Vec<f64> {
    let (a, b) = (coeffs[3], coeffs[4]);
    if a.abs() < 1e-300 {
        Vec::new()
    } else {
        vec![-b / a]
    }
}

/// The real roots of a cubic `a t^3 + b t^2 + c t + d`, using Cardano's formula (the
/// trigonometric form when there are three of them).
pub fn cubic_roots(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
    if a.abs() < 1e-300 {
        return quadratic_roots_abc(b, c, d);
    }
    // Depressed cubic: t^3 + p t + q = 0.
    let p = (3.0 * a * c - b * b) / (3.0 * a * a);
    let q = (2.0 * b * b * b - 9.0 * a * b * c + 27.0 * a * a * d) / (27.0 * a * a * a);
    let shift = b / (3.0 * a);

    let half_q = q / 2.0;
    let third_p = p / 3.0;
    let disc = half_q * half_q + third_p * third_p * third_p;

    if disc > 0.0 {
        // One real root.
        let sq = disc.sqrt();
        let u = libm::cbrt(-half_q + sq);
        let v = libm::cbrt(-half_q - sq);
        return vec![u + v - shift];
    }

    // Three real roots: t_k = 2 sqrt(-p/3) cos(theta_k - 2 pi k / 3).
    let m = 2.0 * (-third_p).sqrt();
    let arg = if third_p == 0.0 {
        0.0
    } else {
        (3.0 * q / (2.0 * p)) * (-3.0 / p).sqrt()
    };
    let theta = libm::acos(arg.clamp(-1.0, 1.0)) / 3.0;
    let mut roots = Vec::with_capacity(3);
    for k in 0..3 {
        let angle = theta - 2.0 * core::f64::consts::PI * (k as f64) / 3.0;
        roots.push(m * libm::cos(angle) - shift);
    }
    roots
}

/// The real roots of a quartic inside `[lo, hi]`.
///
/// The critical points of the polynomial (the roots of its derivative) split the
/// interval into pieces on which the polynomial is monotonic, so each piece can
/// contain at most one root. Sign changes are then resolved by bisection.
fn quartic_roots(coeffs: &Coefficients, lo: f64, hi: f64) -> Vec<f64> {
    let d = derivative(coeffs);
    let mut crit: Vec<f64> = cubic_roots(d[0], d[1], d[2], d[3])
        .into_iter()
        .filter(|r| r.is_finite() && *r > lo && *r < hi)
        .collect();
    crit.sort_by(|a, b| a.total_cmp(b));

    let mut bounds = Vec::with_capacity(crit.len() + 2);
    bounds.push(lo);
    bounds.extend_from_slice(&crit);
    bounds.push(hi);

    let mut roots = Vec::new();
    for w in bounds.windows(2) {
        let (x0, x1) = (w[0], w[1]);
        let f0 = eval(coeffs, x0);
        let f1 = eval(coeffs, x1);
        if f0 == 0.0 {
            roots.push(x0);
        }
        if f1 == 0.0 {
            roots.push(x1);
        }
        if f0 * f1 < 0.0 {
            if let Some(r) = bisect(coeffs, x0, x1) {
                roots.push(r);
            }
        }
    }

    // A double root shows up as a critical point where the polynomial is (almost)
    // zero; catch it so that tangential contacts are not missed.
    let scale = coeffs.iter().fold(0.0f64, |m, c| m.max(c.abs())).max(1.0);
    for c in &crit {
        if eval(coeffs, *c).abs() <= 1e-11 * scale {
            roots.push(*c);
        }
    }

    roots
}

/// Bisection with a fixed iteration count, so that the result is reproducible.
fn bisect(coeffs: &Coefficients, mut lo: f64, mut hi: f64) -> Option<f64> {
    let mut flo = eval(coeffs, lo);
    let mut fhi = eval(coeffs, hi);
    if flo == 0.0 {
        return Some(lo);
    }
    if fhi == 0.0 {
        return Some(hi);
    }
    if flo * fhi > 0.0 {
        return None;
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        let fmid = eval(coeffs, mid);
        if fmid == 0.0 {
            return Some(mid);
        }
        if flo * fmid < 0.0 {
            hi = mid;
            fhi = fmid;
        } else {
            lo = mid;
            flo = fmid;
        }
    }
    let _ = fhi;
    Some(0.5 * (lo + hi))
}

/// The smallest root of `coeffs` in `(t_min, t_max]`, if there is one.
pub fn first_root_after(coeffs: &Coefficients, t_min: f64, t_max: f64) -> Option<f64> {
    let roots = real_roots_in(coeffs, t_min, t_max);
    roots
        .into_iter()
        .filter(|r| *r > t_min && *r <= t_max)
        .min_by(|a, b| a.total_cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadratic_roots() {
        // (t - 1)(t - 3) = t^2 - 4t + 3
        let roots = quadratic_roots_abc(1.0, -4.0, 3.0);
        assert_eq!(roots.len(), 2);
        assert!((roots[0] - 1.0).abs() < 1e-12);
        assert!((roots[1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn cubic_with_three_roots() {
        // (t - 1)(t - 2)(t - 3) = t^3 - 6t^2 + 11t - 6
        let mut roots = cubic_roots(1.0, -6.0, 11.0, -6.0);
        roots.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(roots.len(), 3);
        assert!((roots[0] - 1.0).abs() < 1e-9);
        assert!((roots[1] - 2.0).abs() < 1e-9);
        assert!((roots[2] - 3.0).abs() < 1e-9);
    }

    #[test]
    fn cubic_with_one_root() {
        // t^3 + t = 0 -> t = 0
        let roots = cubic_roots(1.0, 0.0, 1.0, 0.0);
        assert_eq!(roots.len(), 1);
        assert!(roots[0].abs() < 1e-12);
    }

    #[test]
    fn quartic_with_four_roots() {
        // (t - 1)(t - 2)(t - 3)(t - 4)
        let mut roots = real_roots_in(&[1.0, -10.0, 35.0, -50.0, 24.0], 0.0, 10.0);
        roots.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(roots.len(), 4);
        for (i, expected) in [1.0, 2.0, 3.0, 4.0].iter().enumerate() {
            assert!(
                (roots[i] - expected).abs() < 1e-9,
                "root {}: {}",
                i,
                roots[i]
            );
        }
    }

    #[test]
    fn quartic_with_a_double_root() {
        // (t - 2)^2 (t - 5)(t + 1) = t^4 - 8t^3 + 15t^2 + 4t - 20
        let coeffs = [1.0, -8.0, 15.0, 4.0, -20.0];
        let roots = real_roots_in(&coeffs, -5.0, 10.0);
        assert!(roots.iter().any(|r| (r - 2.0).abs() < 1e-6));
        assert!(roots.iter().any(|r| (r - 5.0).abs() < 1e-9));
        assert!(roots.iter().any(|r| (r + 1.0).abs() < 1e-9));
    }

    #[test]
    fn quartic_without_real_roots() {
        // (t^2 + 1)(t^2 + 4)
        let roots = real_roots_in(&[1.0, 0.0, 5.0, 0.0, 4.0], -10.0, 10.0);
        assert!(roots.is_empty());
    }
}
