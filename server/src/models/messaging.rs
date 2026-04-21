use crate::{db::DB, error::Error};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Conversation {
    pub id: RecordId,
    pub participant_a: RecordId,
    pub participant_b: RecordId,
    pub last_message_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub deleted_by: Vec<RecordId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct DirectMessage {
    pub id: RecordId,
    pub conversation: RecordId,
    pub sender: RecordId,
    pub body: String,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct CountResult {
    count: u32,
}

pub struct MessagingModel;

impl Default for MessagingModel {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagingModel {
    pub fn new() -> Self {
        Self
    }

    /// Get or create a conversation between two people.
    /// Participants are stored in canonical order (smaller ID = participant_a).
    pub async fn get_or_create_conversation(
        &self,
        person_a: &str,
        person_b: &str,
    ) -> Result<Conversation, Error> {
        let rid_a =
            RecordId::parse_simple(person_a).map_err(|e| Error::BadRequest(e.to_string()))?;
        let rid_b =
            RecordId::parse_simple(person_b).map_err(|e| Error::BadRequest(e.to_string()))?;

        // Canonical ordering: smaller record ID string is participant_a
        let a_str = person_a.to_string();
        let b_str = person_b.to_string();
        let (canonical_a, canonical_b) = if a_str <= b_str {
            (rid_a, rid_b)
        } else {
            (rid_b, rid_a)
        };

        // Try to find existing conversation
        let existing: Option<Conversation> = DB
            .query(
                "SELECT * FROM conversation WHERE participant_a = $a AND participant_b = $b LIMIT 1",
            )
            .bind(("a", canonical_a.clone()))
            .bind(("b", canonical_b.clone()))
            .await?
            .take(0)?;

        if let Some(conv) = existing {
            return Ok(conv);
        }

        // Create new conversation
        let conv: Option<Conversation> = DB
            .query(
                "CREATE conversation CONTENT {
                    participant_a: $a,
                    participant_b: $b,
                    last_message_at: time::now(),
                    created_at: time::now()
                }",
            )
            .bind(("a", canonical_a))
            .bind(("b", canonical_b))
            .await?
            .take(0)?;

        conv.ok_or_else(|| Error::Database("Failed to create conversation".to_string()))
    }

    /// Send a message in a conversation.
    pub async fn send_message(
        &self,
        conversation_id: &str,
        sender_id: &str,
        body: &str,
    ) -> Result<DirectMessage, Error> {
        let conv_rid = RecordId::parse_simple(conversation_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;
        let sender_rid =
            RecordId::parse_simple(sender_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        // Create the message
        let msg: Option<DirectMessage> = DB
            .query(
                "CREATE direct_message CONTENT {
                    conversation: $conv,
                    sender: $sender,
                    body: $body,
                    read: false
                }",
            )
            .bind(("conv", conv_rid.clone()))
            .bind(("sender", sender_rid))
            .bind(("body", body.to_string()))
            .await?
            .take(0)?;

        // Update conversation last_message_at and clear deleted_by so it reappears for both
        DB.query("UPDATE $conv SET last_message_at = time::now(), deleted_by = []")
            .bind(("conv", conv_rid))
            .await?;

        msg.ok_or_else(|| Error::Database("Failed to create message".to_string()))
    }

    /// Get all conversations for a person, ordered by last message.
    pub async fn get_conversations(&self, person_id: &str) -> Result<Vec<Conversation>, Error> {
        let rid =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        let conversations: Vec<Conversation> = DB
            .query(
                "SELECT * FROM conversation
                 WHERE (participant_a = $pid OR participant_b = $pid)
                   AND $pid NOT IN deleted_by
                 ORDER BY last_message_at DESC",
            )
            .bind(("pid", rid))
            .await?
            .take(0)?;

        Ok(conversations)
    }

    /// Get messages in a conversation, ordered by created_at ascending.
    pub async fn get_messages(
        &self,
        conversation_id: &str,
        limit: u32,
    ) -> Result<Vec<DirectMessage>, Error> {
        let conv_rid = RecordId::parse_simple(conversation_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;

        let messages: Vec<DirectMessage> = DB
            .query(
                "SELECT * FROM direct_message
                 WHERE conversation = $conv
                 ORDER BY created_at ASC
                 LIMIT $limit",
            )
            .bind(("conv", conv_rid))
            .bind(("limit", limit))
            .await?
            .take(0)?;

        Ok(messages)
    }

    /// Mark all messages in a conversation as read for a given recipient.
    pub async fn mark_conversation_read(
        &self,
        conversation_id: &str,
        reader_id: &str,
    ) -> Result<(), Error> {
        let conv_rid = RecordId::parse_simple(conversation_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;
        let reader_rid =
            RecordId::parse_simple(reader_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query(
            "UPDATE direct_message SET read = true
             WHERE conversation = $conv AND sender != $reader AND read = false",
        )
        .bind(("conv", conv_rid))
        .bind(("reader", reader_rid))
        .await?;

        Ok(())
    }

    /// Get unread message count for a person across all conversations.
    pub async fn get_unread_count(&self, person_id: &str) -> Result<u32, Error> {
        let rid =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        let result: Option<CountResult> = DB
            .query(
                "SELECT count() AS count FROM direct_message
                 WHERE sender != $pid AND read = false
                 AND conversation IN (
                     SELECT VALUE id FROM conversation
                     WHERE (participant_a = $pid OR participant_b = $pid)
                       AND $pid NOT IN deleted_by
                 )
                 GROUP ALL",
            )
            .bind(("pid", rid))
            .await?
            .take(0)?;

        Ok(result.map(|r| r.count).unwrap_or(0))
    }

    /// Soft-delete a conversation for a specific person.
    /// The conversation remains visible to the other participant.
    pub async fn delete_conversation(
        &self,
        conversation_id: &str,
        person_id: &str,
    ) -> Result<(), Error> {
        let conv_rid = RecordId::parse_simple(conversation_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;
        let person_rid =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query("UPDATE $conv SET deleted_by += $pid")
            .bind(("conv", conv_rid))
            .bind(("pid", person_rid))
            .await?;

        Ok(())
    }

    /// Get the other participant's person ID from a conversation.
    pub fn get_other_participant(conv: &Conversation, my_id: &str) -> String {
        use crate::record_id_ext::RecordIdExt;
        let a = conv.participant_a.to_raw_string();
        let b = conv.participant_b.to_raw_string();
        if a == my_id { b } else { a }
    }
}
