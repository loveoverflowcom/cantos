use super::{
    copy::{self, Key},
    view::{
        CopyState, GrantItem, IssueForm, TokenReveal, UnrecoverableNotice, FIRST_ACTION,
        REVEAL_HEADING,
    },
    *,
};
use crate::live::{ClientError, Command, Credentials, Phase, Reply, RequestState};
use cantos_contracts::*;
use leptos::prelude::*;

const WORK: &str = "10000000-0000-4000-8000-00000000000a";
const GRANT: &str = "40000000-0000-4000-8000-000000000001";
const SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const CREATED: &[u8] = include_bytes!(
    "../../../../../../packages/contracts/fixtures/delegation/v1/issue-created.json"
);
const REPLAY: &[u8] =
    include_bytes!("../../../../../../packages/contracts/fixtures/delegation/v1/issue-replay.json");
const PAGE: &[u8] =
    include_bytes!("../../../../../../packages/contracts/fixtures/delegation/v1/grant-page.json");

fn request() -> IssueDelegationRequest {
    serde_json::from_slice(include_bytes!(
        "../../../../../../packages/contracts/fixtures/delegation/v1/issue-request.json"
    ))
    .unwrap()
}
fn issue() -> Command {
    Command::IssueDelegation {
        work: WORK.into(),
        request: request(),
    }
}
fn page() -> DelegationGrantPage {
    serde_json::from_slice(PAGE).unwrap()
}
fn api(status: u16, body: &str) -> ClientError {
    ClientError::http(status, body.as_bytes())
}
fn issues(status: u16, code: &str, issues: &[(&str, &str)]) -> ClientError {
    let issues: Vec<_> = issues
        .iter()
        .map(|(path, rule)| serde_json::json!({"path": path, "rule": rule}))
        .collect();
    api(
        status,
        &serde_json::json!({"code": code, "current_revision": null, "issues": issues}).to_string(),
    )
}
fn issued(reply: &Reply) -> &IssuedGrant {
    match reply {
        Reply::DelegationIssued(issued) => issued,
        _ => panic!("not an issuance"),
    }
}

#[test]
fn owner_routes_come_from_shared_templates_and_reject_unsafe_ids() {
    let list = Command::Delegations {
        work: WORK.into(),
        after: None,
    };
    assert_eq!(
        list.path().unwrap(),
        format!("/api/v1/studio/works/{WORK}/delegations?limit=20")
    );
    assert_eq!((list.method(), list.json().unwrap()), ("GET", None));
    let next = Command::Delegations {
        work: WORK.into(),
        after: Some(GRANT.into()),
    };
    assert_eq!(
        next.path().unwrap(),
        format!("/api/v1/studio/works/{WORK}/delegations?limit=20&after={GRANT}")
    );
    assert_eq!(
        issue().path().unwrap(),
        routes::STUDIO_WORK_DELEGATIONS.replace("{work_id}", WORK)
    );
    assert_eq!(issue().method(), "POST");
    assert!(issue().is_write());
    let body: serde_json::Value = serde_json::from_str(&issue().json().unwrap().unwrap()).unwrap();
    assert_eq!(body, serde_json::to_value(request()).unwrap());
    assert!(body.get("grantor_id").is_none());
    let revoke = Command::RevokeDelegation {
        work: WORK.into(),
        grant: GRANT.into(),
    };
    assert_eq!(
        revoke.path().unwrap(),
        format!("/api/v1/studio/works/{WORK}/delegations/{GRANT}")
    );
    assert_eq!(revoke.method(), "DELETE");
    assert!(revoke.expects_no_content());
    // The browser never presents a delegation bearer; owner routes ride the Studio session.
    for command in [&list, &next, &issue(), &revoke] {
        assert_eq!(command.credentials(), Credentials::SameOrigin);
    }
    for bad in [
        "../works",
        "",
        "40000000-0000-4000-8000-00000000000A",
        "x?limit=50",
    ] {
        let unsafe_commands = [
            Command::Delegations {
                work: bad.into(),
                after: None,
            },
            Command::Delegations {
                work: WORK.into(),
                after: Some(bad.into()),
            },
            Command::RevokeDelegation {
                work: WORK.into(),
                grant: bad.into(),
            },
        ];
        for command in unsafe_commands {
            assert_eq!(command.path(), Err(ClientError::InvalidInput), "{bad}");
        }
    }
}

