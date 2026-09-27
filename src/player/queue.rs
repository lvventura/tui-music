use crate::youtube::models::Song;

#[derive(Default)]
pub struct PlaybackQueue {
    songs: Vec<Song>,
    order: Vec<usize>,
    current: usize,
    shuffle: bool,
}

impl PlaybackQueue {
    pub fn add(&mut self, song: Song) {
        self.songs.push(song);
        self.rebuild_order();
    }

    pub fn add_next(&mut self, song: Song) {
        let position = self.current.saturating_add(1).min(self.songs.len());
        self.songs.insert(position, song);
        self.rebuild_order();
    }

    pub fn play_now(&mut self, song: Song) {
        self.songs.insert(0, song);
        self.current = 0;
        self.rebuild_order();
    }

    pub fn songs(&self) -> &[Song] {
        &self.songs
    }

    pub fn remove(&mut self, index: usize) -> Option<Song> {
        if index >= self.songs.len() {
            return None;
        }
        let removed = self.songs.remove(index);
        self.current = self.current.min(self.songs.len().saturating_sub(1));
        self.rebuild_order();
        Some(removed)
    }

    pub fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.songs.len() || to >= self.songs.len() || from == to {
            return;
        }
        let song = self.songs.remove(from);
        self.songs.insert(to, song);
        self.current = to;
        self.rebuild_order();
    }

    pub fn clear(&mut self) {
        self.songs.clear();
        self.order.clear();
        self.current = 0;
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        self.shuffle = enabled;
        self.rebuild_order();
    }

    pub fn current(&self) -> Option<&Song> {
        self.order
            .get(self.current)
            .and_then(|index| self.songs.get(*index))
    }

    pub fn next(&mut self) -> Option<Song> {
        if self.current + 1 >= self.order.len() {
            return None;
        }
        self.current += 1;
        self.current().cloned()
    }

    pub fn previous(&mut self) -> Option<Song> {
        if self.current == 0 {
            return None;
        }
        self.current -= 1;
        self.current().cloned()
    }

    fn rebuild_order(&mut self) {
        self.order = (0..self.songs.len()).collect();
        if self.shuffle {
            let mut seed = self.songs.len() as u64 + 1;
            for index in (1..self.order.len()).rev() {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let swap = (seed as usize) % (index + 1);
                self.order.swap(index, swap);
            }
        }
        self.current = self.current.min(self.order.len().saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::PlaybackQueue;
    use crate::youtube::models::Song;

    fn song(id: &str) -> Song {
        Song {
            id: id.into(),
            title: id.into(),
            artist: "artist".into(),
            duration: Some(1),
            thumbnail: None,
            webpage_url: None,
        }
    }

    #[test]
    fn queue_supports_insert_move_and_shuffle_without_losing_songs() {
        let mut queue = PlaybackQueue::default();
        queue.add(song("one"));
        queue.add(song("three"));
        queue.add_next(song("two"));
        queue.move_item(2, 0);
        queue.set_shuffle(true);
        assert_eq!(queue.songs().len(), 3);
        queue.clear();
        assert!(queue.songs().is_empty());
    }
}
