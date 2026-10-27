use cantos_server::platform::identity::domain::{ActorId, ExternalIdentityKey};
use cantos_server::platform::operator;
use cantos_server::store::Store;
use cantos_store_turso::registry::Intent;
use std::{env, fs::File, io::Read};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The operator owns server.db for the command's duration: stop the Server first.
    let path = env::var_os("CANTOS_SERVER_DATABASE").ok_or("CANTOS_SERVER_DATABASE is required")?;
    let store = Store::open(path, Intent::Serve).await?;
    let args: Vec<String> = env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["sample-metadata", request_path, source_path] => {
            let mut request_bytes = Vec::new();
            File::open(request_path)?
                .take(8193)
                .read_to_end(&mut request_bytes)?;
            if request_bytes.len() > 8192 {
                return Err("sample request exceeds the limit".into());
            }
            let request: cantos_server::contexts::catalog::domain::sample_seed_request::SampleSeedRequest = serde_json::from_slice(&request_bytes)?;
            if request.action != cantos_server::contexts::catalog::domain::sample_seed_request::SampleSeedAction::Publish { return Err("publication request required".into()); }
            let mut bytes = Vec::new();
            File::open(source_path)?
                .take(16103)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 16102 {
                return Err("sample source exceeds the limit".into());
            }
            cantos_server::contexts::catalog::seed_sample(&store, &request, Some(&bytes))
                .await
                .map_err(|_| "sample operator command failed")?;
            println!("sample metadata operation recorded");
            return Ok(());
        }
        ["withdraw-sample-metadata", request_path] => {
            let mut request_bytes = Vec::new();
            File::open(request_path)?
                .take(8193)
                .read_to_end(&mut request_bytes)?;
            if request_bytes.len() > 8192 {
                return Err("sample request exceeds the limit".into());
            }
            let request: cantos_server::contexts::catalog::domain::sample_seed_request::SampleSeedRequest = serde_json::from_slice(&request_bytes)?;
            if request.action != cantos_server::contexts::catalog::domain::sample_seed_request::SampleSeedAction::Withdraw { return Err("withdrawal request required".into()); }
            cantos_server::contexts::catalog::seed_sample(&store, &request, None)
                .await
                .map_err(|_| "sample withdrawal failed")?;
            println!("sample metadata withdrawal recorded");
            return Ok(());
        }
        ["register-identity", issuer, subject] => {
            let key = ExternalIdentityKey::new(*issuer, *subject)?;
            let result = operator::register_identity(&store, &key).await?;
            println!("{}", result.actor_id().as_str());
            return Ok(());
        }
        ["link-identity", actor, issuer, subject] => {
            let key = ExternalIdentityKey::new(*issuer, *subject)?;
            let actor = ActorId::new(*actor)?;
            println!("{:?}", operator::link_identity(&store, &actor, &key).await?);
            return Ok(());
        }
        _ => (),
    }
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["create-creator"] => operator::create_creator(&store).await.map(|id| println!("{id}")),
        ["issue-token", actor, hours] => {
            let hours: u32 = hours.parse().map_err(|_| "hours must be 1..720")?;
            // The credential is returned once, after commit, only on explicit issuance.
            operator::issue_token(&store, actor, hours).await.map(|token| println!("{token}"))
        }
        ["revoke-tokens", actor] => operator::revoke_tokens(&store, actor).await,
        ["register-evidence", owner, kind, id, description] => operator::register_evidence(&store, owner, kind, id, description).await,
        ["record-source", owner, id, reference, path] => {
            let mut text = String::new();
            File::open(path)?.take(1_048_577).read_to_string(&mut text)?;
            operator::record_source(&store, owner, id, reference, &text).await
        }
        _ => return Err("usage: cantos-operator create-creator | register-identity ISSUER SUBJECT | link-identity ACTOR ISSUER SUBJECT | issue-token ACTOR HOURS | revoke-tokens ACTOR | register-evidence OWNER KIND ID DESCRIPTION | record-source OWNER ID REFERENCE UTF8_FILE".into()),
    };
    result.map_err(|_| {
        "operator command failed; check inputs, credentials and database availability".into()
    })
}
