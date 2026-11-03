use crate::config::model::Profile;
use std::f64::consts::PI;

const SAMPLE_RATE: f64 = 48_000.0;

/// Magnitude (dB) of one RBJ peaking biquad (the filter PipeWire's
/// `bq_peaking` uses) at `freq`.
pub fn peaking_db(freq: f64, f0: f64, gain_db: f64, q: f64) -> f64 {
    let a = 10f64.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * f0 / SAMPLE_RATE;
    let alpha = w0.sin() / (2.0 * q.max(0.1));
    let c = w0.cos();
    let (b0, b1, b2) = (1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a);
    let (a0, a1, a2) = (1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a);
    let w = 2.0 * PI * freq / SAMPLE_RATE;
    let mag = |x0: f64, x1: f64, x2: f64| {
        let re = x0 + x1 * w.cos() + x2 * (2.0 * w).cos();
        let im = -(x1 * w.sin() + x2 * (2.0 * w).sin());
        (re * re + im * im).sqrt()
    };
    20.0 * (mag(b0, b1, b2) / mag(a0, a1, a2)).log10()
}

/// Total response (dB) of the whole profile (pre-amp + all bands) at `freq`.
pub fn profile_db(profile: &Profile, freq: f64) -> f64 {
    f64::from(profile.preamp_db)
        + profile
            .bands
            .iter()
            .map(|b| peaking_db(freq, f64::from(b.frequency_hz), f64::from(b.gain_db), f64::from(b.q)))
            .sum::<f64>()
}

/// `points` samples of the response, log-spaced between `fmin` and `fmax`.
pub fn curve(profile: &Profile, points: usize, fmin: f64, fmax: f64) -> Vec<(f64, f64)> {
    (0..points)
        .map(|i| {
            let t = i as f64 / (points - 1).max(1) as f64;
            let f = fmin * (fmax / fmin).powf(t);
            (f, profile_db(profile, f))
        })
        .collect()
}
