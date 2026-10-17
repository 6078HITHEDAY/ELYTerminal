use super::*;
use crate::term::notify::Progress;

fn encoded(text: &[u8]) -> String {
    BASE64.encode(text)
}

/// What `body` says when its sequence is as long as its frame makes it.
fn said(body: &[u8]) -> Option<Status> {
    read(body, body.len() + 9)
}

fn record(state: State, app: Option<&str>) -> Record {
    Record {
        state,
        kind: None,
        progress: None,
        app: app.map(str::to_string),
        title: None,
        msg: None,
    }
}

fn reported(body: &str) -> Record {
    match said(body.as_bytes()) {
        Some(Status::Report(_, record)) => record,
        other => panic!("{body}: {other:?}"),
    }
}

#[test]
fn the_spec_s_examples_read_as_it_means_them() {
    let body = b"state=blocked:kind=permission:app=terraform:msg=QXBwbHkgMyB0byBhZGQsIDEgdG8gY2hhbmdlLCAwIHRvIGRlc3Ryb3k/";
    let blocked = Record {
        kind: Some(Kind::Permission),
        msg: Some("Apply 3 to add, 1 to change, 0 to destroy?".into()),
        ..record(State::Blocked, Some("terraform"))
    };
    assert_eq!(said(body), Some(Status::Report(String::new(), blocked)));
    let body = b"state=working:id=us-east:title=VVMgRWFzdA==:progress=40:msg=UHVzaGluZyBpbWFnZQ==";
    let working = Record {
        progress: Some(40),
        title: Some("US East".into()),
        msg: Some("Pushing image".into()),
        ..record(State::Working, None)
    };
    assert_eq!(said(body), Some(Status::Report("us-east".into(), working)));
    assert_eq!(
        said(b"state=clear:id=us-east"),
        Some(Status::Clear(Some("us-east".into())))
    );
    assert_eq!(said(b"state=clear"), Some(Status::Clear(None)));
    assert_eq!(said(b"?"), Some(Status::Query));
    assert_eq!(
        reported("state=done:msg=VXBncmFkZWQgMTIgcGFja2FnZXM")
            .msg
            .as_deref(),
        Some("Upgraded 12 packages"),
        "padding is optional"
    );
}

#[test]
fn a_malformed_pair_is_skipped_the_last_of_a_key_wins_and_unknown_keys_pass() {
    let record = reported(" junk : =x : state = idle :Key=v:app=a b:color=red:state=done:app=ok ");
    assert_eq!(record, self::record(State::Done, Some("ok")));
}

#[test]
fn a_sequence_that_breaks_a_rule_drops_whole() {
    let long = |key: &str, length: usize| format!("state=done:{key}={}", "a".repeat(length));
    let drops = [
        "app=x".to_string(),
        "state=sleeping".into(),
        "state=done:id=".into(),
        "state=done:id=a//b".into(),
        "state=done:id=a/b/c/d/e/f/g/h/i".into(),
        format!("state=done:id={}", "a".repeat(33)),
        format!("state=done:{}=1", "k".repeat(17)),
        long("app", 33),
        long("title", 257),
        long("msg", 2733),
        "state=done:msg=A".into(),
        format!("state=done:msg={}", encoded(b"a\nb")),
        format!("state=done:msg={}", encoded("a\u{85}b".as_bytes())),
        format!("state=done:msg={}", encoded(&[0xff])),
        format!("state=done:title={}", encoded(&[b'a'; 193])),
        format!("state=done:x={}", "a".repeat(4096 - 9 - 12)),
    ];
    for body in drops {
        assert_eq!(said(body.as_bytes()), None, "{body}");
    }
    assert!(said(format!("state=done:x={}", "a".repeat(4096 - 9 - 14)).as_bytes()).is_some());
}

#[test]
fn kind_progress_and_app_count_only_where_the_spec_has_them() {
    let done = reported("state=done:kind=auth:progress=5");
    assert_eq!((done.kind, done.progress), (None, None));
    let blocked = reported("state=blocked:kind=nope:progress=101");
    assert_eq!((blocked.kind, blocked.progress), (None, None));
    assert_eq!(reported("state=working:progress=007").progress, Some(7));
    assert_eq!(reported("state=working:progress=+5").progress, None);
    assert_eq!(reported("state=blocked:kind=auth").kind, Some(Kind::Auth));
    assert_eq!(
        reported("state=working:app=a.b+c_d-e").app.as_deref(),
        Some("a.b+c_d-e")
    );
    assert_eq!(reported("state=working:app=a,b").app, None);
}

