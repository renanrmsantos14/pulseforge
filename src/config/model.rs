use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Band {
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub preamp_db: f32,
    pub bands: Vec<Band>,
    pub limiter: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SinkInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OutputRule {
    pub sink_id: Option<String>,
    pub name_contains: Option<String>,
    pub kind: Option<String>,
    pub profile_id: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub schema_version: u32,
    pub profiles: Vec<Profile>,
    pub output_rules: Vec<OutputRule>,
    pub fallback_profile_id: String,
    /// Balanço estéreo global em [-1, 1] (negativo favorece L, positivo R).
    #[serde(default)]
    pub balance: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        let freqs = [
            31., 62., 125., 250., 500., 1000., 2000., 4000., 6000., 8000., 12000., 16000.,
        ];
        let make = |id: &str, name: &str, gains: &[f32]| Profile {
            id: id.into(),
            name: name.into(),
            preamp_db: 0.,
            bands: freqs
                .iter()
                .zip(gains.iter().copied())
                .map(|(f, gain_db)| Band {
                    frequency_hz: *f,
                    gain_db,
                    q: 1.4,
                })
                .collect(),
            limiter: true,
        };
        let flat = make("flat", "Flat", &[0.; 12]);
        let bass = make(
            "bass-lift",
            "Bass Lift",
            &[5., 4., 3., 2., 1., 0., 0., 0., 0., 1., 1., 2.],
        );
        let vocal = make(
            "vocal",
            "Vocal",
            &[-2., -1., 0., 1., 3., 4., 3., 2., 1., 1., 0., 0.],
        );
        let night = make(
            "night",
            "Night Listening",
            &[-4., -3., -2., 0., 2., 2., 1., 0., -1., -1., -2., -2.],
        );
        let headphones = make(
            "headphones",
            "Headphones",
            &[2., 2., 1., 0., -1., 1., 2., 2., 2., 3., 3., 2.],
        );
        let flow_xa = make(
            "flow-xa-10",
            "FLow Xa 10",
            &[
                3.1, 3.0, 2.8, 2.1, -0.0, -0.2, -0.1, -0.1, -0.5, 0.2, 0.9, 1.0,
            ],
        );
        Self {
            schema_version: 1,
            profiles: vec![flat, bass, vocal, night, headphones, flow_xa],
            output_rules: vec![],
            fallback_profile_id: "flat".into(),
            balance: 0.0,
        }
    }
}
