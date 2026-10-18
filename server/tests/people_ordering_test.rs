//! "No photo → listed last", for every public list of people: the `/people`
//! directory (`Person::directory_page`), hybrid search
//! (`services::search::search_people`), and a member's likes
//! (`LikesModel::get_liked_people`).

mod common;

use slatehub::config;
use slatehub::db::DB;
use slatehub::models::likes::LikesModel;
use slatehub::models::person::Person;
use slatehub::services::search::{self, SearchParams};
use slatehub::services::search_utils;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(serde::Deserialize, SurrealValue)]
struct R {
    id: RecordId,
}

/// Create a directory-eligible person. `avatar: None` = no photo; `Some("")`
/// exercises the legacy empty-string form, which must also count as none.
async fn create_person(
    username: &str,
    display_name: &str,
    headline: &str,
    avatar: Option<&str>,
    status: &str,
    days_old: i64,
) -> RecordId {
    let sql = format!(
        "CREATE person CONTENT {{
            username: $username, email: $email, password: 'h', name: $name,
            verification_status: $status,
            created_at: time::now() - {days_old}d,
            profile: {{ name: $name, headline: $headline, avatar: $avatar, skills: [],
                        social_links: [], ethnicity: [], unions: [], languages: [],
                        education: [], reels: [], media_other: [], awards: [] }}
        }} RETURN id"
    );
    let rows: Vec<R> = DB
        .query(&sql)
        .bind(("username", username.to_string()))
        .bind(("email", format!("{username}@ordering.test")))
        .bind(("name", display_name.to_string()))
        .bind(("status", status.to_string()))
        .bind(("headline", headline.to_string()))
        .bind(("avatar", avatar.map(String::from)))
        .await
        .expect("create person")
        .take(0)
        .expect("take person");
    rows.into_iter().next().expect("one person").id
}

fn usernames<'a>(it: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    it.collect()
}

#[test]
fn directory_lists_photoless_profiles_last_regardless_of_verification_or_age() {
    common::setup_test_db();
    common::clean_table("person");
    common::run(async {
        // Newest and identity-verified, but no photo…
        create_person(
            "ord_nophoto_identity",
            "No Photo",
            "Director",
            None,
            "identity",
            0,
        )
        .await;
        // …a legacy empty-string avatar counts as no photo too…
        create_person(
            "ord_empty_avatar",
            "Blank Avatar",
            "Colorist",
            Some(""),
            "identity",
            1,
        )
        .await;
        // …older and only email-verified, with a photo…
        create_person(
            "ord_photo_email",
            "Has Photo",
            "Editor",
            Some("/p/a.jpg"),
            "email",
            10,
        )
        .await;
        // …and identity-verified with a photo.
        create_person(
            "ord_photo_identity",
            "Has Photo Too",
            "Grip",
            Some("/p/b.jpg"),
            "identity",
            5,
        )
        .await;
        // Never listed: unverified accounts.
        create_person(
            "ord_unverified",
            "Unverified",
            "Gaffer",
            Some("/p/c.jpg"),
            "unverified",
            0,
        )
        .await;

        let page = Person::directory_page(10, 0).await.expect("directory page");
        assert_eq!(
            usernames(page.iter().map(|p| p.username.as_str())),
            [
                "ord_photo_identity",
                "ord_photo_email",
                "ord_nophoto_identity",
                "ord_empty_avatar"
            ],
            "photo first, then identity-verified, then newest"
        );

        // Paging walks the same ordering.
        let second = Person::directory_page(1, 1).await.expect("page 2");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].username, "ord_photo_email");
    });
    common::clean_table("person");
}

#[test]
fn search_lists_photoless_profiles_last_even_when_they_score_higher() {
    common::setup_test_db();
    common::clean_table("person");
    common::run(async {
        // Matches on name AND headline (highest text score) but has no photo…
        create_person(
            "srch_nophoto",
            "Quokka Director",
            "Quokka wrangler",
            None,
            "identity",
            0,
        )
        .await;
        // …vs a headline-only match (lower score) with a photo.
        create_person(
            "srch_photo",
            "Someone Else",
            "Quokka handler",
            Some("/p/q.jpg"),
            "email",
            0,
        )
        .await;
        // Gated in via a skills match alone → score 0 → must not appear at all,
        // and must not consume a page slot (the `WHERE score > 0` lives in SQL).
        let skills_only = create_person(
            "srch_skills_only",
            "Skills Only",
            "Runner",
            Some("/p/s.jpg"),
            "email",
            0,
        )
        .await;
        DB.query("UPDATE $id SET profile.skills = ['Quokka']")
            .bind(("id", skills_only))
            .await
            .expect("set skills");

        let parsed = search_utils::parse_query("quokka");
        let params = SearchParams {
            query: &parsed.cleaned,
            embedding: None,
            weights: config::search_weights(),
            limit: 2,
            offset: 0,
        };
        let results = search::search_people(&params, &parsed, None)
            .await
            .expect("search people");

        assert_eq!(
            usernames(results.iter().map(|r| r.username.as_str())),
            ["srch_photo", "srch_nophoto"],
            "photo'd result first despite the lower score; zero-score row absent"
        );
        assert!(
            results[0].score < results[1].score,
            "sanity: the photo'd one really scores lower"
        );
        assert!(results.iter().all(|r| r.score > 0.0));
    });
    common::clean_table("person");
}

#[test]
fn liked_people_list_photoless_profiles_last() {
    common::setup_test_db();
    common::clean_table("likes");
    common::clean_table("person");
    common::run(async {
        let liker = create_person(
            "like_me",
            "Liker",
            "Producer",
            Some("/p/me.jpg"),
            "identity",
            0,
        )
        .await;
        let a = create_person("like_a_nophoto", "Aardvark", "Actor", None, "identity", 0).await;
        let b = create_person("like_b_photo", "Bee", "Actor", Some("/p/b.jpg"), "email", 0).await;
        let c = create_person("like_c_photo", "Cat", "Actor", Some("/p/c.jpg"), "email", 0).await;
        for target in [&a, &b, &c] {
            assert!(
                LikesModel::toggle(&liker, target).await.expect("like"),
                "toggle returns true when the like is created"
            );
        }

        let liked = LikesModel::get_liked_people(&liker)
            .await
            .expect("liked people");
        assert_eq!(
            usernames(liked.iter().map(|p| p.username.as_str())),
            ["like_b_photo", "like_c_photo", "like_a_nophoto"]
        );
    });
    common::clean_table("likes");
    common::clean_table("person");
}
