use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use methodfig::config::RoleConfig;
use methodfig::llm::openai_compatible::OpenAiCompatibleProvider;
use methodfig::llm::{ChatMessage, ChatProvider, ChatRequest};

#[tokio::test]
async fn openai_compatible_provider_retries_transient_server_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
    let addr = listener.local_addr().expect("local address");
    let server = thread::spawn(move || {
        for attempt in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            if attempt == 0 {
                write_response(&mut stream, 500, r#"{"error":"temporary"}"#);
            } else {
                write_response(
                    &mut stream,
                    200,
                    r#"{"choices":[{"message":{"content":"ok after retry"}}]}"#,
                );
            }
        }
    });

    let provider = OpenAiCompatibleProvider::new(RoleConfig {
        base_url: format!("http://{addr}/v1"),
        api_key: Some("test-key".to_string()),
        model: Some("test-model".to_string()),
    });

    let text = provider
        .complete(ChatRequest {
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            temperature: 0.0,
        })
        .await
        .expect("transient 500 should be retried");

    assert_eq!(text, "ok after retry");
    server.join().expect("server thread should finish");
}

fn write_response(stream: &mut std::net::TcpStream, status: u16, body: &str) {
    let reason = if status == 200 {
        "OK"
    } else {
        "Internal Server Error"
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .expect("write response");
}