#[test]
fn secret_is_revealed_only_by_201_and_never_by_a_200_replay_or_debug() {
    let created = issue().decode(201, CREATED).unwrap();
    let token = issued(&created).token.as_ref().unwrap();
    assert_eq!(token.reveal(), format!("cantos_dlg_{SECRET}"));
    for debug in [format!("{created:?}"), format!("{token:?}")] {
        assert!(!debug.contains(SECRET), "{debug}");
    }
    let replay = issue().decode(200, REPLAY).unwrap();
    assert_eq!(issued(&replay).token, None);
    assert_eq!(issued(&replay).grant, issued(&created).grant);
    // Status and secret must agree; a malformed secret is never accepted.
    for (status, body) in [(200, CREATED), (201, REPLAY)] {
        assert_eq!(
            issue().decode(status, body),
            Err(ClientError::Protocol { status })
        );
    }
    let mut response: serde_json::Value = serde_json::from_slice(CREATED).unwrap();
    for bad in [
        format!("cantos_dlg_{}", SECRET.to_uppercase()),
        format!("cantos_dlg_{}", &SECRET[1..]),
        SECRET.to_owned(),
        format!("cantos_prd_{SECRET}"),
    ] {
        response["token"] = bad.into();
        let body = serde_json::to_vec(&response).unwrap();
        assert_eq!(
            issue().decode(201, &body),
            Err(ClientError::Protocol { status: 201 })
        );
    }
    assert_eq!(
        issue().decode(
            422,
            br#"{"code":"invalid_catalog","current_revision":null,"issues":[]}"#
        ),
        Err(issues(422, "invalid_catalog", &[]))
    );
}

#[test]
fn issuance_reply_must_answer_the_exact_request() {
    let original: serde_json::Value = serde_json::from_slice(CREATED).unwrap();
    let mut reordered = original.clone();
    reordered["grant"]["actions"] =
        serde_json::json!(["content:write", "work:read", "content:read"]);
    assert!(issue()
        .decode(201, &serde_json::to_vec(&reordered).unwrap())
        .is_ok());
    for (field, value) in [
        (
            "id",
            serde_json::json!("40000000-0000-4000-8000-000000000009"),
        ),
        (
            "work_id",
            serde_json::json!("10000000-0000-4000-8000-000000000001"),
        ),
        ("delegate_actor_id", serde_json::json!("agent-other")),
        ("expires_at", serde_json::json!(1792800000001_i64)),
        ("actions", serde_json::json!(["work:read"])),
    ] {
        let mut changed = original.clone();
        changed["grant"][field] = value;
        assert_eq!(
            issue().decode(201, &serde_json::to_vec(&changed).unwrap()),
            Err(ClientError::Protocol { status: 201 }),
            "{field}"
        );
    }
    let mut foreign = page();
    foreign.items[1].work_id = "10000000-0000-4000-8000-000000000001".into();
    let list = Command::Delegations {
        work: WORK.into(),
        after: None,
    };
    assert_eq!(
        list.check_reply(&Reply::Delegations(foreign)),
        Err(ClientError::ReplyMismatch)
    );
    assert!(list.decode(200, PAGE).is_ok());
}

#[test]
fn acknowledgement_requires_the_signed_in_grantor() {
    let created = issue().decode(201, CREATED).unwrap();
    let alice = RequestState::default().context("alice", WORK).unwrap();
    let (sending, ticket) = alice.begin(issue()).unwrap();
    let ready = sending.finish(ticket, Ok(created.clone()));
    assert_eq!(ready.phase(), Phase::Ready);
    assert_eq!(progress(&ready), Some(Key::IssuedOk));
    let bob = RequestState::default().context("bob", WORK).unwrap();
    let (sending, ticket) = bob.begin(issue()).unwrap();
    let refused = sending.finish(ticket, Ok(created));
    assert_eq!(refused.phase(), Phase::Ambiguous);
    assert_eq!(refused.error(), Some(&ClientError::SessionChanged));
}

