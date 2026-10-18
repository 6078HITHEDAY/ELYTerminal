//! What Ely feels from the sessions' marks, and the face she makes at each moment of it.

use std::time::Duration;

use super::faces::Face;
use crate::sessions::orb::Look;

/// What the sessions ask of her, least pressing first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mood {
    Calm,
    Done,
    Working,
    Error,
    Waiting,
}

/// The most pressing of the marks' looks; calm with none.
pub fn mood(looks: impl IntoIterator<Item = Look>) -> Mood {
    looks
        .into_iter()
        .map(|look| match look {
            Look::Waiting => Mood::Waiting,
            Look::Error => Mood::Error,
            Look::Working(_) => Mood::Working,
            Look::Done => Mood::Done,
            Look::Idle => Mood::Calm,
        })
        .max()
        .unwrap_or(Mood::Calm)
}

/// How long she cheers a program that finished, and how long calm lasts before she dozes.
const CHEER: Duration = Duration::from_secs(4);
const DOZE: Duration = Duration::from_secs(300);
/// How long a press keeps her wink.
pub const PETTED: Duration = Duration::from_millis(1500);

/// A calm round of 20 s: she blinks twice and glances left, then right.
const CALM: (f32, [(f32, Face); 9]) = (
    20.0,
    [
        (0.0, Face::Calm),
        (4.0, Face::Blink),
        (4.15, Face::Calm),
        (9.0, Face::Left),
        (10.2, Face::Calm),
        (13.0, Face::Blink),
        (13.15, Face::Calm),
        (16.0, Face::Right),
        (17.2, Face::Calm),
    ],
);

/// A working round of 8 s: her eyes stay on the terminal but for a blink.
const WORKING: (f32, [(f32, Face); 3]) = (
    8.0,
    [(0.0, Face::Down), (5.0, Face::Blink), (5.15, Face::Down)],
);

/// Her face `held` into `mood`, and how long it holds; none while it holds for good.
pub fn face(mood: Mood, held: Duration) -> (Face, Option<Duration>) {
    match mood {
        Mood::Waiting => (Face::Surprised, None),
        Mood::Error => (Face::Worried, None),
        Mood::Done if held < CHEER => (Face::Happy, Some(CHEER - held)),
        Mood::Working => round(WORKING.0, &WORKING.1, held),
        Mood::Calm | Mood::Done if held >= DOZE => (Face::Sleepy, None),
        Mood::Calm | Mood::Done => {
            let (face, next) = round(CALM.0, &CALM.1, held);
            (face, next.map(|next| next.min(DOZE - held)))
        }
    }
}

/// The face a round of `period` seconds shows `held` into it, and how long it holds.
fn round(period: f32, steps: &[(f32, Face)], held: Duration) -> (Face, Option<Duration>) {
    let at = held.as_secs_f32() % period;
    let now = steps
        .iter()
        .rposition(|&(start, _)| start <= at)
        .expect("a round starts at 0");
    let end = steps.get(now + 1).map_or(period, |&(start, _)| start);
    (steps[now].1, Some(Duration::from_secs_f32(end - at)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_pressing_mark_sets_her_mood() {
        assert_eq!(mood([]), Mood::Calm);
        assert_eq!(mood([Look::Idle, Look::Done]), Mood::Done);
        assert_eq!(
            mood([Look::Done, Look::Working(Some(40)), Look::Idle]),
            Mood::Working
        );
        assert_eq!(mood([Look::Working(None), Look::Error]), Mood::Error);
        assert_eq!(
            mood([Look::Error, Look::Waiting, Look::Done]),
            Mood::Waiting
        );
    }

    #[test]
    fn calm_blinks_and_glances_then_dozes() {
        let at = |secs: f32| face(Mood::Calm, Duration::from_secs_f32(secs)).0;
        assert_eq!(at(1.0), Face::Calm);
        assert_eq!(at(4.1), Face::Blink);
        assert_eq!(at(9.5), Face::Left);
        assert_eq!(at(16.5), Face::Right);
        assert_eq!(at(24.1), Face::Blink, "the round comes again");
        assert_eq!(at(300.0), Face::Sleepy);
        assert_eq!(
            face(Mood::Calm, Duration::from_secs(400)).1,
            None,
            "she sleeps on"
        );
        let (_, wait) = face(Mood::Calm, Duration::from_secs_f32(1.0));
        assert!((wait.expect("a next face").as_secs_f32() - 3.0).abs() < 1e-3);
    }

    #[test]
    fn a_finished_program_cheers_her_then_she_calms() {
        let (face_now, wait) = face(Mood::Done, Duration::from_secs(1));
        assert_eq!(face_now, Face::Happy);
        assert_eq!(wait, Some(Duration::from_secs(3)));
        assert_eq!(face(Mood::Done, Duration::from_secs(5)).0, Face::Calm);
    }

    #[test]
    fn waiting_and_errors_hold_their_faces() {
        assert_eq!(
            face(Mood::Waiting, Duration::from_secs(999)),
            (Face::Surprised, None)
        );
        assert_eq!(face(Mood::Error, Duration::ZERO), (Face::Worried, None));
        assert_eq!(
            face(Mood::Working, Duration::from_secs_f32(5.1)).0,
            Face::Blink
        );
        assert_eq!(
            face(Mood::Working, Duration::from_secs(1000)).0,
            Face::Down,
            "work never dozes"
        );
    }
}
