use s1code::{
    decisions::*,
    domain::Metrics,
    generation::{Generator, Responses, Sse},
};
use serde_json::json;
use std::collections::BTreeMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn request() -> DecisionRequest {
    DecisionRequest {
        model: "jev-1.13.0".into(),
        state: json!({"task":"choose", "revision":"1"}),
        questions: BTreeMap::from([
            (
                "pick".into(),
                Question::Choice {
                    instructions: "Select useful action".into(),
                    criteria: BTreeMap::from([
                        ("read".into(), Some("Read evidence".into())),
                        ("escape".into(), None),
                    ]),
                },
            ),
            (
                "retain".into(),
                Question::Noul {
                    instructions: "Retain evidence?".into(),
                },
            ),
            (
                "relevance".into(),
                Question::Score {
                    instructions: "Rate relevance".into(),
                    criteria: vec!["low".into(), "high".into()],
                },
            ),
        ]),
    }
}
fn response() -> serde_json::Value {
    json!({"model":"jev-1.13.0","answers":{"pick":{"type":"choice","choice":"read","probabilities":{"read":0.8,"escape":0.2},"confidence":0.6},"retain":{"type":"noul","noul":0.2},"relevance":{"type":"score","score":0.75,"legend":{"0":"low","1":"high"},"probabilities":{"0":0.25,"1":0.75},"confidence":0.5}},"usage":{"input_tokens":123,"output_tokens":23}})
}

#[test]
fn validate_all_real_types_and_reject_bad_answers() {
    let r = request();
    assert!(validate(&r, &serde_json::from_value(response()).unwrap()).is_ok());
    for (field, value) in [
        ("/answers/pick/choice", json!("invented")),
        ("/answers/pick/probabilities/read", json!(2)),
        ("/answers/retain/noul", json!(-0.1)),
        ("/answers/relevance/score", json!(9)),
        ("/model", json!("jev-preview")),
    ] {
        let mut v = response();
        *v.pointer_mut(field).unwrap() = value;
        assert!(
            validate(&r, &serde_json::from_value(v).unwrap()).is_err(),
            "{field}"
        );
    }
    let mut v = response();
    v["answers"].as_object_mut().unwrap().remove("retain");
    assert!(validate(&r, &serde_json::from_value(v).unwrap()).is_err());
}
#[test]
fn both_token_budgets_and_status_retry_rules() {
    let r = request();
    assert!(check_budget(&r, 64000, 32000).is_ok());
    assert!(check_budget(&r, 1100, 32000).is_err());
    assert!(check_budget(&r, 64000, 1100).is_err());
    assert!(retryable(429) && retryable(529));
    assert!(!retryable(401) && !retryable(422));
    assert!(retry_delay(0, Some("3")).as_secs() >= 3);
}

async fn mock(responses: Vec<(u16, String)>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut requests = vec![];
        for (status, body) in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = vec![];
            let mut buf = [0; 4096];
            loop {
                let n = socket.read(&mut buf).await.unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buf[..n]);
                if let Some(i) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..i]);
                    let length = header
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|x| x.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= i + 4 + length {
                        break;
                    }
                }
            }
            requests.push(String::from_utf8(bytes).unwrap());
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\nRetry-After: 0\r\n\r\n{body}",
                body.len()
            );
            for chunk in response.as_bytes().chunks(7) {
                if let Err(error) = socket.write_all(chunk).await {
                    // A bounded reader intentionally closes oversized responses early.
                    assert!(matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                    ));
                    break;
                }
            }
        }
        requests
    });
    (format!("http://{addr}"), task)
}

#[tokio::test]
async fn jev_real_http_shape_retry_and_exact_cache() {
    let (url, server) = mock(vec![
        (429, "{}".into()),
        (200, response().to_string()),
        (200, response().to_string()),
    ])
    .await;
    let mut jev = Jev::new("test-only-key".into(), "jev-1.13.0", &url, 64000, 32000).unwrap();
    let r = request();
    let mut m = Metrics::default();
    let c = CancellationToken::new();
    jev.ask(r.state.clone(), r.questions.clone(), &c, &mut m)
        .await
        .unwrap();
    jev.ask(r.state.clone(), r.questions.clone(), &c, &mut m)
        .await
        .unwrap();
    assert_eq!(m.cache_hits, 1);
    assert_eq!(m.retries, 1);
    jev.ask(
        json!({"task":"different","revision":"2"}),
        r.questions,
        &c,
        &mut m,
    )
    .await
    .unwrap();
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].contains("Bearer test-only-key"));
    assert_eq!(m.decision_requests, 3);
    assert_eq!(m.decision_questions, 9);
}

