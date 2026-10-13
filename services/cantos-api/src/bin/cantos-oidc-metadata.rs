//! Read-only discovery/JWKS check, invoked through cargo xtask kanidm verify.
use cantos_api::platform::studio_oidc::{OidcConfig, StudioOidc};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("use cargo xtask kanidm verify --issuer ISSUER --web-origin ORIGIN".into());
    }
    // Metadata fetch never sends client authentication or requests user/account data.
    let config = OidcConfig::new(&args[0], &args[1], "metadata-check-only".into())
        .map_err(|_| "invalid exact Studio OIDC configuration")?;
    let client = StudioOidc::new(config).map_err(|_| "HTTP client unavailable")?;
    client
        .verify_metadata()
        .await
        .map_err(|_| "OIDC discovery/JWKS contract failed")?;
    println!("PASS Studio OIDC discovery/JWKS contract; user login/disable/logout NOT_RUN");
    Ok(())
}
