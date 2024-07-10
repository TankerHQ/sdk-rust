mod app;
pub use app::App;

mod admin;
pub use admin::Admin;

mod oidc;
#[allow(unused_imports)]
// This module is compiled per-test. Not all tests will use all functions!
pub use oidc::{extract_subject, get_id_token, OIDCProvider};

#[cfg(test)]
mod test_app;
pub use test_app::TestApp;

use base64::prelude::*;
use blake2::digest::{consts, VariableOutput};
use blake2::{Blake2b, Blake2bVar};
use ed25519_dalek::{Signer, SigningKey};
use rand::{rngs::OsRng, Rng};
use serde_json::{json, Value};
use tankersdk::{Error, ErrorCode};

const APP_CREATION_NATURE: u8 = 1;
const APP_SECRET_SIZE: usize = 64;
const APP_PUBLIC_KEY_SIZE: usize = 32;
const AUTHOR_SIZE: usize = 32;
const BLOCK_HASH_SIZE: usize = 32;
const USER_SECRET_SIZE: usize = 32;

pub fn hash_user_id(app_id: &[u8], user_id: &str) -> Vec<u8> {
    use blake2::digest::Update;

    let mut hasher = Blake2bVar::new(BLOCK_HASH_SIZE).unwrap();
    hasher.update(user_id.as_bytes());
    hasher.update(app_id);
    hasher.finalize_boxed().to_vec()
}

pub fn generate_user_secret(hashed_user_id: &[u8]) -> Vec<u8> {
    use blake2::Digest;

    let random_bytes: [u8; USER_SECRET_SIZE - 1] = rand::thread_rng().gen();
    let mut hasher = Blake2b::<consts::U1>::new();
    hasher.update(random_bytes);
    hasher.update(hashed_user_id);

    let mut user_secret = random_bytes.to_vec();
    let res = hasher.finalize();
    user_secret.push(res[0]);
    user_secret
}

pub fn generate_app_id(app_secret: &[u8]) -> Vec<u8> {
    use blake2::digest::Update;

    let mut hasher = Blake2bVar::new(BLOCK_HASH_SIZE).unwrap();
    hasher.update(&[APP_CREATION_NATURE]);
    hasher.update(&[0u8; AUTHOR_SIZE]);
    hasher.update(&app_secret[app_secret.len() - APP_PUBLIC_KEY_SIZE..]);
    hasher.finalize_boxed().to_vec()
}

pub fn create_identity(
    b64_app_id: &str,
    b64_app_secret: &str,
    user_id: &str,
) -> Result<String, Error> {
    let invalid_arg = Error::new(
        ErrorCode::InvalidArgument,
        "Invalid argument to create_identity".into(),
    );

    let app_id = BASE64_STANDARD
        .decode(b64_app_id)
        .map_err(|_| invalid_arg.clone())?;
    let app_secret = BASE64_STANDARD
        .decode(b64_app_secret)
        .map_err(|_| invalid_arg.clone())?;

    if app_id.len() != BLOCK_HASH_SIZE || app_secret.len() != APP_SECRET_SIZE {
        return Err(invalid_arg);
    }
    if app_id != generate_app_id(&app_secret) {
        return Err(invalid_arg);
    }

    let app_secret_keypair =
        SigningKey::from_keypair_bytes(&app_secret.try_into().unwrap()).unwrap();

    let hashed_user_id = hash_user_id(&app_id, user_id);
    let sign_keypair = SigningKey::generate(&mut OsRng {});
    let mut message = sign_keypair.verifying_key().to_bytes().to_vec();
    message.extend(&hashed_user_id);
    let signature = app_secret_keypair.sign(&message).to_bytes();
    let user_secret = generate_user_secret(&hashed_user_id);

    let json = json!({
        "trustchain_id": b64_app_id,
        "target": "user",
        "value": BASE64_STANDARD.encode(&hashed_user_id),
        "delegation_signature": BASE64_STANDARD.encode(signature.as_ref()),
        "ephemeral_public_signature_key": BASE64_STANDARD.encode(sign_keypair.verifying_key()),
        "ephemeral_private_signature_key": BASE64_STANDARD.encode(sign_keypair.to_keypair_bytes().as_ref()),
        "user_secret": BASE64_STANDARD.encode(user_secret),
    });

    Ok(BASE64_STANDARD.encode(json.to_string()))
}

pub fn create_provisional_identity(b64_app_id: &str, email: &str) -> Result<String, Error> {
    let invalid_arg = Error::new(
        ErrorCode::InvalidArgument,
        "Invalid argument to create_provisional_identity".into(),
    );

    let app_id = BASE64_STANDARD
        .decode(b64_app_id)
        .map_err(|_| invalid_arg.clone())?;
    if app_id.len() != BLOCK_HASH_SIZE {
        return Err(invalid_arg);
    }

    let sign_keypair = SigningKey::generate(&mut OsRng {});
    let encrypt_sk = x25519_dalek::StaticSecret::random_from_rng(OsRng);
    let encrypt_pk = x25519_dalek::PublicKey::from(&encrypt_sk);

    let json = json!({
        "trustchain_id": b64_app_id,
        "target": "email",
        "value": email,
        "public_encryption_key": BASE64_STANDARD.encode(encrypt_pk.as_bytes()),
        "private_encryption_key": BASE64_STANDARD.encode(encrypt_sk.to_bytes()),
        "public_signature_key": BASE64_STANDARD.encode(sign_keypair.verifying_key()),
        "private_signature_key": BASE64_STANDARD.encode(sign_keypair.to_keypair_bytes().as_ref()),
    });

    Ok(BASE64_STANDARD.encode(json.to_string()))
}

pub fn get_public_identity(identity_b64: &str) -> Result<String, Error> {
    let invalid_arg = || {
        Error::new(
            ErrorCode::InvalidArgument,
            "Invalid argument to get_public_identity".into(),
        )
    };

    let identity = BASE64_STANDARD
        .decode(identity_b64)
        .map_err(|_| invalid_arg())?;
    let mut identity: Value = serde_json::from_slice(&identity).map_err(|_| invalid_arg())?;

    let public_fields: &[&str] = match identity["target"].as_str() {
        Some("user") => &["trustchain_id", "target", "value"],
        Some("email") => &[
            "trustchain_id",
            "target",
            "value",
            "public_encryption_key",
            "public_signature_key",
        ],
        Some(t) => {
            return Err(Error::new(
                ErrorCode::InvalidArgument,
                format!("Unsupported identity type: {t}"),
            ))
        }
        None => return Err(invalid_arg()),
    };
    let public_identity: Value = public_fields
        .iter()
        .map(|&key| (key.to_owned(), identity[key].take()))
        .collect::<serde_json::Map<_, _>>()
        .into();

    Ok(BASE64_STANDARD.encode(public_identity.to_string()))
}