#[test]
fn lost_response_retries_the_same_operation_and_a_replay_is_unrecoverable() {
    let state = RequestState::default().context("alice", WORK).unwrap();
    let (sending, ticket) = state.begin(issue()).unwrap();
    assert_eq!(progress(&sending), Some(Key::Issuing));
    let lost = sending.finish(ticket, Err(ClientError::Transport));
    assert_eq!(lost.phase(), Phase::Ambiguous);
    assert_eq!(progress(&lost), Some(Key::Ambiguous));
    assert_eq!(problems(lost.error().unwrap()), vec![Problem::Interrupted]);
    // No other request may start; the exact snapshot is retried.
    assert!(lost
        .begin(Command::Delegations {
            work: WORK.into(),
            after: None
        })
        .is_none());
    let (retrying, ticket) = lost.retry().unwrap();
    assert_eq!(
        retrying.pending().unwrap().json().unwrap(),
        issue().json().unwrap()
    );
    let replay = issue().decode(200, REPLAY).unwrap();
    let settled = retrying.finish(ticket, Ok(replay.clone()));
    assert_eq!(settled.phase(), Phase::Ready);
    assert_eq!(progress(&settled), Some(Key::Replayed));
    let grants = Grants::for_work(WORK).apply(&issue(), replay);
    assert_eq!(grants.reveal, None);
    assert_eq!(grants.unrecoverable.as_ref().unwrap().id, GRANT);
    assert_eq!(grants.items.len(), 1);
    // A definitive refusal on replay stays unresolved until the owner abandons it.
    let (again, ticket) = lost.retry().unwrap();
    let archived = again.finish(
        ticket,
        Err(issues(
            409,
            "catalog_conflict",
            &[("lifecycle", "archived")],
        )),
    );
    assert_eq!(archived.phase(), Phase::Ambiguous);
    let abandoned = archived.abandon().unwrap();
    assert_eq!(
        (abandoned.phase(), abandoned.pending().is_none()),
        (Phase::Idle, true)
    );
    assert!(abandoned
        .begin(Command::Delegations {
            work: WORK.into(),
            after: None
        })
        .is_some());
    assert!(settled.abandon().is_none());
    assert!(state.abandon().is_none());
}

#[test]
fn dismissing_or_revoking_clears_the_retained_secret_and_acknowledgement() {
    let created = issue().decode(201, CREATED).unwrap();
    let state = RequestState::default().context("alice", WORK).unwrap();
    let (sending, ticket) = state.begin(issue()).unwrap();
    let ready = sending.finish(ticket, Ok(created.clone()));
    let grants = Grants::for_work(WORK).apply(&issue(), created);
    let reveal = grants.reveal.clone().unwrap();
    assert_eq!(reveal.grant.id, GRANT);
    assert!(!format!("{grants:?}").contains(SECRET));
    assert_eq!(grants.dismiss_reveal().reveal, None);
    let forgotten = ready.forget_result().unwrap();
    assert_eq!((forgotten.phase(), forgotten.result()), (Phase::Idle, None));
    assert!(forgotten.forget_result().is_none());
    let revoke = Command::RevokeDelegation {
        work: WORK.into(),
        grant: GRANT.into(),
    };
    assert_eq!(revoke.decode(204, b""), Ok(Reply::Revoked));
    assert_eq!(
        revoke.decode(200, b"{}"),
        Err(ClientError::Protocol { status: 200 })
    );
    let revoked = grants.apply(&revoke, Reply::Revoked);
    assert_eq!(revoked.reveal, None);
    assert!(revoked.revoked_here(GRANT));
    assert_eq!(
        grant_state(&revoked.items[0], revoked.revoked_here(GRANT), 0),
        GrantState::Revoked
    );
    // The next authoritative read carries the server's revocation time and clears the mark.
    let mut listed = page();
    listed.items[0].revoked_at = Some(1792281600000);
    let read = revoked.apply(
        &Command::Delegations {
            work: WORK.into(),
            after: None,
        },
        Reply::Delegations(listed),
    );
    assert!(!read.revoked_here(GRANT));
    assert_eq!(read.items[0].revoked_at, Some(1792281600000));
}