#[tokio::test]
async fn openrouter_decisions_use_own_contract_and_pin_resolved_build() {
    let mut body = response();
    body["model"] = OPENROUTER_RESOLVED.into();
    body["provider"] = "TypeSafe".into();
    body["usage"]["cost"] = json!(0.01); // Synthetic transport fixture, not a measurement.
    body["answers"]["relevance"]
        .as_object_mut()
        .unwrap()
        .remove("legend");
    let (url, server) = mock(vec![(200, body.to_string())]).await;
    let mut adapter = Jev::new_openrouter(
        "test-router-key".into(),
        OPENROUTER_MODEL,
        OPENROUTER_RESOLVED,
        &format!("{url}/api/alpha/decisions"),
        64000,
        32000,
    )
    .unwrap();
    assert_eq!(adapter.total, 32000);
    let r = request();
    let mut metrics = Metrics::default();
    let result = adapter
        .ask(
            r.state.clone(),
            r.questions.clone(),
            &CancellationToken::new(),
            &mut metrics,
        )
        .await
        .unwrap();
    assert_eq!(result.model, OPENROUTER_RESOLVED);
    assert_eq!(result.usage.cost, Some(0.01));
    assert!(matches!(
        &result.answers["relevance"],
        Answer::Score { legend: None, .. }
    ));
    adapter
        .ask(
            r.state,
            r.questions,
            &CancellationToken::new(),
            &mut metrics,
        )
        .await
        .unwrap();
    assert_eq!(metrics.cache_hits, 1);
    let requests = server.await.unwrap();
    assert!(requests[0].starts_with("POST /api/alpha/decisions "));
    let sent: serde_json::Value =
        serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(sent["model"], OPENROUTER_MODEL);
    assert_eq!(
        sent["provider"],
        json!({"only":["typesafe"],"allow_fallbacks":false})
    );
    assert_eq!(sent["questions"]["pick"]["criteria"]["escape"], "escape");
    assert!(sent.get("messages").is_none());
}

