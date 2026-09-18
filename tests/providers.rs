use nerve::{
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
                socket.write_all(chunk).await.unwrap();
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
