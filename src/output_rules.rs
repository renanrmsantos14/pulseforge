use crate::config::model::{OutputRule, Profile, SinkInfo};

pub fn match_profile<'a>(
    sink: &SinkInfo,
    rules: &'a [OutputRule],
    profiles: &'a [Profile],
    fallback: &'a str,
) -> Option<&'a Profile> {
    let hit = rules
        .iter()
        .filter(|r| r.enabled)
        .find(|r| r.sink_id.as_deref() == Some(&sink.id))
        .or_else(|| {
            rules.iter().filter(|r| r.enabled).find(|r| {
                r.name_contains
                    .as_ref()
                    .is_some_and(|v| sink.name.to_lowercase().contains(&v.to_lowercase()))
            })
        })
        .or_else(|| {
            rules
                .iter()
                .filter(|r| r.enabled)
                .find(|r| r.kind.as_deref() == Some(&sink.kind))
        });
    profiles
        .iter()
        .find(|p| hit.map_or(p.id == fallback, |r| p.id == r.profile_id))
}