#[test]
fn authoritative_refresh_drops_a_secret_revoked_outside_this_panel() {
    let grants = Grants::for_work(WORK).apply(&issue(), issue().decode(201, CREATED).unwrap());
    assert!(grants.reveal.is_some());
    let mut listed = page();
    listed.items[0].revoked_at = Some(1792281600000);
    let read = grants.apply(
        &Command::Delegations {
            work: WORK.into(),
            after: None,
        },
        Reply::Delegations(listed),
    );
    assert_eq!(read.reveal, None);
    assert_eq!(read.items[0].revoked_at, Some(1792281600000));
    assert_eq!(grant_state(&read.items[0], false, 0), GrantState::Revoked);
}

#[test]
fn a_page_without_the_revealed_grant_does_not_infer_revocation() {
    let grants = Grants::for_work(WORK).apply(&issue(), issue().decode(201, CREATED).unwrap());
    let mut listed = page();
    listed.items.remove(0);
    let read = grants.apply(
        &Command::Delegations {
            work: WORK.into(),
            after: None,
        },
        Reply::Delegations(listed),
    );
    assert_eq!(read.reveal, grants.reveal);
}

#[test]
fn late_issuance_cannot_reveal_a_secret_after_cancellation_or_context_change() {
    let state = RequestState::default().context("alice", WORK).unwrap();
    let (sending, ticket) = state.begin(issue()).unwrap();
    assert!(sending.begin(issue()).is_none());
    let created = issue().decode(201, CREATED).unwrap();
    let contexts = [
        sending.cancel().unwrap(),
        sending.context("bob", WORK).unwrap(),
        sending
            .context("alice", "10000000-0000-4000-8000-000000000001")
            .unwrap(),
    ];
    for context in contexts {
        let late = context.finish(ticket, Ok(created.clone()));
        assert_eq!(late.phase(), Phase::Ambiguous);
        assert_eq!(late.result(), None);
        assert_eq!(
            late.pending().unwrap().json().unwrap(),
            issue().json().unwrap()
        );
        assert_eq!(late.pending().unwrap().path(), issue().path());
    }
    let another_actor = sending.context("bob", WORK).unwrap();
    assert!(!another_actor.can_retry());
    assert!(another_actor.retry().is_none());
    let cancelled = sending.cancel().unwrap();
    let (retrying, next_ticket) = cancelled.retry().unwrap();
    let stale = retrying.finish(ticket, Ok(created));
    assert_eq!(stale.phase(), Phase::Pending);
    assert_eq!(stale.result(), None);
    let ready = stale.finish(next_ticket, issue().decode(200, REPLAY));
    assert_eq!(ready.phase(), Phase::Ready);
    assert_eq!(issued(ready.result().unwrap()).token, None);
}

#[test]
fn grant_pages_append_in_id_order_and_other_works_never_change_facts() {
    let mut first = page();
    let second = DelegationGrantPage {
        items: vec![first.items.remove(1)],
        next_after: None,
    };
    first.next_after = Some(GRANT.into());
    let list = |after: Option<&str>| Command::Delegations {
        work: WORK.into(),
        after: after.map(str::to_owned),
    };
    let grants = Grants::for_work(WORK)
        .apply(&list(None), Reply::Delegations(first))
        .apply(&list(Some(GRANT)), Reply::Delegations(second.clone()));
    assert!(grants.loaded);
    assert_eq!(grants.next_after, None);
    let ids: Vec<_> = grants.items.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, [GRANT, "40000000-0000-4000-8000-000000000002"]);
    let other = Grants::for_work("10000000-0000-4000-8000-000000000001");
    assert_eq!(other.apply(&list(None), Reply::Delegations(second)), other);
    assert_eq!(
        other.apply(&issue(), issue().decode(201, CREATED).unwrap()),
        other
    );
    // A fresh first page replaces, never merges with, stale rows.
    let reloaded = grants.apply(
        &list(None),
        Reply::Delegations(DelegationGrantPage {
            items: vec![],
            next_after: None,
        }),
    );
    assert!(reloaded.loaded && reloaded.items.is_empty());
    assert!(grants.forbid().forbidden);
}