#[test]
fn records_replace_and_clear_as_the_spec_says() {
    let mut records = Records::default();
    records.report(String::new(), record(State::Working, Some("deploy")));
    records.report("us-east".into(), record(State::Working, None));
    records.report("us-east/push".into(), record(State::Blocked, None));
    records.report("us-easter".into(), record(State::Done, Some("other")));
    records.report("us-east".into(), record(State::Done, None));
    let ids: Vec<&str> = records.list.iter().map(|(id, ..)| id.as_str()).collect();
    assert_eq!(ids, ["", "us-east", "us-east/push", "us-easter"]);
    let east = records
        .list
        .iter()
        .find(|(id, ..)| id == "us-east")
        .map(|(_, record, _)| record.clone());
    assert_eq!(
        east,
        Some(record(State::Done, None)),
        "a report replaces its record whole"
    );
    records.clear(Some("us-east"));
    let ids: Vec<&str> = records.list.iter().map(|(id, ..)| id.as_str()).collect();
    assert_eq!(
        ids,
        ["", "us-easter"],
        "clear takes the record and those under it"
    );
    records.clear(None);
    assert!(records.list.is_empty());
}

#[test]
fn a_new_prompt_leaves_done_and_error() {
    let mut records = Records::default();
    for (id, state) in [
        ("a", State::Idle),
        ("b", State::Working),
        ("c", State::Done),
        ("d", State::Blocked),
        ("e", State::Error),
    ] {
        records.report(id.into(), record(state, None));
    }
    records.prompt();
    let ids: Vec<&str> = records.list.iter().map(|(id, ..)| id.as_str()).collect();
    assert_eq!(ids, ["c", "e"]);
}

#[test]
fn past_the_cap_the_record_updated_longest_ago_goes() {
    let mut records = Records::default();
    for at in 0..RECORDS {
        records.report(format!("r{at}"), record(State::Working, None));
    }
    records.report("r0".into(), record(State::Done, None));
    records.report("new".into(), record(State::Working, None));
    assert_eq!(records.list.len(), RECORDS);
    assert!(
        records.list.iter().all(|(id, ..)| id != "r1"),
        "r1 was updated longest ago"
    );
    assert!(records.list.iter().any(|(id, ..)| id == "r0"));
}

#[test]
fn a_clear_that_breaks_a_rule_drops_whole() {
    let drops = [
        "state=clear:msg=Cg==".to_string(),
        "state=clear:msg=A".into(),
        format!("state=clear:app={}", "a".repeat(33)),
        format!("state=clear:title={}", "a".repeat(257)),
        "state=clear:id=a//b".into(),
    ];
    for body in drops {
        assert_eq!(said(body.as_bytes()), None, "{body}");
    }
}

#[test]
fn every_pair_counts_though_the_last_of_a_key_wins() {
    let drops = [
        format!("state=done:app={}:app=ok", "a".repeat(33)),
        "state=done:title=A:title=VVMgRWFzdA==".into(),
        "state=done:id=a//b:id=ok".into(),
        format!("state=done:msg={}:msg=b2s=", encoded(b"a\nb")),
    ];
    for body in drops {
        assert_eq!(said(body.as_bytes()), None, "{body}");
    }
    assert_eq!(reported("state=done:app=x:app=y").app.as_deref(), Some("y"));
    assert_eq!(
        reported("state=done:app=a,b:app=ok").app.as_deref(),
        Some("ok")
    );
}

#[test]
fn the_sequence_counts_from_osc_through_st() {
    assert_eq!(read(b"state=done", 4097), None);
    assert!(read(b"state=done", 4096).is_some());
}

#[test]
fn osc_9_4_maps_to_the_root_record_until_a_program_reports() {
    let root = |records: &Records| {
        records
            .list
            .iter()
            .find(|(id, ..)| id.is_empty())
            .map(|(_, record, _)| (record.state, record.progress))
    };
    let mut records = Records::default();
    records.progress(Progress::Set(Some(40)));
    assert_eq!(root(&records), Some((State::Working, Some(40))));
    records.progress(Progress::Pause(Some(5)));
    assert_eq!(root(&records), Some((State::Working, Some(5))));
    records.progress(Progress::Indeterminate);
    assert_eq!(root(&records), Some((State::Working, None)));
    records.progress(Progress::Error(Some(9)));
    assert_eq!(root(&records), Some((State::Error, None)));
    records.progress(Progress::Remove);
    assert_eq!(root(&records), None);
    records.report("a".into(), record(State::Idle, None));
    records.progress(Progress::Set(Some(1)));
    assert_eq!(root(&records), None, "a program reports through OSC 7501");
    records.reset();
    assert!(records.list.is_empty());
    records.progress(Progress::Set(Some(2)));
    assert_eq!(
        root(&records),
        Some((State::Working, Some(2))),
        "a full reset maps again"
    );
}
