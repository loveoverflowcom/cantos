use cantos_server::platform::identity::domain::{ActorId, ExternalIdentityKey};
use cantos_server::platform::{operator, postgres::local_config};
use std::{env, fs::File, io::Read};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config =
        local_config(&env::var("DATABASE_URL")?).map_err(|_| "invalid local PostgreSQL config")?;
    let args: Vec<String> = env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["register-identity", issuer, subject] => {
            let key = ExternalIdentityKey::new(*issuer, *subject)?;
            let result = operator::register_identity(&config, &key).await?;
            println!("{}", result.actor_id().as_str());
            return Ok(());
        }
        ["link-identity", actor, issuer, subject] => {
            let key = ExternalIdentityKey::new(*issuer, *subject)?;
            let actor = ActorId::new(*actor)?;
            println!(
                "{:?}",
                operator::link_identity(&config, &actor, &key).await?
            );
            return Ok(());
        }
        _ => (),
    }
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["create-creator"] => operator::create_creator(&config).await.map(|id| println!("{id}")),
        ["issue-token", actor, hours] => {
            let hours: u32 = hours.parse().map_err(|_| "hours must be 1..720")?;
            // The credential is returned once, after commit, only on explicit issuance.
            operator::issue_token(&config, actor, hours).await.map(|token| println!("{token}"))
        }
        ["revoke-tokens", actor] => operator::revoke_tokens(&config, actor).await,
        ["register-evidence", owner, kind, id, description] => operator::register_evidence(&config, owner, kind, id, description).await,
        ["record-source", owner, id, reference, path] => {
            let mut text = String::new();
            File::open(path)?.take(1_048_577).read_to_string(&mut text)?;
            operator::record_source(&config, owner, id, reference, &text).await
        }
        _ => return Err("usage: cantos-operator create-creator | register-identity ISSUER SUBJECT | link-identity ACTOR ISSUER SUBJECT | issue-token ACTOR HOURS | revoke-tokens ACTOR | register-evidence OWNER KIND ID DESCRIPTION | record-source OWNER ID REFERENCE UTF8_FILE".into()),
    };
    result.map_err(|_| {
        "operator command failed; check inputs, credentials and database availability".into()
    })
}
