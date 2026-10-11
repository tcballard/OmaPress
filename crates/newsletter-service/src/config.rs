use anyhow::{Context, Result, ensure};
use std::env;

#[derive(Clone)]
pub struct Config {
    pub public_url: String,
    pub admin_token: String,
    pub signing_key: String,
    pub transport: String,
    pub sending_enabled: bool,
    pub test_recipient: String,
    pub ses_region: String,
    pub ses_configuration_set: String,
    pub events_queue_url: String,
    pub events_topic_arn: String,
    pub send_interval_ms: u64,
}
impl Config {
    pub fn from_env() -> Result<Self> {
        let required = |key| env::var(key).with_context(|| format!("Set {key}"));
        let c = Self {
            public_url: required("NEWSLETTER_PUBLIC_URL")?
                .trim_end_matches('/')
                .into(),
            admin_token: required("NEWSLETTER_ADMIN_TOKEN")?,
            signing_key: required("NEWSLETTER_SIGNING_KEY")?,
            transport: env::var("NEWSLETTER_TRANSPORT").unwrap_or("capture".into()),
            sending_enabled: env::var("NEWSLETTER_SENDING_ENABLED").as_deref() == Ok("true"),
            test_recipient: required("NEWSLETTER_TEST_RECIPIENT")?,
            ses_region: env::var("AWS_REGION").unwrap_or("eu-west-2".into()),
            ses_configuration_set: env::var("NEWSLETTER_SES_CONFIGURATION_SET").unwrap_or_default(),
            events_queue_url: env::var("NEWSLETTER_EVENTS_QUEUE_URL").unwrap_or_default(),
            events_topic_arn: env::var("NEWSLETTER_EVENTS_TOPIC_ARN").unwrap_or_default(),
            send_interval_ms: env::var("NEWSLETTER_SEND_INTERVAL_MS")
                .unwrap_or("1000".into())
                .parse()?,
        };
        let u = url::Url::parse(&c.public_url)?;
        ensure!(
            (u.scheme() == "https"
                || (u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1" | "[::1]"))))
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
                && u.path() == "/"
                && u.query().is_none()
                && u.fragment().is_none(),
            "Public URL must be an HTTPS origin (numeric loopback HTTP allowed for development)"
        );
        ensure!(
            c.admin_token.len() >= 32
                && c.signing_key.len() >= 32
                && c.admin_token != c.signing_key,
            "Use distinct random secrets of at least 32 characters"
        );
        ensure!(
            matches!(c.transport.as_str(), "capture" | "ses"),
            "Transport must be capture or ses"
        );
        ensure!(
            c.send_interval_ms >= 100,
            "Sending rate is capped at ten emails per second"
        );
        crate::mail::email(&c.test_recipient)
            .map_err(|_| anyhow::anyhow!("Invalid test recipient"))?;
        if c.transport == "ses" {
            ensure!(
                !c.ses_configuration_set.is_empty()
                    && !c.events_queue_url.is_empty()
                    && !c.events_topic_arn.is_empty(),
                "SES requires a configuration set and SNS→SQS event queue"
            );
        }
        Ok(c)
    }
}
