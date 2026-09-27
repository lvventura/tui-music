use std::collections::{HashMap, HashSet};

use crate::youtube::models::Song;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DJMode {
    Balanced,
    Favorites,
    Discovery,
    ArtistFocus,
}

#[derive(Clone, Debug)]
pub struct DJConfig {
    pub enabled: bool,
    pub auto_queue_enabled: bool,
    pub auto_queue_size: usize,
    pub minimum_queue_size: usize,
    pub exploration_ratio: f32,
    pub favorite_bonus: f32,
    pub history_weight: f32,
    pub diversity_weight: f32,
    pub recent_track_penalty: f32,
    pub recent_artist_penalty: f32,
    pub default_mode: DJMode,
}

impl Default for DJConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_queue_enabled: false,
            auto_queue_size: 5,
            minimum_queue_size: 2,
            exploration_ratio: 0.2,
            favorite_bonus: 4.0,
            history_weight: 1.0,
            diversity_weight: 2.0,
            recent_track_penalty: 8.0,
            recent_artist_penalty: 3.0,
            default_mode: DJMode::Balanced,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct UserMusicProfile {
    pub artist_affinity: HashMap<String, f32>,
    pub track_affinity: HashMap<String, f32>,
    pub favorite_tracks: HashSet<String>,
}

impl UserMusicProfile {
    pub fn from_history(history: &[Song], favorites: &[Song]) -> Self {
        let favorite_tracks = favorites.iter().map(|song| song.id.clone()).collect();
        let mut profile = Self {
            favorite_tracks,
            ..Self::default()
        };
        for (index, song) in history.iter().enumerate() {
            let recency = 1.0 / (index as f32 + 1.0).sqrt();
            *profile
                .artist_affinity
                .entry(song.artist.clone())
                .or_default() += recency;
            *profile.track_affinity.entry(song.id.clone()).or_default() += recency;
        }
        for song in favorites {
            *profile
                .artist_affinity
                .entry(song.artist.clone())
                .or_default() += 2.0;
            *profile.track_affinity.entry(song.id.clone()).or_default() += 3.0;
        }
        normalize(&mut profile.artist_affinity);
        normalize(&mut profile.track_affinity);
        profile
    }
}

fn normalize(values: &mut HashMap<String, f32>) {
    if let Some(maximum) = values.values().copied().reduce(f32::max) {
        if maximum > 0.0 {
            values.values_mut().for_each(|value| *value /= maximum);
        }
    }
}

#[derive(Clone, Debug)]
pub struct DJContext {
    pub current_song: Option<Song>,
    pub recent_songs: Vec<Song>,
    pub history: Vec<Song>,
    pub favorites: Vec<Song>,
    pub queued_ids: HashSet<String>,
    pub mode: DJMode,
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub song: Song,
    pub source: &'static str,
}

pub trait CandidateSource {
    fn candidates(&self) -> Vec<Candidate>;
}

pub struct LocalCandidateSource {
    pub history: Vec<Song>,
    pub favorites: Vec<Song>,
    pub playlist: Vec<Song>,
}

