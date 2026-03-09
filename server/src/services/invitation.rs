use crate::{
    error::Error,
    models::{
        notification::NotificationModel,
        organization::OrganizationModel,
        pending_invitation::PendingInvitationModel,
    },
    record_id_ext::RecordIdExt,
    services::email::EmailService,
};
use tracing::{debug, error, info, warn};

#[derive(Debug)]
pub enum InviteResult {
    ExistingUser,
    NewUserInvited,
    AlreadyMember,
    AlreadyInvited,
}

pub struct InvitationService;

impl InvitationService {
    /// Invite a user to an organization. Handles both existing and non-existing users.
    pub async fn invite_to_organization(
        org_id: &str,
        org_name: &str,
        org_slug: &str,
        identifier: &str,
        role: &str,
        inviter_id: &str,
        inviter_name: &str,
        message: Option<&str>,
    ) -> Result<InviteResult, Error> {
        let org_model = OrganizationModel::new();
        let notification_model = NotificationModel::new();

        // Try to find the user
        match org_model.find_user_by_username_or_email(identifier).await {
            Ok(person_id) => {
                // Check if already a member
                let existing_role = org_model.get_member_role(org_id, &person_id).await?;
                if existing_role.is_some() {
                    return Ok(InviteResult::AlreadyMember);
                }

                // Check if already has a pending invitation (member_of with pending status)
                let membership_model = crate::models::membership::MembershipModel::new();
                if let Ok(Some(membership)) =
                    membership_model.find_by_person_and_org(&person_id, org_id).await
                {
                    if membership.invitation_status == "pending" {
                        return Ok(InviteResult::AlreadyInvited);
                    }
                }

                // Add member with pending status
                org_model
                    .add_member(org_id, &person_id, role, Some(inviter_id))
                    .await?;

                // Create notification for the invitee
                let mut notification_msg = format!(
                    "{} invited you to join {} as a {}",
                    inviter_name, org_name, role
                );
                if let Some(msg) = message {
                    if !msg.is_empty() {
                        notification_msg.push_str(&format!("\n\n\"{}\"", msg));
                    }
                }

                notification_model
                    .create(
                        &person_id,
                        "invitation",
                        &format!("Invitation to {}", org_name),
                        &notification_msg,
                        Some(&format!("/orgs/{}", org_slug)),
                        Some(org_id),
                    )
                    .await?;

                info!(
                    "Invited existing user {} to organization {} ({})",
                    person_id, org_name, org_slug
                );

                Ok(InviteResult::ExistingUser)
            }
            Err(Error::NotFound) => {
                // User not found — check if identifier is an email
                if !identifier.contains('@') {
                    return Err(Error::BadRequest(format!(
                        "No user found with username '{}'",
                        identifier
                    )));
                }

                let pending_model = PendingInvitationModel::new();

                // Check for existing pending invitation
                if let Some(_) = pending_model.find_existing(identifier, org_id).await? {
                    return Ok(InviteResult::AlreadyInvited);
                }

                // Create pending invitation
                pending_model
                    .create(
                        identifier,
                        "organization",
                        org_id,
                        org_name,
                        org_slug,
                        role,
                        inviter_id,
                    )
                    .await?;

                // Send invitation email
                let base_url = crate::config::app_url();
                let signup_url = format!(
                    "{}/signup?ref=invite&email={}",
                    base_url,
                    urlencoding::encode(identifier)
                );

                match EmailService::from_env() {
                    Ok(email_service) => {
                        let to_email = identifier.to_string();
                        let org = org_name.to_string();
                        let inviter = inviter_name.to_string();
                        let url = signup_url.clone();
                        let msg = message.map(|m| m.to_string());

                        tokio::spawn(async move {
                            if let Err(e) = email_service
                                .send_invitation_email(&to_email, &org, &inviter, &url, msg.as_deref())
                                .await
                            {
                                error!("Failed to send invitation email to {}: {}", to_email, e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Email service not configured, skipping invitation email: {}", e);
                    }
                }

                info!(
                    "Created pending invitation for {} to organization {} ({})",
                    identifier, org_name, org_slug
                );

                Ok(InviteResult::NewUserInvited)
            }
            Err(e) => Err(e),
        }
    }

    /// Process pending invitations for a newly verified user.
    /// Returns the redirect URL of the most recent invitation target, or None.
    pub async fn process_pending_invitations(
        person_id: &str,
        email: &str,
    ) -> Result<Option<String>, Error> {
        debug!(
            "Processing pending invitations for person {} (email: {})",
            person_id, email
        );

        let pending_model = PendingInvitationModel::new();
        let org_model = OrganizationModel::new();
        let notification_model = NotificationModel::new();

        let invitations = pending_model.find_pending_by_email(email).await?;

        if invitations.is_empty() {
            debug!("No pending invitations found for {}", email);
            return Ok(None);
        }

        info!(
            "Found {} pending invitation(s) for {}",
            invitations.len(),
            email
        );

        let mut most_recent_url: Option<String> = None;

        for invitation in &invitations {
            let invitation_id = invitation.id.to_raw_string();

            match invitation.target_type.as_str() {
                "organization" => {
                    // Add member with accepted status (they signed up via invite)
                    if let Err(e) = org_model
                        .add_member(
                            &invitation.target_id,
                            person_id,
                            &invitation.role,
                            None,
                        )
                        .await
                    {
                        error!(
                            "Failed to add person {} to org {}: {}",
                            person_id, invitation.target_id, e
                        );
                        continue;
                    }

                    // Mark invitation as accepted
                    pending_model.mark_accepted(&invitation_id).await?;

                    // Notify the inviter
                    let inviter_id = invitation.invited_by.to_raw_string();
                    let _ = notification_model
                        .create(
                            &inviter_id,
                            "invitation_accepted",
                            &format!("Invitation accepted"),
                            &format!(
                                "{} accepted your invitation to join {}",
                                email, invitation.target_name
                            ),
                            Some(&format!("/orgs/{}", invitation.target_slug)),
                            None,
                        )
                        .await;

                    most_recent_url = Some(format!("/orgs/{}", invitation.target_slug));

                    info!(
                        "Auto-joined person {} to organization {} via pending invitation",
                        person_id, invitation.target_name
                    );
                }
                "production" => {
                    // TODO: Handle production invitations in Phase 2
                    warn!(
                        "Production invitations not yet implemented, skipping: {}",
                        invitation.target_id
                    );
                }
                other => {
                    warn!("Unknown invitation target type: {}", other);
                }
            }
        }

        Ok(most_recent_url)
    }
}
