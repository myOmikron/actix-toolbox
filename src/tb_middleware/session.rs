use std::collections::HashMap;
use std::ops::Add;

pub use actix_session;
pub use actix_session::config::PersistentSession;
use actix_session::storage::{LoadError, SaveError, SessionKey, SessionStore, UpdateError};
pub use actix_session::{Session, SessionMiddleware};
use actix_web::cookie::time::Duration;
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rand::distributions::{Alphanumeric, DistString};
use rorm::{delete, insert, query, update, FieldAccess, Model};

/**
DB representation of a session.
*/
#[derive(Model, Debug, Clone)]
pub struct DBSession {
    /// Key of the session
    #[rorm(primary_key)]
    #[rorm(max_length = 4096)]
    pub session_key: String,

    /// State of the session. json encoded HashMap<String, String>
    #[rorm(max_length = 16383)]
    pub session_state: Option<String>,

    /// DateTime after the session will be invalid
    pub expired_after: DateTime<Utc>,
}

/**
Wrapper for a instance of [rorm::Database].
*/
#[derive(Clone)]
pub struct DBSessionStore(rorm::Database);

impl DBSessionStore {
    /// Create a new DBSessionStore
    ///
    /// **Parameter**:
    /// - `db`: Instance of a connected database
    pub fn new(db: rorm::Database) -> Self {
        Self(db)
    }
}

#[async_trait(?Send)]
impl SessionStore for DBSessionStore {
    async fn load(
        &self,
        session_key: &SessionKey,
    ) -> Result<Option<HashMap<String, String>>, LoadError> {
        let now = Utc::now();

        let session = query!(&self.0, DBSession)
            .condition(DBSession::F.session_key.equals(session_key.as_ref()))
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
        let expired_after =
            Utc::now().add(chrono::Duration::nanoseconds(ttl.whole_nanoseconds() as i64));

        let mut session_key;
        loop {
            session_key = Alphanumeric.sample_string(&mut rand::thread_rng(), 512);

            let res = query!(&self.0, (DBSession::F.session_key,))
                .condition(DBSession::F.session_key.equals(&session_key))
                .optional()
                .await
                .map_err(|e| SaveError::Other(anyhow!(e)))?;

            if res.is_some() {
                continue;
            }

            let state = serde_json::to_string(&session_state)
                .map_err(|e| SaveError::Serialization(anyhow!(e)))?;

            let s = DBSession {
                session_key: session_key.clone(),
                session_state: Some(state),
                expired_after,
            };

            insert!(&self.0, DBSession)
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
        let expired_after =
            Utc::now().add(chrono::Duration::nanoseconds(ttl.whole_nanoseconds() as i64));

        let state = serde_json::to_string(&session_state)
            .map_err(|e| UpdateError::Serialization(anyhow!(e)))?;

        update!(&self.0, DBSession)
            .condition(DBSession::F.session_key.equals(session_key.as_ref()))
            .set(DBSession::F.session_state, Some(state))
            .set(DBSession::F.expired_after, expired_after)
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
        let expired_after =
            Utc::now().add(chrono::Duration::nanoseconds(ttl.whole_nanoseconds() as i64));

        update!(&self.0, DBSession)
            .condition(DBSession::F.session_key.equals(session_key.as_ref()))
            .set(DBSession::F.expired_after, expired_after)
            .exec()
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(())
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        delete!(&self.0, DBSession)
            .condition(DBSession::F.session_key.equals(session_key.as_ref()))
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(())
    }
}
