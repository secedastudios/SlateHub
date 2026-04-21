use slatehub::models::membership::{InvitationStatus, MembershipModel, MembershipRole, Permission};

#[test]
fn test_role_conversion() {
    assert_eq!(
        MembershipRole::from_str("owner").unwrap(),
        MembershipRole::Owner
    );
    assert_eq!(
        MembershipRole::from_str("admin").unwrap(),
        MembershipRole::Admin
    );
    assert_eq!(
        MembershipRole::from_str("member").unwrap(),
        MembershipRole::Member
    );
    assert!(MembershipRole::from_str("invalid").is_err());
}

#[test]
fn test_invitation_status_conversion() {
    assert_eq!(
        InvitationStatus::from_str("pending").unwrap(),
        InvitationStatus::Pending
    );
    assert_eq!(
        InvitationStatus::from_str("accepted").unwrap(),
        InvitationStatus::Accepted
    );
    assert_eq!(
        InvitationStatus::from_str("declined").unwrap(),
        InvitationStatus::Declined
    );
    assert!(InvitationStatus::from_str("invalid").is_err());
}

#[test]
fn test_default_permissions() {
    let owner_perms = MembershipModel::get_default_permissions(&MembershipRole::Owner);
    assert!(owner_perms.contains(&Permission::DeleteOrganization));

    let admin_perms = MembershipModel::get_default_permissions(&MembershipRole::Admin);
    assert!(admin_perms.contains(&Permission::InviteMembers));
    assert!(!admin_perms.contains(&Permission::DeleteOrganization));

    let member_perms = MembershipModel::get_default_permissions(&MembershipRole::Member);
    assert!(member_perms.contains(&Permission::CreateProjects));
    assert!(!member_perms.contains(&Permission::InviteMembers));
}
