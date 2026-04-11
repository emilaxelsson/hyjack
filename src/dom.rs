pub struct PlayerTime {
  pub hour: u8,
  pub minute: u8,
  pub second: u8,
  pub millis: u16,
}

pub struct TopRow {
  pub drackerVersion: String,
  pub playerTime: PlayerTime,
  pub songTitle: String,
  pub patternTitle: String,
}

pub struct EditingSettings {
  pub octave: u8,
  pub velocity: u8,
  pub step: u8,
  pub instrument: String,
}

pub struct SequencePanel {
  pub sequence: Vec<String>,
  pub sequenceIndex: usize,
  pub sequencePool: Vec<String>,
}

pub struct Note {
  pub pitch: u8,
  pub instrument: u8,
  pub velocity: u8,
}

pub struct Track {
  pub name: String,
  pub notes: Vec<Option<Note>>,
}

pub struct PatternPanel {
  pub patternTitle: String,
  pub tracks: Vec<Track>,
  pub currentNote: String,
  pub beats: u8,
  pub rpb: u8,
}

pub struct Dom {
  pub topRow: TopRow,
  pub editingSettings: EditingSettings,
  pub sequencePanel: SequencePanel,
  pub patternPanel: PatternPanel,
}

pub fn defaultDom() -> Dom {
  return Dom {
    topRow: TopRow {
      drackerVersion: "0.2".to_string(),
      playerTime: PlayerTime {
        hour: 0,
        minute: 2,
        second: 32,
        millis: 0,
      },
      songTitle: "My song".to_string(),
      patternTitle: "Intro A".to_string(),
    },
    editingSettings: EditingSettings {
      octave: 3,
      velocity: 50,
      step: 4,
      instrument: "2/Hihat".to_string(),
    },
    sequencePanel: SequencePanel {
      sequence: vec![
        "Intro A".to_string(),
        "Intro B".to_string(),
        "Verse 1 A".to_string(),
        "Verse 1 B".to_string(),
        "Verse 1 C".to_string(),
        "Verse 1 D".to_string(),
        "Bridge A".to_string(),
        "Bridge B".to_string(),
      ],
      sequenceIndex: 0,
      sequencePool: vec!["Bridge B".to_string(), "Intro X".to_string()],
    },
    patternPanel: PatternPanel {
      patternTitle: "Intro A".to_string(),
      tracks: vec![
        Track {
          name: "Hat/ride".to_string(),
          notes: vec![
            Some(Note {
              pitch: 36,
              instrument: 2,
              velocity: 20,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
          ],
        },
        Track {
          name: "Kick".to_string(),
          notes: vec![
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
          ],
        },
        Track {
          name: "Snare".to_string(),
          notes: vec![
            None,
            None,
            None,
            None,
            Some(Note {
              pitch: 36,
              instrument: 2,
              velocity: 20,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
          ],
        },
        Track {
          name: "Cymbals".to_string(),
          notes: vec![
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
          ],
        },
      ],
      currentNote: "2/Hihat".to_string(),
      beats: 4,
      rpb: 4,
    },
  };
}
