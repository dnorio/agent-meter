use crate::errors::AppError;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, Debug)]
pub struct CheckoutResponse {
    pub url: String,
    pub mode: String, // "live" or "stub"
}

/// Create a Stripe Checkout session. If STRIPE_SECRET_KEY is missing, returns
/// a stub URL so the UI flow still works in dev.
pub async fn create_checkout(
    stripe_secret_key: Option<&str>,
    price_id: &str,
    org_id: Uuid,
    success_url: &str,
    cancel_url: &str,
) -> Result<CheckoutResponse, AppError> {
    let Some(sk) = stripe_secret_key else {
        return Ok(CheckoutResponse {
            url: format!("/billing/stub?price={}&org={}", price_id, org_id),
            mode: "stub".into(),
        });
    };
    let client = reqwest::Client::new();
    let form = [
        ("mode", "subscription"),
        ("line_items[0][price]", price_id),
        ("line_items[0][quantity]", "1"),
        ("success_url", success_url),
        ("cancel_url", cancel_url),
        ("client_reference_id", &org_id.to_string()),
        ("metadata[org_id]", &org_id.to_string()),
    ];
    let resp = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(sk, Some(""))
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("stripe checkout: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!("stripe checkout {}", body)));
    }

    #[derive(Deserialize)]
    struct CS {
        url: String,
    }
    let parsed: CS = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("stripe parse: {e}")))?;
    Ok(CheckoutResponse {
        url: parsed.url,
        mode: "live".into(),
    })
}

/// Verify Stripe-Signature header. Format: "t=...,v1=...".
pub fn verify_webhook_signature(secret: &str, payload: &[u8], header: &str) -> bool {
    let mut t: Option<&str> = None;
    let mut v1: Option<&str> = None;
    for part in header.split(',') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("t=") {
            t = Some(rest);
        } else if let Some(rest) = part.strip_prefix("v1=") {
            v1 = Some(rest);
        }
    }
    let (Some(t), Some(v1)) = (t, v1) else {
        return false;
    };
    let signed_payload = format!("{}.{}", t, std::str::from_utf8(payload).unwrap_or(""));
    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    expected.len() == v1.len()
        && expected
            .bytes()
            .zip(v1.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

#[derive(Debug, Deserialize)]
pub struct StripeEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: serde_json::Value,
}

pub async fn record_event(pool: &PgPool, ev: &StripeEvent) -> Result<(), AppError> {
    // Try to extract org_id from metadata or client_reference_id
    let org_id = ev
        .data
        .pointer("/object/metadata/org_id")
        .and_then(|v| v.as_str())
        .or_else(|| ev.data.pointer("/object/client_reference_id").and_then(|v| v.as_str()))
        .and_then(|s| Uuid::parse_str(s).ok());

    sqlx::query(
        r#"
        INSERT INTO billing_events (id, org_id, event_type, payload)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(&ev.id)
    .bind(org_id)
    .bind(&ev.event_type)
    .bind(&ev.data)
    .execute(pool)
    .await?;

    // Apply state changes
    if let Some(org_id) = org_id {
        match ev.event_type.as_str() {
            "checkout.session.completed" => {
                let customer = ev
                    .data
                    .pointer("/object/customer")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let subscription = ev
                    .data
                    .pointer("/object/subscription")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                sqlx::query(
                    r#"UPDATE organizations
                       SET stripe_customer_id     = COALESCE($2, stripe_customer_id),
                           stripe_subscription_id = COALESCE($3, stripe_subscription_id),
                           plan                   = 'pro',
                           plan_status            = 'active'
                       WHERE id = $1"#,
                )
                .bind(org_id)
                .bind(customer)
                .bind(subscription)
                .execute(pool)
                .await?;
            }
            "customer.subscription.updated" | "customer.subscription.created" => {
                let status = ev
                    .data
                    .pointer("/object/status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("active");
                sqlx::query("UPDATE organizations SET plan_status = $2 WHERE id = $1")
                    .bind(org_id)
                    .bind(status)
                    .execute(pool)
                    .await?;
            }
            "customer.subscription.deleted" => {
                sqlx::query(
                    "UPDATE organizations SET plan = 'free', plan_status = 'canceled' WHERE id = $1",
                )
                .bind(org_id)
                .execute(pool)
                .await?;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Returns the URL of a Stripe customer portal session.
pub async fn create_portal(
    stripe_secret_key: Option<&str>,
    customer_id: &str,
    return_url: &str,
) -> Result<String, AppError> {
    let Some(sk) = stripe_secret_key else {
        return Ok(format!("/billing/stub?portal=1&cus={}", customer_id));
    };
    let client = reqwest::Client::new();
    let form = [("customer", customer_id), ("return_url", return_url)];
    let resp = client
        .post("https://api.stripe.com/v1/billing_portal/sessions")
        .basic_auth(sk, Some(""))
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("stripe portal: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "stripe portal: {}",
            resp.text().await.unwrap_or_default()
        )));
    }
    #[derive(Deserialize)]
    struct PS {
        url: String,
    }
    let parsed: PS = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("stripe portal parse: {e}")))?;
    Ok(parsed.url)
}

// Avoid unused warning for base64 if not used internally.
#[allow(dead_code)]
fn _b64_unused() {
    let _ = base64::engine::general_purpose::STANDARD.encode("");
}
