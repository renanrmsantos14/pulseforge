use crate::config::model::{Band, Profile};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: [f32; 2],
    x2: [f32; 2],
    y1: [f32; 2],
    y2: [f32; 2],
}

impl Biquad {
    pub fn peaking(sample_rate: f32, band: &Band) -> Self {
        let a = 10f32.powf(band.gain_db / 40.);
        let w = 2. * PI * band.frequency_hz / sample_rate;
        let alpha = w.sin() / (2. * band.q.max(0.1));
        let c = w.cos();
        let a0 = 1. + alpha / a;
        Self {
            b0: (1. + alpha * a) / a0,
            b1: -2. * c / a0,
            b2: (1. - alpha * a) / a0,
            a1: -2. * c / a0,
            a2: (1. - alpha / a) / a0,
            x1: [0.; 2],
            x2: [0.; 2],
            y1: [0.; 2],
            y2: [0.; 2],
        }
    }
    fn process(&mut self, channel: usize, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1[channel] + self.b2 * self.x2[channel]
            - self.a1 * self.y1[channel]
            - self.a2 * self.y2[channel];
        self.x2[channel] = self.x1[channel];
        self.x1[channel] = x;
        self.y2[channel] = self.y1[channel];
        self.y1[channel] = y;
        y
    }
}

pub struct EqualizerChain {
    bands: Vec<Biquad>,
    preamp: f32,
    pub bypass: bool,
}
impl EqualizerChain {
    pub fn from_profile(profile: &Profile, sample_rate: f32) -> Self {
        Self {
            bands: profile
                .bands
                .iter()
                .map(|b| Biquad::peaking(sample_rate, b))
                .collect(),
            preamp: 10f32.powf(profile.preamp_db / 20.),
            bypass: false,
        }
    }
    pub fn process_in_place(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.bypass {
            return;
        }
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let mut a = *l * self.preamp;
            let mut b = *r * self.preamp;
            for band in &mut self.bands {
                a = band.process(0, a);
                b = band.process(1, b);
            }
            *l = a;
            *r = b;
        }
    }
}