#[tokio::test]
async fn openrouter_rejects_serving_drift_and_missing_confidence() {
    for field in ["model", "confidence"] {
        let mut body = response();
        body["model"] = OPENROUTER_RESOLVED.into();
        if field == "model" {
            body["model"] = "typesafe/jev-1.13-20990101".into();
        } else {
            body["answers"]["pick"]
                .as_object_mut()
                .unwrap()
                .remove("confidence");
        }
        let (url, server) = mock(vec![(200, body.to_string())]).await;
        let mut adapter = Jev::new_openrouter(
            "test".into(),
            OPENROUTER_MODEL,
            OPENROUTER_RESOLVED,
            &url,
            32000,
            32000,
        )
        .unwrap();
        let r = request();
        assert!(
            adapter
                .ask(
                    r.state,
                    r.questions,
                    &CancellationToken::new(),
                    &mut Metrics::default()
                )
                .await
                .is_err()
        );
        assert_eq!(server.await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn streaming_response_validates_terminal_and_structured_contract() {
    let body=[json!({"type":"response.output_text.delta","delta":"{\"message\":\"inspect\",\"actions\":["}),json!({"type":"response.output_text.delta","delta":"{\"type\":\"list\"}]}"}),json!({"type":"response.completed","response":{"status":"completed","model":"fixture","usage":{"input_tokens":10,"output_tokens":12,"input_tokens_details":{"cached_tokens":3}}}})].into_iter().map(|e|format!("data: {e}\n\n")).collect::<String>();
    let (url, server) = mock(vec![(200, body)]).await;
    let gen_ = Responses::new("test".into(), "fixture", &url).unwrap();
    let (tx, _) = mpsc::unbounded_channel();
    let result = gen_
        .generate(json!({"task":"test"}), &CancellationToken::new(), tx)
        .await
        .unwrap();
    assert_eq!(result.usage.cached_input_tokens, Some(3));
    let requests = server.await.unwrap();
    assert!(requests[0].contains("json_schema"));
    assert!(requests[0].contains("\"store\":false"));
}

#[test]
fn malformed_stream_never_becomes_action() {
    let mut s = Sse::default();
    assert!(s.push(b"data: not-json\n\n").is_err());
}

#[tokio::test]
async fn permanent_errors_are_not_retried_and_exhaustion_is_explicit() {
    for status in [401, 422] {
        let (url, server) = mock(vec![(status, "{}".into())]).await;
        let mut jev = Jev::new("test".into(), "jev-1.13.0", &url, 64000, 32000).unwrap();
        let r = request();
        let mut m = Metrics::default();
        assert!(
            jev.ask(r.state, r.questions, &CancellationToken::new(), &mut m)
                .await
                .is_err()
        );
        assert_eq!(m.decision_requests, 1);
        assert_eq!(server.await.unwrap().len(), 1);
    }
    let (url, server) = mock(vec![
        (529, "{}".into()),
        (503, "{}".into()),
        (429, "{}".into()),
    ])
    .await;
    let mut jev = Jev::new("test".into(), "jev-1.13.0", &url, 64000, 32000).unwrap();
    let r = request();
    let mut m = Metrics::default();
    let error = jev
        .ask(r.state, r.questions, &CancellationToken::new(), &mut m)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("exhausted"));
    assert_eq!(m.decision_requests, 3);
    assert_eq!(m.retries, 2);
    assert_eq!(server.await.unwrap().len(), 3);
}

#[tokio::test]
async fn missing_completion_and_pre_cancelled_requests_fail_closed() {
    let (url, server) = mock(vec![(
        200,
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"{}\"}\n\n".into(),
    )])
    .await;
    let provider = Responses::new("test".into(), "fixture", &url).unwrap();
    let (tx, _) = mpsc::unbounded_channel();
    assert!(
        provider
            .generate(json!({}), &CancellationToken::new(), tx)
            .await
            .is_err()
    );
    server.await.unwrap();
    let provider = Responses::new("test".into(), "fixture", "http://127.0.0.1:1").unwrap();
    let c = CancellationToken::new();
    c.cancel();
    let (tx, _) = mpsc::unbounded_channel();
    let error = provider.generate(json!({}), &c, tx).await.err().unwrap();
    assert!(error.to_string().contains("cancelled"));
}

fn claude_events(text: &str, stop: &str) -> Vec<serde_json::Value> {
    vec![
        json!({"type":"message_start","message":{"model":"claude-fixture","content":[],"usage":{"input_tokens":20,"output_tokens":1,"cache_read_input_tokens":3,"cache_creation_input_tokens":5}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"private-not-for-display"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":text}}),
        json!({"type":"content_block_stop","index":1}),
        json!({"type":"message_delta","delta":{"stop_reason":stop},"usage":{"output_tokens":8}}),
        json!({"type":"message_stop"}),
    ]
}
fn claude_wire(events: Vec<serde_json::Value>) -> String {
    events
        .into_iter()
        .map(|v| {
            format!(
                "event: {}\r\ndata: {v}\r\n\r\n",
                v["type"].as_str().unwrap()
            )
        })
        .collect()
}
#[tokio::test]
async fn claude_stream_contract_usage_and_hidden_content() {
    let text = json!({"message":"İncele","actions":[{"type":"list"}]}).to_string();
    let (url, server) = mock(vec![(200, claude_wire(claude_events(&text, "end_turn")))]).await;
    let generator =
        s1code::claude::Claude::new("fixture-anthropic-key".into(), "claude-fixture", &url)
            .unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let response = generator
        .generate(json!({"task":"inspect"}), &CancellationToken::new(), tx)
        .await
        .unwrap();
    assert_eq!(response.proposal.message, "İncele");
    assert_eq!(response.usage.output_tokens, Some(8)); // Cumulative, not 1 + 8.
    assert_eq!(response.usage.cached_input_tokens, Some(3));
    assert_eq!(response.usage.cache_creation_input_tokens, Some(5));
    let mut rendered = String::new();
    while let Some(delta) = rx.recv().await {
        rendered.push_str(&delta);
    }
    assert_eq!(rendered, text);
    assert!(!rendered.contains("private-not-for-display"));
    let requests = server.await.unwrap();
    assert!(requests[0].contains("x-api-key: fixture-anthropic-key"));
    assert!(requests[0].contains("anthropic-version: 2023-06-01"));
    assert!(!requests[0].contains("authorization:"));
    let body: serde_json::Value =
        serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert!(body["output_config"]["format"].is_null());
    assert_eq!(body["max_tokens"], 16384);
    assert_eq!(body["tool_choice"]["disable_parallel_tool_use"], true);
    assert_eq!(body["stream"], true);
    assert_eq!(
        body["messages"][0]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert!(
        body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "patch")
    );
    assert!(
        !body["output_config"]["format"]["schema"]
            .to_string()
            .contains("\"minimum\"")
    );
}
#[tokio::test]
async fn claude_rejects_partial_refused_or_malformed_streams() {
    let text = json!({"message":"inspect","actions":[{"type":"list"}]}).to_string();
    let valid = claude_events(&text, "end_turn");
    let mut absent_stop = valid.clone();
    absent_stop.pop();
    let mut orphan_delta = valid.clone();
    orphan_delta.remove(4);
    let mut duplicate_start = valid.clone();
    duplicate_start.insert(1, valid[0].clone());
    let mut index_mismatch = valid.clone();
    index_mismatch[5]["index"] = json!(99);
    let mut tool_block = valid.clone();
    tool_block[4]["content_block"]["type"] = "tool_use".into();
    let mut late_text = valid.clone();
    late_text.push(valid[5].clone());
    let mut cases = vec![
        absent_stop,
        orphan_delta,
        duplicate_start,
        index_mismatch,
        tool_block,
        late_text,
        claude_events(&text, "max_tokens"),
        claude_events(&text, "refusal"),
        claude_events("not JSON", "end_turn"),
        vec![json!({"type":"error"})],
    ]
    .into_iter()
    .map(claude_wire)
    .collect::<Vec<_>>();
    cases.push(format!("{}data: {{", claude_wire(valid)));
    for body in cases {
        let (url, server) = mock(vec![(200, body)]).await;
        let generator =
            s1code::claude::Claude::new("fixture-key".into(), "claude-fixture", &url).unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        assert!(
            generator
                .generate(json!({}), &CancellationToken::new(), tx)
                .await
                .is_err()
        );
        server.await.unwrap();
    }
}
#[tokio::test]
async fn claude_cancellation_interrupts_an_in_progress_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (seen, received) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0; 4096];
        assert!(socket.read(&mut buf).await.unwrap() > 0);
        let _ = seen.send(());
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    });
    let generator = s1code::claude::Claude::new(
        "fixture".into(),
        "claude-fixture",
        &format!("http://{addr}"),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let task = tokio::spawn(async move {
        let (tx, _rx) = mpsc::unbounded_channel();
        generator.generate(json!({}), &token, tx).await
    });
    received.await.unwrap();
    cancel.cancel();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    server.abort();
    let (url, server) = mock(vec![(401, "do-not-print-provider-body".into())]).await;
    let generator = s1code::claude::Claude::new("fixture".into(), "claude-fixture", &url).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let error = generator
        .generate(json!({}), &CancellationToken::new(), tx)
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("401") && !error.contains("do-not-print-provider-body"));
    assert_eq!(server.await.unwrap().len(), 1);
}
#[tokio::test]
async fn claude_http_fixture_drives_native_patch_and_real_verification() {
    use s1code::{
        domain::*,
        engine::{Engine, workspace_for},
        session::{Store, hash},
    };
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    s1code::demo::fixture(root.path()).unwrap();
    let original = std::fs::read_to_string(root.path().join("parser.py")).unwrap();
    let patch = original.replace("int(text.strip()[0])", "int(text.strip())");
    let run = json!({"type":"run","argv":["python3","-m","unittest","-v"],"verification":true});
    let actions = [
        run.clone(),
        json!({"type":"read","path":"parser.py","start":1,"lines":40}),
        json!({"type":"patch","edits":[{"path":"parser.py","before_hash":hash(original.as_bytes()),"content":patch}]}),
        run,
        json!({"type":"finish","summary":"Fixture checks passed"}),
    ];
    let bodies = actions
        .iter()
        .map(|a| (200, claude_wire(claude_tool_events(a.clone(), "tool_use"))))
        .collect();
    let (url, server) = mock(bodies).await;
    let config = RunConfig {
        generation_provider: "claude".into(),
        generation_model: "claude-fixture".into(),
        ..Default::default()
    };
    let (store, session) =
        Store::create(home.path(), root.path(), s1code::demo::TASK.into(), config).unwrap();
    let (events, mut rx) = mpsc::unbounded_channel();
    let (tx, input) = mpsc::unbounded_channel();
    let engine = Engine {
        workspace: workspace_for(&session).unwrap(),
        store,
        session,
        generator: std::sync::Arc::new(
            s1code::claude::Claude::new("fixture".into(), "claude-fixture", &url).unwrap(),
        ),
        cancel: CancellationToken::new(),
        events,
        input,
        interactive: true,
        approved: None,
    };
    let task = tokio::spawn(engine.run());
    let mut approvals = 0;
    let mut failed_checks = 0;
    while let Some(event) = rx.recv().await {
        if event.kind == "approval_required" {
            approvals += 1;
            tx.send(UiInput::Approve(
                event.data["candidate"]["id"].as_str().unwrap().into(),
            ))
            .unwrap();
        }
        if event.kind == "tool_result" && event.data["exit_code"] == 1 {
            failed_checks += 1;
        }
    }
    let session = task.await.unwrap().unwrap();
    assert_eq!(session.status, RunStatus::Completed);
    assert_eq!(approvals, 3);
    assert_eq!(failed_checks, 1);
    assert_eq!(session.metrics.generative_calls, 5);
    assert_eq!(session.metrics.simulated_turns, 0); // Calls used local HTTP, never a live label in docs.
    assert_eq!(server.await.unwrap().len(), 5);
    assert_eq!(
        std::fs::read_to_string(root.path().join("parser.py")).unwrap(),
        patch
    );
    let (_, resumed) = Store::resume(home.path(), &session.id).unwrap();
    assert_eq!(resumed.config.generation_provider, "claude");
}

#[tokio::test]
async fn claude_http_rejection_reports_sanitized_reason_without_retry() {
    for body in [
        json!({"error":{"type":"invalid_request_error","message":"credit balance too low fixture-credential"},"request_id":"req_fixture"}).to_string(),
        "x".repeat(20_000),
    ] {
        let oversized=body.len()>16*1024;
        let (url,server)=mock(vec![(400,body)]).await;
        let generator=s1code::claude::Claude::new("fixture-credential".into(),"claude-fixture",&url).unwrap();
        let (tx,mut rx)=mpsc::unbounded_channel();
        let error=generator.generate(json!({}),&CancellationToken::new(),tx).await.err().unwrap().to_string();
        assert!(error.contains("400") && !error.contains("fixture-credential"));
        if oversized { assert!(error.contains("no readable JSON")); }
        else { assert!(error.contains("credit balance too low") && error.contains("req_fixture")); }
        assert!(rx.try_recv().is_err());
        assert_eq!(server.await.unwrap().len(),1);
    }
}

#[tokio::test]
async fn claude_terminal_failures_keep_reason_and_usage_without_partial_proposal() {
    for reason in ["max_tokens", "refusal", "pause_turn"] {
        let (url, server) = mock(vec![(
            200,
            claude_wire(claude_events("{incomplete", reason)),
        )])
        .await;
        let generator =
            s1code::claude::Claude::new("fixture-key".into(), "claude-fixture", &url).unwrap();
        let (tx, _) = mpsc::unbounded_channel();
        let error = generator
            .generate(json!({}), &CancellationToken::new(), tx)
            .await
            .err()
            .unwrap();
        let failure = error
            .downcast_ref::<s1code::claude::IncompleteResponse>()
            .unwrap();
        assert_eq!(failure.reason, reason);
        assert_eq!(failure.usage.output_tokens, Some(8));
        assert!(error.to_string().contains(reason));
        assert!(!error.to_string().contains("{incomplete"));
        assert_eq!(server.await.unwrap().len(), 1); // Adapter never hides another request.
    }
}

fn claude_tool_events(action: serde_json::Value, stop: &str) -> Vec<serde_json::Value> {
    let mut arguments = action.as_object().unwrap().clone();
    let name = arguments.remove("type").unwrap();
    let input = serde_json::to_string(&arguments).unwrap();
    let mut events = vec![
        json!({"type":"message_start","message":{"model":"claude-fixture","content":[],"usage":{"input_tokens":30,"output_tokens":0}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Preparing the next action."}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tool_fixture","name":name,"input":{}}}),
    ];
    for c in input.chars() {
        events.push(json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":c.to_string()}}));
    }
    events.extend([
        json!({"type":"content_block_stop","index":1}),
        json!({"type":"message_delta","delta":{"stop_reason":stop},"usage":{"output_tokens":42,"output_tokens_details":{"thinking_tokens":12}}}),
        json!({"type":"message_stop"}),
    ]);
    events
}

#[tokio::test]
async fn claude_client_tool_stream_proposes_without_executing_and_counts_reasoning() {
    let action =
        json!({"type":"patch","edits":[{"path":"new.js","before_hash":null,"content":"// çığ\n"}]});
    let (url, server) = mock(vec![(
        200,
        claude_wire(claude_tool_events(action.clone(), "tool_use")),
    )])
    .await;
    let generator = s1code::claude::Claude::new("fixture-key".into(), "claude-opus-5", &url)
        .unwrap()
        .configure(24576, Some("medium".into()))
        .unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let result = generator
        .generate(json!({}), &CancellationToken::new(), tx)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&result.proposal.actions[0]).unwrap(),
        action
    );
    assert_eq!(result.usage.output_tokens, Some(42));
    assert_eq!(result.usage.reasoning_tokens, Some(12));
    assert_eq!(rx.recv().await.unwrap(), "");
    assert_eq!(rx.recv().await.unwrap(), "Preparing the next action.");
    assert!(rx.recv().await.is_none());
    let requests = server.await.unwrap();
    let body: serde_json::Value =
        serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["max_tokens"], 24576);
    assert_eq!(body["output_config"]["effort"], "medium");
    assert!(body["output_config"]["format"].is_null());
}

