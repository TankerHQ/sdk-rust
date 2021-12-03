use serde_json::{Map, Value};
use tankersdk::{Error, ErrorCode};

pub async fn admin_rest_request(req: reqwest::RequestBuilder) -> Result<Map<String, Value>, Error> {
    let reply = match req.send().await {
        Err(e) => {
            return Err(Error::new_with_source(
                ErrorCode::NetworkError,
                "Admin network request failed".into(),
                e,
            ))
        }
        Ok(reply) => reply,
    };

    let status = reply.status();
    let reply = match reply.text().await {
        Err(e) => {
            return Err(Error::new_with_source(
                ErrorCode::InternalError,
                format!(
                    "Invalid non-text reply for admin request (status {})",
                    status.as_u16()
                ),
                e,
            ))
        }
        Ok(text) => text,
    };

    if status.is_success() {
        let reply: Value = serde_json::from_str(&reply).unwrap();
        if let Value::Object(object) = reply {
            Ok(object)
        } else {
            Err(Error::new(
                ErrorCode::InternalError,
                format!("Invalid JSON reply for admin request: {}", &reply),
            ))
        }
    } else {
        Err(Error::new(
            ErrorCode::InternalError,
            format!("Request error: {}", reply),
        ))
    }
}