#[test]
fn every_contract_refusal_maps_to_its_field_or_the_form() {
    let cases = [
        (
            "delegate_actor_id",
            "invalid_actor",
            Problem::InvalidActor,
            Some(Field::Delegate),
        ),
        (
            "delegate_actor_id",
            "self_delegation",
            Problem::SelfDelegation,
            Some(Field::Delegate),
        ),
        (
            "delegate_actor_id",
            "unavailable_actor",
            Problem::UnavailableActor,
            Some(Field::Delegate),
        ),
        (
            "actions",
            "empty",
            Problem::EmptyActions,
            Some(Field::Actions),
        ),
        (
            "actions",
            "duplicate_action",
            Problem::DuplicateAction,
            Some(Field::Actions),
        ),
        (
            "expires_at",
            "invalid_expiry",
            Problem::InvalidExpiry,
            Some(Field::Expiry),
        ),
    ];
    for (path, rule, problem, field) in cases {
        let found = problems(&issues(422, "invalid_catalog", &[(path, rule)]));
        assert_eq!(found, vec![problem.clone()]);
        assert_eq!(problem.field(), field);
        assert_eq!(field_problem(&found, field.unwrap()), Some(problem));
    }
    let several = problems(&issues(
        422,
        "invalid_catalog",
        &[
            ("delegate_actor_id", "self_delegation"),
            ("actions", "empty"),
            ("id", "invalid_uuid"),
        ],
    ));
    assert_eq!(
        field_problem(&several, Field::Delegate),
        Some(Problem::SelfDelegation)
    );
    assert_eq!(
        field_problem(&several, Field::Actions),
        Some(Problem::EmptyActions)
    );
    assert_eq!(field_problem(&several, Field::Expiry), None);
    assert!(several.contains(&Problem::Unrecognized {
        path: "id".into(),
        rule: "invalid_uuid".into()
    }));
    let form = [
        (
            issues(409, "catalog_conflict", &[("id", "duplicate_id")]),
            Problem::DuplicateGrantId,
        ),
        (
            issues(409, "catalog_conflict", &[("lifecycle", "archived")]),
            Problem::Archived,
        ),
        (
            issues(409, "operation_reused", &[]),
            Problem::OperationReused,
        ),
        (issues(409, "unauthenticated", &[]), Problem::SignIn),
        (issues(401, "unauthenticated", &[]), Problem::SignIn),
        (issues(403, "forbidden", &[]), Problem::NotOwner),
        (issues(404, "not_found", &[]), Problem::NotFound),
        (issues(400, "invalid_request", &[]), Problem::Malformed),
        (issues(503, "unavailable", &[]), Problem::Unavailable),
        (
            ClientError::Protocol { status: 201 },
            Problem::UnexpectedReply,
        ),
        (ClientError::ReplyMismatch, Problem::UnexpectedReply),
        (ClientError::InvalidInput, Problem::InvalidInput),
        (ClientError::Cancelled, Problem::Interrupted),
    ];
    for (error, problem) in form {
        assert_eq!(problems(&error), vec![problem.clone()], "{error:?}");
        assert_eq!(problem.field(), None);
    }
    for problem in [
        Problem::InvalidActor,
        Problem::Unrecognized {
            path: "id".into(),
            rule: "invalid_uuid".into(),
        },
    ] {
        for en in [false, true] {
            assert!(!copy::problem(&problem, en).trim().is_empty());
        }
    }
}

#[test]
fn drafts_are_canonical_and_lifetimes_never_exceed_thirty_days() {
    let draft = IssueDraft {
        delegate: "  agent-editor ".into(),
        ..IssueDraft::default()
    };
    assert_eq!(
        draft.actions,
        [DelegationAction::WorkRead, DelegationAction::ContentRead]
    );
    let write = draft
        .with_action(DelegationAction::ContentWrite, true)
        .with_action(DelegationAction::ContentWrite, true)
        .with_action(DelegationAction::WorkRead, false);
    assert_eq!(
        write.actions,
        [
            DelegationAction::ContentRead,
            DelegationAction::ContentWrite
        ]
    );
    let now = 1_792_195_200_000;
    let request = write
        .request("operation".into(), "grant".into(), now)
        .unwrap();
    assert_eq!(request.delegate_actor_id, "agent-editor");
    assert_eq!(request.expires_at, now + 7 * 86_400_000);
    let empty = write
        .with_action(DelegationAction::ContentRead, false)
        .with_action(DelegationAction::ContentWrite, false);
    // The server, not the browser, refuses an empty selection.
    assert!(empty
        .request("o".into(), "g".into(), now)
        .unwrap()
        .actions
        .is_empty());
    for lifetime in Lifetime::ALL {
        assert_eq!(Lifetime::parse(lifetime.value()), Some(lifetime));
        let expiry = lifetime.expires_at(now).unwrap();
        assert!(expiry > now && expiry - now <= 30 * 86_400_000);
    }
    assert_eq!(Lifetime::parse("31"), None);
    let overflow = IssueDraft {
        lifetime: Lifetime::Maximum,
        ..IssueDraft::default()
    };
    assert_eq!(
        overflow.request("o".into(), "g".into(), i64::MAX),
        Err(ClientError::InvalidInput)
    );
}