#[tokio::test]
async fn claude_rejects_unknown_tools_bad_arguments_and_truncated_tool_inputs() {
    for action in [
        json!({"type":"shell","command":"echo bad"}),
        json!({"type":"read","path":"a","start":"bad","lines":3}),
    ] {
        let (url, server) = mock(vec![(
            200,
            claude_wire(claude_tool_events(action, "tool_use")),
        )])
        .await;
        let provider =
            s1code::claude::Claude::new("fixture".into(), "claude-fixture", &url).unwrap();
        let (tx, _) = mpsc::unbounded_channel();
        assert!(
            provider
                .generate(json!({}), &CancellationToken::new(), tx)
                .await
                .is_err()
        );
        server.await.unwrap();
    }
    let mut events = claude_tool_events(json!({"type":"list"}), "max_tokens");
    events.remove(6); // Truncated JSON: only the opening brace is delivered.
    let (url, server) = mock(vec![(200, claude_wire(events))]).await;
    let provider = s1code::claude::Claude::new("fixture".into(), "claude-fixture", &url).unwrap();
    let (tx, _) = mpsc::unbounded_channel();
    let error = provider
        .generate(json!({}), &CancellationToken::new(), tx)
        .await
        .err()
        .unwrap();
    assert_eq!(
        error
            .downcast_ref::<s1code::claude::IncompleteResponse>()
            .unwrap()
            .reason,
        "max_tokens"
    );
    server.await.unwrap();
}
