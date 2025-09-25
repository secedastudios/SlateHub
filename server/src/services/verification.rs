use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;
use thiserror::Error;
use tracing::{debug, error, info};

use crate::db::DB;
use crate::error::Error as AppError;

#[derive(Error, Debug)]
pub enum VerificationError {
    #[error("Verification code not found or expired")]
    InvalidCode,
    #[error("Verification code already used")]
    CodeAlreadyUsed,
    #[error("Database error: {0}")]
    DatabaseError(#[from] surrealdb::Error),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<VerificationError> for AppError {
    fn from(err: VerificationError) -> Self {
        match err {
            VerificationError::InvalidCode => AppError::BadRequest(err.to_string()),
            VerificationError::CodeAlreadyUsed => AppError::BadRequest(err.to_string()),
            VerificationError::DatabaseError(e) => AppError::Database(e.to_string()),
            VerificationError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

type Result<T> = std::result::Result<T, VerificationError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCode {
    pub id: RecordId,
    pub person_id: RecordId,
    pub code: String,
    pub code_type: CodeType,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CodeType {
    EmailVerification,
    PasswordReset,
}

impl std::fmt::Display for CodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeType::EmailVerification => write!(f, "email_verification"),
            CodeType::PasswordReset => write!(f, "password_reset"),
        }
    }
}

pub struct VerificationService;

impl VerificationService {
    /// Generate a random 6-digit verification code
    fn generate_code() -> String {
        let mut rng = rand::thread_rng();
        let code: u32 = rng.gen_range(100000..999999);
        code.to_string()
    }

    /// Create a new verification code for a user
    pub async fn create_verification_code(
        person_id: &RecordId,
        code_type: CodeType,
    ) -> Result<String> {
        let code = Self::generate_code();

        // Set expiration based on code type
        let expires_at = match code_type {
            CodeType::EmailVerification => Utc::now() + Duration::hours(24),
            CodeType::PasswordReset => Utc::now() + Duration::hours(1),
        };

        // Delete any existing unused codes of the same type for this user
        let delete_sql = "DELETE verification_codes WHERE person_id = $person_id AND code_type = $code_type AND used = false";
        DB.query(delete_sql)
            .bind(("person_id", person_id.clone()))
            .bind(("code_type", code_type.to_string()))
            .await?;

        // Create the new verification code
        let sql = "CREATE verification_codes SET
            person_id = $person_id,
            code = $code,
            code_type = $code_type,
            expires_at = <datetime>$expires_at,
            used = false,
            created_at = time::now()";

        let mut response = DB
            .query(sql)
            .bind(("person_id", person_id.clone()))
            .bind(("code", code.clone()))
            .bind(("code_type", code_type.to_string()))
            .bind(("expires_at", expires_at.to_rfc3339()))
            .await?;

        let codes: Vec<VerificationCode> = response.take(0)?;

        if codes.is_empty() {
            return Err(VerificationError::Internal(
                "Failed to create verification code".to_string(),
            ));
        }

        debug!(
            "Created {} code for person {}: {}",
            code_type, person_id, code
        );

        Ok(code)
    }

    /// Verify a code and mark it as used
    pub async fn verify_code(person_id: &RecordId, code: &str, code_type: CodeType) -> Result<()> {
        // Find the verification code
        let sql = "SELECT * FROM verification_codes
            WHERE person_id = $person_id
            AND code = $code
            AND code_type = $code_type
            LIMIT 1";

        let mut response = DB
            .query(sql)
            .bind(("person_id", person_id.clone()))
            .bind(("code", code.to_string()))
            .bind(("code_type", code_type.to_string()))
            .await?;

        let codes: Vec<VerificationCode> = response.take(0)?;

        let verification = codes
            .into_iter()
            .next()
            .ok_or(VerificationError::InvalidCode)?;

        // Check if code is already used
        if verification.used {
            return Err(VerificationError::CodeAlreadyUsed);
        }

        // Check if code has expired
        if verification.expires_at < Utc::now() {
            debug!("Code expired for person {}", person_id);
            return Err(VerificationError::InvalidCode);
        }

        // Mark the code as used
        let update_sql = "UPDATE $id SET used = true";
        DB.query(update_sql)
            .bind(("id", verification.id.clone()))
            .await?;

        info!(
            "Successfully verified {} code for person {}",
            code_type, person_id
        );

        Ok(())
    }

    /// Clean up expired verification codes (optional maintenance task)
    pub async fn cleanup_expired_codes() -> Result<u64> {
        let sql = "DELETE verification_codes WHERE expires_at < time::now() RETURN BEFORE";

        let mut response = DB.query(sql).await?;
        let deleted: Vec<VerificationCode> = response.take(0)?;
        let count = deleted.len() as u64;

        if count > 0 {
            info!("Cleaned up {} expired verification codes", count);
        }

        Ok(count)
    }

    /// Check if a person has a valid email verification
    pub async fn is_email_verified(person_id: &str) -> Result<bool> {
        let sql = "SELECT verification_status FROM person WHERE id = $person_id";

        let mut response = DB
            .query(sql)
            .bind(("person_id", format!("person:{}", person_id)))
            .await?;

        #[derive(Deserialize)]
        struct PersonStatus {
            verification_status: String,
        }

        let persons: Vec<PersonStatus> = response.take(0)?;

        Ok(persons
            .first()
            .map(|p| p.verification_status == "email")
            .unwrap_or(false))
    }

    /// Mark a person's email as verified
    pub async fn mark_email_verified(person_id: &RecordId) -> Result<()> {
        let sql = "UPDATE person SET verification_status = 'email' WHERE id = $person_id";

        DB.query(sql).bind(("person_id", person_id.clone())).await?;

        info!("Marked email as verified for person {}", person_id);

        Ok(())
    }
}