#[test]
fn grant_state_prefers_server_revocation_then_expiry() {
    let grants = page();
    let (active, revoked) = (&grants.items[0], &grants.items[1]);
    assert_eq!(
        grant_state(active, false, active.expires_at - 1),
        GrantState::Active
    );
    assert_eq!(
        grant_state(active, false, active.expires_at),
        GrantState::Expired
    );
    assert_eq!(grant_state(active, true, 0), GrantState::Revoked);
    assert_eq!(grant_state(revoked, false, 0), GrantState::Revoked);
    assert_eq!(display_time(1792195200000, false), "17/10/2026 00:00 UTC");
    assert_eq!(display_time(-86_400_000, true), "31/12/1969 00:00 UTC");
}

fn render(view: impl FnOnce() -> AnyView) -> String {
    let owner = Owner::new();
    owner.with(|| view().to_html())
}

#[test]
fn reveal_shows_the_secret_once_read_only_with_copy_and_warning() {
    let reveal = Grants::for_work(WORK)
        .apply(&issue(), issue().decode(201, CREATED).unwrap())
        .reveal
        .unwrap();
    for en in [false, true] {
        let calls = RwSignal::new(0usize);
        let copy_state = RwSignal::new(CopyState::Manual);
        let html = render(|| {
            view! { <TokenReveal reveal=reveal.clone() english=RwSignal::new(en) copy_state=copy_state.into()
                on_copy=Callback::new(move |()| calls.update(|n| *n += 1)) on_dismiss=Callback::new(|()| {})/> }
            .into_any()
        });
        assert_eq!(html.matches(SECRET).count(), 1, "{html}");
        assert!(html.contains(&format!("id=\"{REVEAL_HEADING}\"")));
        assert!(html.contains("data-tone=\"warning\""));
        for attribute in ["readonly", "autocomplete=\"off\"", "spellcheck=\"false\""] {
            assert!(html.contains(attribute), "{attribute}");
        }
        for key in [
            Key::RevealTitle,
            Key::RevealBody,
            Key::RevealUse,
            Key::Copy,
            Key::Dismiss,
            Key::CopyManual,
        ] {
            assert!(html.contains(&escape(copy::text(key, en))), "{key:?}");
        }
        assert!(html.contains("agent-editor"));
        assert_eq!(calls.get_untracked(), 0);
    }
}