impl CandidateSource for LocalCandidateSource {
    fn candidates(&self) -> Vec<Candidate> {
        let mut seen = HashSet::new();
        self.favorites
            .iter()
            .map(|song| (song, "favorite"))
            .chain(self.playlist.iter().map(|song| (song, "playlist")))
            .chain(self.history.iter().map(|song| (song, "history")))
            .filter_map(|(song, source)| {
                seen.insert(song.id.clone()).then(|| Candidate {
                    song: song.clone(),
                    source,
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct SongScore {
    pub total: f32,
    pub reasons: Vec<String>,
}

pub struct Scorer<'a> {
    pub profile: &'a UserMusicProfile,
    pub config: &'a DJConfig,
}

impl<'a> Scorer<'a> {
    pub fn score(&self, candidate: &Candidate, context: &DJContext) -> SongScore {
        let song = &candidate.song;
        let mut total = self
            .profile
            .artist_affinity
            .get(&song.artist)
            .copied()
            .unwrap_or_default()
            * self.config.history_weight;
        let mut reasons = Vec::new();
        if total > 0.0 {
            reasons.push("Artista frecuente".into());
        }
        if self.profile.favorite_tracks.contains(&song.id) {
            total += self.config.favorite_bonus;
            reasons.push("Favorito".into());
        }
        if self.profile.track_affinity.contains_key(&song.id) {
            reasons.push("Historial de reproducción".into());
        }
        if context
            .recent_songs
            .iter()
            .any(|recent| recent.id == song.id)
        {
            reasons.push("Escuchada recientemente".into());
            total = f32::MIN;
        }
        if context
            .recent_songs
            .iter()
            .any(|recent| recent.artist == song.artist)
        {
            total -= self.config.recent_artist_penalty;
            reasons.push("Diversidad de artistas".into());
        } else {
            total += self.config.diversity_weight;
            reasons.push("Buena diversidad".into());
        }
        if context.queued_ids.contains(&song.id)
            || context
                .current_song
                .as_ref()
                .is_some_and(|current| current.id == song.id)
        {
            total = f32::MIN;
            reasons.push("Ya está en reproducción o cola".into());
        }
        match context.mode {
            DJMode::Favorites if !self.profile.favorite_tracks.contains(&song.id) => total -= 2.0,
            DJMode::Discovery if self.profile.track_affinity.contains_key(&song.id) => total -= 2.0,
            DJMode::ArtistFocus => {
                if context
                    .current_song
                    .as_ref()
                    .is_some_and(|current| current.artist == song.artist)
                {
                    total += 3.0;
                }
            }
            _ => {}
        }
        SongScore { total, reasons }
    }
}

#[derive(Clone, Debug)]
pub struct DJDecision {
    pub song: Song,
    pub score: f32,
    pub strategy: DJMode,
    pub reasons: Vec<String>,
}

pub struct DJEngine {
    pub config: DJConfig,
    pub profile: UserMusicProfile,
}

impl DJEngine {
    pub fn new(config: DJConfig, profile: UserMusicProfile) -> Self {
        Self { config, profile }
    }

    pub fn recommend(&self, context: &DJContext, candidates: &[Candidate]) -> Vec<DJDecision> {
        let scorer = Scorer {
            profile: &self.profile,
            config: &self.config,
        };
        let mut ranked: Vec<_> = candidates
            .iter()
            .map(|candidate| (candidate, scorer.score(candidate, context)))
            .filter(|(_, score)| score.total > f32::MIN / 2.0)
            .collect();
        ranked.sort_by(|(_, left), (_, right)| right.total.total_cmp(&left.total));
        let limit = self.config.auto_queue_size.max(1);
        let exploration = ((limit as f32) * self.config.exploration_ratio).round() as usize;
        let mut result = Vec::new();
        for (index, (candidate, score)) in ranked.into_iter().enumerate() {
            if index >= limit {
                break;
            }
            if exploration > 0 && index + exploration >= limit && index % 2 == 1 {
                continue;
            }
            result.push(DJDecision {
                song: candidate.song.clone(),
                score: score.total,
                strategy: context.mode,
                reasons: score.reasons,
            });
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str, artist: &str) -> Song {
        Song {
            id: id.into(),
            title: id.into(),
            artist: artist.into(),
            duration: None,
            thumbnail: None,
            webpage_url: None,
        }
    }

    #[test]
    fn favorites_rank_above_unknown_tracks() {
        let favorite = song("favorite", "known");
        let unknown = song("unknown", "new");
        let profile = UserMusicProfile::from_history(&[], std::slice::from_ref(&favorite));
        let engine = DJEngine::new(DJConfig::default(), profile);
        let context = DJContext {
            current_song: None,
            recent_songs: vec![],
            history: vec![],
            favorites: vec![favorite.clone()],
            queued_ids: HashSet::new(),
            mode: DJMode::Balanced,
        };
        let results = engine.recommend(
            &context,
            &[
                Candidate {
                    song: unknown,
                    source: "provider",
                },
                Candidate {
                    song: favorite,
                    source: "favorite",
                },
            ],
        );
        assert_eq!(
            results.first().map(|item| item.song.id.as_str()),
            Some("favorite")
        );
    }

    #[test]
    fn recent_tracks_are_excluded_from_recommendations() {
        let recent = song("recent", "artist");
        let profile = UserMusicProfile::default();
        let engine = DJEngine::new(DJConfig::default(), profile);
        let context = DJContext {
            current_song: None,
            recent_songs: vec![recent.clone()],
            history: vec![],
            favorites: vec![],
            queued_ids: HashSet::new(),
            mode: DJMode::Balanced,
        };
        let results = engine.recommend(
            &context,
            &[Candidate {
                song: recent,
                source: "history",
            }],
        );
        assert!(results.is_empty());
    }
}
