//! The production SMTP adapter against a run-owned loopback sink: real sockets, real SMTP
//! dialog, no external relay. Transport acceptance is not mailbox delivery.
#![cfg(feature = "service")]

mod support;

use cantos_identity::{
    domain::{email::EmailAddress, ids::OperationId, mailbox_proof::ProofId},
    mail::{MailFault, Mailer, ProofMail, SmtpConfig, SmtpMailer, SmtpTls},
};
use support::{SinkMode, SmtpSink};

fn mailer(port: u16) -> SmtpMailer {
    SmtpMailer::new(
        SmtpConfig::new(
            "127.0.0.1".into(),
            port,
            SmtpTls::LoopbackPlaintext,
            "cantos@example.test".into(),
            None,
            None,
        )
        .unwrap(),
    )
    .unwrap()
}

fn mail() -> ProofMail {
    ProofMail::new(
        EmailAddress::parse("Owner@Example.com").unwrap(),
        OperationId::from_bytes([1; 16]).unwrap(),
        ProofId::from_bytes([2; 16]).unwrap(),
        format!(
            "https://theatre.example/signup#operation={}&proof={}&secret={}",
            "01".repeat(16),
            "02".repeat(16),
            "03".repeat(32)
        ),
    )
}

#[tokio::test]
async fn an_accepted_message_reaches_only_its_recipient_with_the_decoded_link_in_the_body() {
    let sink = SmtpSink::start(SinkMode::Accept).await;
    assert_eq!(mailer(sink.port).send(&mail()).await, Ok(()));
    let captured = sink.wait_for(1).await.remove(0);
    assert_eq!(captured.to, ["Owner@example.com"]);
    let link = captured.link();
    assert_eq!(
        (link.operation, link.proof, link.secret),
        ([1; 16], [2; 16], [3; 32])
    );
    let subject = captured
        .raw
        .lines()
        .find(|l| l.starts_with("Subject:"))
        .unwrap();
    assert!(!subject.contains(&support::hex(&link.secret)));
    assert!(captured
        .body()
        .contains("Chỉ mở liên kết không tạo tài khoản"));
}

#[tokio::test]
async fn relay_refusals_are_rejected_a_lost_final_answer_is_uncertain_and_no_relay_is_rejected() {
    let sink = SmtpSink::start(SinkMode::RejectRecipient).await;
    assert_eq!(
        mailer(sink.port).send(&mail()).await,
        Err(MailFault::Rejected)
    );
    assert!(sink.messages().is_empty());
    let sink = SmtpSink::start(SinkMode::RejectData).await;
    assert_eq!(
        mailer(sink.port).send(&mail()).await,
        Err(MailFault::Rejected)
    );
    // The relay read the whole message and then vanished: it may have it, so not "rejected".
    let sink = SmtpSink::start(SinkMode::DropDataAck).await;
    assert_eq!(
        mailer(sink.port).send(&mail()).await,
        Err(MailFault::Uncertain)
    );
    assert_eq!(sink.messages().len(), 1);
    // Nothing listening: positive evidence that nothing was sent.
    let port = {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap().port()
    };
    assert_eq!(mailer(port).send(&mail()).await, Err(MailFault::Rejected));
}