/// The opening tag of the element with `id`.
fn tag<'a>(html: &'a str, id: &str) -> &'a str {
    let start = html.find(&format!("id=\"{id}\"")).unwrap();
    let open = html[..start].rfind('<').unwrap();
    &html[open..start + html[start..].find('>').unwrap()]
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[test]
fn grant_rows_state_text_and_named_revocation() {
    let grants = page();
    for en in [false, true] {
        let row = |grant: DelegationGrantResponse, here: bool, now: i64| {
            render(move || {
                view! { <GrantItem grant=grant revoked_here=here now_ms=now english=RwSignal::new(en)
                    blocked=Signal::derive(|| false) on_revoke=Callback::new(|_| {})/> }
                .into_any()
            })
        };
        let active = row(grants.items[0].clone(), false, 0);
        assert!(active.contains(copy::state(GrantState::Active, en)));
        assert!(active.contains("data-tone=\"success\""));
        assert!(active.contains(&format!("id=\"delegation-revoke-{GRANT}\"")));
        assert!(active.contains(&format!("{}</span>", "agent-editor")));
        assert!(active.contains(copy::text(Key::RevokeFor, en)));
        for label in ["work:read", "content:read", "content:write"] {
            assert!(active.contains(label));
        }
        assert!(active.contains("17/10/2026 00:00 UTC") && active.contains("24/10/2026 00:00 UTC"));
        let expired = row(grants.items[0].clone(), false, grants.items[0].expires_at);
        assert!(expired.contains(copy::state(GrantState::Expired, en)));
        assert!(expired.contains("delegation-revoke-"));
        let revoked = row(grants.items[1].clone(), false, 0);
        assert!(revoked.contains(copy::state(GrantState::Revoked, en)));
        assert!(revoked.contains("18/10/2026 00:00 UTC"));
        assert!(!revoked.contains("delegation-revoke-"));
        let just = row(grants.items[0].clone(), true, 0);
        assert!(just.contains(&escape(copy::text(Key::RevokedNow, en))));
        assert!(!just.contains("delegation-revoke-"));
    }
}

#[test]
fn issue_form_projects_server_refusals_onto_their_fields() {
    let problems_found = problems(&issues(
        422,
        "invalid_catalog",
        &[
            ("delegate_actor_id", "self_delegation"),
            ("actions", "empty"),
            ("expires_at", "invalid_expiry"),
        ],
    ));
    for en in [false, true] {
        let html = render(|| {
            let problems = RwSignal::new(problems_found.clone());
            view! { <IssueForm draft=RwSignal::new(IssueDraft::default()) problems=problems english=RwSignal::new(en)
                blocked=Signal::derive(|| false) archived=Signal::derive(|| false) on_submit=Callback::new(|()| {})/> }
            .into_any()
        });
        for problem in [
            Problem::SelfDelegation,
            Problem::EmptyActions,
            Problem::InvalidExpiry,
        ] {
            assert!(
                html.contains(&escape(&copy::problem(&problem, en))),
                "{problem:?}"
            );
        }
        assert!(html.contains("aria-errormessage=\"delegation-delegate-error\""));
        assert!(html.contains("aria-errormessage=\"delegation-lifetime-error\""));
        assert!(
            html.contains("aria-describedby=\"delegation-actions-help delegation-actions-error\"")
        );
        assert!(tag(&html, "delegation-delegate").contains("aria-invalid=\"true\""));
        assert!(tag(&html, "delegation-lifetime").contains("aria-invalid=\"true\""));
        assert!(tag(&html, "delegation-delegate").starts_with("<input "));
        assert!(tag(&html, "delegation-lifetime").starts_with("<select "));
        for id in ["delegation-delegate", "delegation-lifetime", FIRST_ACTION] {
            assert!(html.contains(&format!("for=\"{id}\"")), "{id}");
        }
        let read = tag(&html, FIRST_ACTION);
        assert!(read.contains("type=\"checkbox\"") && read.contains(" checked"));
        assert!(read.contains("aria-errormessage=\"delegation-actions-error\""));
        let write = tag(&html, "delegation-action-content-write");
        assert!(write.contains("type=\"checkbox\"") && !write.contains(" checked"));
        assert!(!tag(&html, "delegation-actions-error").contains("hidden"));
        assert!(html.contains("value=\"7\" selected"));
        let submit = tag(&html, "delegation-issue-submit");
        assert!(
            submit.contains("type=\"submit\"") && !submit.contains(" disabled"),
            "{submit}"
        );
        let archived = render(|| {
            view! { <IssueForm draft=RwSignal::new(IssueDraft::default()) problems=RwSignal::new(vec![]) english=RwSignal::new(en)
                blocked=Signal::derive(|| false) archived=Signal::derive(|| true) on_submit=Callback::new(|()| {})/> }
            .into_any()
        });
        assert!(tag(&archived, "delegation-issue-submit").contains(" disabled"));
        assert!(archived.contains(&escape(copy::text(Key::Archived, en))));
        assert!(!archived.contains("aria-invalid=\"true\""));
        assert!(tag(&archived, "delegation-actions-error").contains("hidden"));
    }
}

