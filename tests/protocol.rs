use serde_json::{Value, json};
use sumi_skk::{Engine, Phase, dictionary::Dictionary, protocol::respond};
fn request(engine: &mut Engine, request: Value) -> Value {
    respond(engine, &Dictionary::demo(), &request.to_string())
}
#[test]
fn inspection_selection_commit_and_reset_do_not_need_synthetic_keys() {
    let mut e = Engine::default();
    for key in ["K", "a", "n", "j", "i", "Space"] {
        request(&mut e, json!({"id":0,"key":key}));
    }
    let state = request(&mut e, json!({"id":"read","op":"state"}));
    assert_eq!(state["result"]["candidate_index"], 0);
    assert_eq!(state["result"]["committed"], "");
    let selected = request(&mut e, json!({"id":1,"op":"select","index":1}));
    assert_eq!(selected["result"]["preedit"], "▼感じ");
    assert_eq!(selected["result"]["committed"], "");
    let bad = request(&mut e, json!({"id":2,"op":"select","index":99}));
    assert!(bad["result"]["error"].is_string());
    assert_eq!(e.snapshot().candidate_index, Some(1));
    let committed = request(&mut e, json!({"id":3,"op":"commit"}));
    assert_eq!(committed["result"]["committed"], "感じ");
    assert_eq!(
        request(&mut e, json!({"id":4,"op":"commit"}))["result"]["passthrough"],
        Value::Null
    );
    request(&mut e, json!({"id":5,"op":"key","key":"K"}));
    request(&mut e, json!({"id":6,"op":"reset"}));
    assert!(e.is_idle());
    assert_eq!(e.snapshot().phase, Phase::Direct);
}
#[test]
fn invalid_operation_fields_never_mutate_the_engine() {
    let mut e = Engine::default();
    request(&mut e, json!({"id":0,"key":"K"}));
    for value in [
        json!({"id":1,"op":"reset","key":"a"}),
        json!({"id":2,"op":"select","index":-1}),
        json!({"id":3,"op":"key"}),
        json!({"id":4,"op":"state","index":0}),
        json!({"id":5,"op":"unknown"}),
        json!({"id":6,"op":"commit","extra":true}),
    ] {
        assert!(request(&mut e, value)["error"].is_object());
        assert_eq!(e.snapshot().preedit, "▽k");
    }
}

#[test]
fn session_registration_preserves_preedit_and_makes_words_available() {
    use sumi_skk::{protocol::respond_session, session::Session};
    let mut session = Session::new(Engine::default(), Dictionary::demo());
    for key in ["M", "i", "c", "h", "i"] {
        respond_session(&mut session, &json!({"id":0,"key":key}).to_string());
    }
    let before = session.snapshot().preedit;
    let reply = respond_session(
        &mut session,
        &json!({"id":1,"op":"register","reading":"みち","text":"未知"}).to_string(),
    );
    assert_eq!(reply["result"]["preedit"], before);
    assert_eq!(reply["result"]["committed"], "");
    respond_session(&mut session, &json!({"id":2,"key":"Space"}).to_string());
    assert_eq!(
        respond_session(&mut session, &json!({"id":3,"op":"commit"}).to_string())["result"]["committed"],
        "未知"
    );
    assert!(
        respond_session(
            &mut session,
            &json!({"id":4,"op":"state","text":"bad"}).to_string()
        )["error"]
            .is_object()
    );
}
