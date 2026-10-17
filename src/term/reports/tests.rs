use super::*;

fn notice(id: Option<&str>, title: &str, body: &str) -> Notice {
    Notice {
        id: id.map(String::from),
        title: title.into(),
        body: body.into(),
        focus: true,
        report: false,
        close: false,
        expire: None,
    }
}

#[test]
fn a_session_posts_once_in_3_seconds_its_own_under_one_tag() {
    let mut notices = Notices::default();
    let start = Instant::now();
    let first = notices.turn(7, None, start).expect("the first post");
    assert_eq!((&*first.tag, first.focus, first.report), ("7", true, false));
    assert_eq!(notices.turn(7, None, start + Duration::from_secs(2)), None);
    let again = notices.turn(7, None, start + GAP).expect("past the gap");
    assert_eq!(&*again.tag, "7");
    assert_eq!(notices.posted.len(), 1, "the last takes its place");
}

#[test]
fn a_program_s_notifications_go_by_their_id_and_live_until_taken() {
    let mut notices = Notices::default();
    let start = Instant::now();
    let mut asked = notice(Some("build"), "T", "");
    asked.report = true;
    asked.focus = false;
    let posted = notices.turn(7, Some(&asked), start).expect("a post");
    assert_eq!(
        (&*posted.tag, posted.focus, posted.report),
        ("7:build", false, true)
    );
    let untagged = notices
        .turn(7, Some(&notice(None, "T", "")), start + GAP)
        .expect("a post");
    assert!(untagged.tag.starts_with("7#"), "{}", untagged.tag);
    assert_eq!(notices.alive(), ["build"]);
    let update = notices
        .turn(7, Some(&asked), start + GAP * 2)
        .expect("an update");
    assert!(
        notices.replaced(&posted.tag, posted.count),
        "a later post holds the tag"
    );
    assert!(!notices.replaced(&update.tag, update.count));
    assert_eq!(
        notices.take("7:build", Some(posted.count)),
        None,
        "an expiry takes its own post only"
    );
    assert_eq!(notices.take("7:build", Some(update.count)), Some(update));
    assert!(notices.alive().is_empty());
}

#[test]
fn a_session_keeps_its_last_64_notifications() {
    let mut notices = Notices::default();
    let start = Instant::now();
    for at in 0..=POSTED as u32 {
        let asked = notice(Some(format!("n{at}").as_str()), "T", "");
        notices
            .turn(7, Some(&asked), start + GAP * at)
            .expect("a post");
    }
    assert_eq!(notices.posted.len(), POSTED);
    assert!(!notices.alive().contains(&"n0"), "the oldest went");
    assert!(notices.alive().contains(&"n64"));
}

#[test]
fn a_notification_says_its_text_as_text() {
    let asked = notice(None, "Build\u{202e} done", "Line 1\n\nLine 2\u{200b}");
    assert_eq!(said(&asked), "Build done\nLine 1\nLine 2");
    assert_eq!(finished(Some(0)), "Command finished");
    assert_eq!(finished(None), "Command finished");
    assert_eq!(finished(Some(2)), "Command exited with 2");
}