#[test]
fn issue_form_freezes_pending_fields_and_requires_saving_the_visible_secret() {
    for en in [false, true] {
        assert!(!copy::text(Key::SaveTokenFirst, en).trim().is_empty());
        for pending in [false, true] {
            for visible in [false, true] {
                let html = render(|| {
                    view! { <IssueForm draft=RwSignal::new(IssueDraft::default()) problems=RwSignal::new(vec![])
                        english=RwSignal::new(en) blocked=Signal::derive(move || pending)
                        archived=Signal::derive(|| false) secret_visible=Signal::derive(move || visible)
                        on_submit=Callback::new(|()| {})/> }
                    .into_any()
                });
                let fields = html
                    .split("<fieldset")
                    .nth(1)
                    .unwrap()
                    .split('>')
                    .next()
                    .unwrap();
                assert!(fields.contains("class=\"delegation-inputs\""));
                assert_eq!(fields.contains(" disabled"), pending);
                assert_eq!(
                    tag(&html, "delegation-issue-submit").contains(" disabled"),
                    pending || visible
                );
                assert_eq!(
                    tag(&html, "delegation-issue-help").contains(" hidden"),
                    !visible
                );
                assert!(html.contains(&escape(copy::text(Key::SaveTokenFirst, en))));
            }
        }
    }
}

#[test]
fn replay_notice_explains_the_lost_secret_and_offers_revocation() {
    let grant = page().items.remove(0);
    for en in [false, true] {
        let html = render(|| {
            view! { <UnrecoverableNotice grant=grant.clone() english=RwSignal::new(en) blocked=Signal::derive(|| false)
                on_revoke=Callback::new(|_| {})/> }
            .into_any()
        });
        assert!(html.contains(&escape(copy::text(Key::UnrecoverableTitle, en))));
        assert!(html.contains(&escape(copy::text(Key::UnrecoverableBody, en))));
        assert!(html.contains("delegation-revoke-unrecoverable"));
        assert!(html.contains(GRANT));
        assert!(!html.contains("cantos_dlg_"));
    }
}

#[test]
fn lost_focus_returns_to_the_field_retry_or_status_that_explains_the_outcome() {
    use super::view::{DELEGATE_FIELD, FIRST_ACTION, LIFETIME_FIELD, LIST_HEADING, RETRY, STATUS};
    let refused = |rule: (&str, &str)| problems(&issues(422, "invalid_catalog", &[rule]));
    let list = Command::Delegations {
        work: WORK.into(),
        after: None,
    };
    let revoke = Command::RevokeDelegation {
        work: WORK.into(),
        grant: GRANT.into(),
    };
    assert_eq!(
        recovery_focus(
            &issue(),
            Phase::Failed,
            &refused(("delegate_actor_id", "self_delegation"))
        ),
        DELEGATE_FIELD
    );
    assert_eq!(
        recovery_focus(&issue(), Phase::Failed, &refused(("actions", "empty"))),
        FIRST_ACTION
    );
    assert_eq!(
        recovery_focus(
            &issue(),
            Phase::Failed,
            &refused(("expires_at", "invalid_expiry"))
        ),
        LIFETIME_FIELD
    );
    assert_eq!(
        recovery_focus(&issue(), Phase::Failed, &[Problem::Archived]),
        STATUS
    );
    for command in [&issue(), &revoke] {
        assert_eq!(
            recovery_focus(command, Phase::Ambiguous, &[Problem::Interrupted]),
            RETRY
        );
    }
    assert_eq!(recovery_focus(&list, Phase::Ready, &[]), LIST_HEADING);
    assert_eq!(
        recovery_focus(&list, Phase::Failed, &[Problem::Unavailable]),
        STATUS
    );
    // Every target exists in the rendered form or panel markup.
    let html = render(|| {
        view! { <IssueForm draft=RwSignal::new(IssueDraft::default()) problems=RwSignal::new(vec![]) english=RwSignal::new(false)
            blocked=Signal::derive(|| false) archived=Signal::derive(|| false) on_submit=Callback::new(|()| {})/> }
        .into_any()
    });
    for id in [DELEGATE_FIELD, FIRST_ACTION, LIFETIME_FIELD] {
        assert!(html.contains(&format!("id=\"{id}\"")), "{id}");
    }
}
