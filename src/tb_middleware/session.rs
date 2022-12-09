use std::collections::HashMap;
use std::ops::Add;

use actix_session::storage::{LoadError, SaveError, SessionKey, SessionStore, UpdateError};
use actix_web::cookie::time::Duration;
use anyhow::anyhow;
use async_trait::async_trait;
use rand::distributions::{Alphanumeric, DistString};
use rorm::{delete, insert, query, update, Model};

/**
DB representation of a session.
*/
#[derive(Model)]
pub struct Session {
    /// Key of the session
    #[rorm(primary_key)]
    #[rorm(max_length = 4096)]
    pub session_key: String,

    /// State of the session. json encoded HashMap<String, String>
    #[rorm(max_length = 16777216)]
    pub session_state: Option<String>,

    /// DateTime after the session will be invalid
    pub expired_after: chrono::NaiveDateTime,
}

/**
Wrapper for a instance of [rorm::Database].
*/
pub struct DBSession(rorm::Database);

#[async_trait(?Send)]
impl SessionStore for DBSession {
    async fn load(
        &self,
        session_key: &SessionKey,
    ) -> Result<Option<HashMap<String, String>>, LoadError> {
        let now = chrono::Utc::now().naive_utc();

        let session = query!(&self.0, Session)
            .condition(Session::F.session_key.equals(session_key))
            .optional()
            .await
            .map_err(|e| LoadError::Other(anyhow!(e)))?;

        Ok(if let Some(s) = session {
            if s.expired_after.lt(&now) {
                None
            } else if let Some(state) = s.session_state {
                serde_json::from_str(&state).map_err(|e| LoadError::Deserialization(anyhow!(e)))?
            } else {
                None
            }
        } else {
            None
        })
    }

    async fn save(
        &self,
        session_state: HashMap<String, String>,
        ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        let expired_after = chrono::Utc::now()
            .naive_utc()
            .add(chrono::Duration::nanoseconds(ttl.whole_nanoseconds() as i64));

        let mut session_key;
        loop {
            session_key = Alphanumeric.sample_string(&mut rand::thread_rng(), 512);

            let res = query!(&self.0, (Session::F.session_key,))
                .condition(Session::F.session_key.equals(&session_key))
                .optional()
                .await
                .map_err(|e| SaveError::Other(anyhow!(e)))?;

            if res.is_some() {
                continue;
            }

            let state = serde_json::to_string(&session_state)
                .map_err(|e| SaveError::Serialization(anyhow!(e)))?;

            let s = Session {
                session_key: session_key.clone(),
                session_state: Some(state),
                expired_after,
            };

            insert!(&self.0, Session)
                .single(&s)
                .await
                .map_err(|e| SaveError::Other(anyhow!(e)))?;

            break;
        }

        Ok(SessionKey::try_from(session_key).map_err(|e| SaveError::Other(anyhow!(e)))?)
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: HashMap<String, String>,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        let expired_after = chrono::Utc::now()
            .naive_utc()
            .add(chrono::Duration::nanoseconds(ttl.whole_nanoseconds() as i64));

        let state = serde_json::to_string(&session_state)
            .map_err(|e| UpdateError::Serialization(anyhow!(e)))?;

        update!(&self.0, Session)
            .condition(Session::F.session_key.equals(&session_key))
            .set(Session::F.session_state, state.as_str())
            .set(Session::F.expired_after, expired_after)
            .exec()
            .await
            .map_err(|e| UpdateError::Other(anyhow!(e)))?;

        Ok(session_key)
    }

    async fn update_ttl(
        &self,
        session_key: &SessionKey,
        ttl: &Duration,
    ) -> Result<(), anyhow::Error> {
        let expired_after = chrono::Utc::now()
            .naive_utc()
            .add(chrono::Duration::nanoseconds(ttl.whole_nanoseconds() as i64));

        update!(&self.0, Session)
            .condition(Session::F.session_key.equals(&session_key))
            .set(Session::F.expired_after, expired_after)
            .exec()
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(())
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        delete!(&self.0, Session)
            .condition(Session::F.session_key.equals(&session_key))
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(())
    }
}
