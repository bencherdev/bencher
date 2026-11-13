#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for organization member endpoints.

use bencher_api_tests::{
    TestOrg, TestServer, TestUser,
    helpers::{grant_project_role, user_organizations},
};
use bencher_json::{
    InviteSecret, JsonMembers, OrganizationUuid, UserUuid,
    organization::member::{JsonNewMember, OrganizationRole},
};
use bencher_rbac::project::Role;
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::{Method, StatusCode};

// GET /v0/organizations/{organization}/members - list members
#[tokio::test]
async fn members_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "memberlist@example.com").await;
    let org = server.create_org(&user, "Members Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let members: JsonMembers = resp.json().await.expect("Failed to parse response");
    // Creator should be a member
    assert!(!members.0.is_empty());
}

// GET /v0/organizations/{organization}/members - verify creator is member
#[tokio::test]
async fn members_creator_is_member() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "creatormember@example.com")
        .await;
    let org = server.create_org(&user, "Creator Member Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let members: JsonMembers = resp.json().await.expect("Failed to parse response");

    // Find the creator in the members list
    let creator_found = members.0.iter().any(|m| m.uuid == user.uuid);
    assert!(creator_found, "Creator should be in members list");
}

// POST /v0/organizations/{organization}/members - invite member
#[tokio::test]
async fn members_invite() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "inviter@example.com").await;
    let org = server.create_org(&user, "Invite Org").await;

    let body = JsonNewMember {
        name: Some("Invitee".parse().unwrap()),
        email: "invitee@example.com".parse().unwrap(),
        role: OrganizationRole::Leader,
    };

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .headers(server.fresh_session(&user))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Invitation sent
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// DELETE /v0/organizations/{organization}/members/{user} - a remove that leaves the member listed
// or takes them out of their other organizations
#[tokio::test]
async fn members_remove() {
    let server = TestServer::new().await;
    let leader = server.signup("Test User", "remover@example.com").await;
    let org = server.create_org(&leader, "Remove Org").await;
    let member = server.signup("Test Member", "removed@example.com").await;
    let invite = server.invite(&leader, &org, member.email.as_ref());
    let resp = server
        .client
        .post(server.api_url("/v0/auth/invite/accept"))
        .headers(server.fresh_session(&member))
        .json(&serde_json::json!({"secret": invite.as_ref()}))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        member_uuids(&server, &leader, &org)
            .await
            .contains(&member.uuid)
    );

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/organizations/{}/members/{}",
            org_slug, member.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&leader.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(member_uuids(&server, &leader, &org).await, [leader.uuid]);
    assert_eq!(
        user_organizations(&server, member.uuid),
        [OrganizationUuid::from(member.uuid)]
    );
}

// DELETE /v0/organizations/{organization}/members/{user} - the last Leader stays, whoever asks:
// the Leader themself, leaving, or a server admin; the refusal says what to do, and the Leader is
// still a member, with their project roles.
// Kills a refused removal that takes the Leader's project roles.
#[tokio::test]
async fn last_leader_stays() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "lastleaderadmin@example.com").await;
    let leader = server.signup("Leader", "lastleader@example.com").await;
    let org = server.create_org(&leader, "Last Leader Org").await;
    let _project = server
        .create_project(&leader, &org, "Last Leader Project")
        .await;

    let invited = server
        .signup("Invited", "lastleaderinvited@example.com")
        .await;
    let invite = server.invite(&leader, &org, invited.email.as_ref());

    for by in [&leader, &admin] {
        let (status, json) = remove(&server, by, &org, &leader).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["message"], LAST_LEADER);
    }
    assert_eq!(member_uuids(&server, &admin, &org).await, [leader.uuid]);
    assert_eq!(project_roles(&server, &leader), 1);
    assert_eq!(accept(&server, &invited, &invite).await, StatusCode::OK);
}

// DELETE /v0/organizations/{organization}/members/{user} - a Leader leaves while another stays,
// who then cannot leave.
#[tokio::test]
async fn leader_leaves_while_another_stays() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "leaveadmin@example.com").await;
    let leader = server.signup("Leader", "leaveleader@example.com").await;
    let org = server.create_org(&leader, "Leave Org").await;
    let other = server.signup("Other", "leaveother@example.com").await;
    join(&server, &leader, &org, &other).await;

    let (status, _json) = remove(&server, &leader, &org, &leader).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(member_uuids(&server, &admin, &org).await, [other.uuid]);

    let (status, _json) = remove(&server, &other, &org, &other).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(member_uuids(&server, &admin, &org).await, [other.uuid]);
}

// DELETE /v0/organizations/{organization}/members/{user} - a removal counts the Leaders on the
// writer, in its own write, so a Leader's removal that landed after any read another connection
// made still keeps the other: with the remover gone as the writer sees it, the other Leader is
// the last, and stays.
// Kills a count of the Leaders on any connection but the writer.
#[tokio::test]
async fn removal_meets_the_leaders_the_writer_sees() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "writeradmin@example.com").await;
    let leader = server.signup("Leader", "writerleader@example.com").await;
    let org = server.create_org(&leader, "Writer Org").await;
    let other = server.signup("Other", "writerother@example.com").await;
    join(&server, &leader, &org, &other).await;
    server.remove_after_the_check(&leader).await;

    let (status, json) = remove(&server, &leader, &org, &other).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["message"], LAST_LEADER);
    let mut members = member_uuids(&server, &admin, &org).await;
    members.sort();
    let mut both = vec![leader.uuid, other.uuid];
    both.sort();
    assert_eq!(members, both);
}

// DELETE /v0/organizations/{organization}/members/{user} - two Leaders who remove each other at
// once leave one of them: in each of several organizations, one removal goes through and the
// organization keeps a Leader.
// Kills a count of the Leaders that is not in the same write as the removal.
#[tokio::test(flavor = "multi_thread")]
async fn leaders_removing_each_other_at_once_keep_one() {
    const ORGANIZATIONS: usize = 8;
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "pairadmin@example.com").await;
    let mut pairs = Vec::new();
    for n in 0..ORGANIZATIONS {
        let first = server
            .signup("First", &format!("pairfirst{n}@example.com"))
            .await;
        let org = server.create_org(&first, &format!("Pair Org {n}")).await;
        let second = server
            .signup("Second", &format!("pairsecond{n}@example.com"))
            .await;
        join(&server, &first, &org, &second).await;
        pairs.push((org, first, second));
    }

    // Every removal queues for the writer behind the test, so each pair meets there at once.
    let writer = server.context().database.connection.lock().await;
    let mut requests = tokio::task::JoinSet::new();
    for (n, (org, first, second)) in pairs.iter().enumerate() {
        for (by, member) in [(first, second), (second, first)] {
            let removal = removal(&server, by, org, member);
            requests.spawn(async move { (n, removal.await) });
        }
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    drop(writer);
    let statuses = requests.join_all().await;
    for (n, (org, _first, _second)) in pairs.iter().enumerate() {
        let removed = statuses
            .iter()
            .filter(|(of, status)| *of == n && *status == StatusCode::NO_CONTENT)
            .count();
        assert_eq!(removed, 1, "{n}: {statuses:?}");
        assert_eq!(member_uuids(&server, &admin, org).await.len(), 1, "{n}");
    }
}

// DELETE /v0/organizations/{organization}/members/{user} - a removed member keeps no role on the
// organization's projects: the private project they made, its reports, a rename, a new project
// key, and a delete each answer them as they answer someone who never belonged. Their project
// role in another organization stays, and so does another user's project role in this one.
// Kills a removal that keeps the member's project roles, or that takes their project roles in
// other organizations or another user's.
#[tokio::test]
async fn removed_member_keeps_no_project_role() {
    let server = TestServer::new().await;
    let _admin = server.signup("Admin", "projectroleadmin@example.com").await;
    let leader = server
        .signup("Leader", "projectroleleader@example.com")
        .await;
    let org = server.create_org(&leader, "Project Role Org").await;
    server.license_org(&leader, &org).await;
    let member = server
        .signup("Member", "projectrolemember@example.com")
        .await;
    join(&server, &leader, &org, &member).await;
    let made = server.create_project(&member, &org, "Member Made").await;
    let leaders = server.create_project(&leader, &org, "Leader Made").await;
    let guest = server.signup("Guest", "projectroleguest@example.com").await;
    grant_project_role(&server, &guest, leaders.slug.as_ref(), Role::Maintainer);
    let other = server.signup("Other", "projectroleother@example.com").await;
    let other_org = server.create_org(&other, "Other Org").await;
    let elsewhere = server.create_project(&other, &other_org, "Elsewhere").await;
    grant_project_role(&server, &member, elsewhere.slug.as_ref(), Role::Maintainer);
    let never = server.signup("Never", "projectrolenever@example.com").await;
    let slug: &str = made.slug.as_ref();
    let private = serde_json::json!({"visibility": "private"});
    let status = send(
        &server,
        &leader,
        Method::PATCH,
        &format!("/v0/projects/{slug}"),
        Some(&private),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _json) = remove(&server, &leader, &org, &member).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let rename = serde_json::json!({"name": "Renamed"});
    let key = serde_json::json!({"name": "Kept Key"});
    for (method, path, body) in [
        (Method::GET, format!("/v0/projects/{slug}"), None),
        (Method::GET, format!("/v0/projects/{slug}/reports"), None),
        (Method::PATCH, format!("/v0/projects/{slug}"), Some(&rename)),
        (
            Method::POST,
            format!("/v0/projects/{slug}/keys"),
            Some(&key),
        ),
        (Method::DELETE, format!("/v0/projects/{slug}"), None),
    ] {
        let as_never = send(&server, &never, method.clone(), &path, body).await;
        let as_member = send(&server, &member, method, &path, body).await;
        assert!(!as_member.is_success(), "{path}: {as_member}");
        assert_eq!(as_member, as_never, "{path}");
    }
    let still = serde_json::json!({"name": "Still Kept"});
    for (user, project) in [(&member, &elsewhere), (&guest, &leaders)] {
        let path = format!("/v0/projects/{}", project.slug);
        let status = send(&server, user, Method::PATCH, &path, Some(&still)).await;
        assert_eq!(status, StatusCode::OK, "{path}");
    }
}

// DELETE /v0/organizations/{organization}/members/{user} - a removed member's pending invitations
// to the organization end with the removal: one they sent to another address no longer joins it,
// while one another member sent still does, and so does one the removed member sent to another
// organization.
// Kills a removal that keeps the member's invitations, or that ends another member's or the
// member's own to another organization.
#[tokio::test]
async fn removed_member_s_invitations_end() {
    let server = TestServer::new().await;
    let leader = server
        .signup("Leader", "invitationsleader@example.com")
        .await;
    let org = server.create_org(&leader, "Invitations Org").await;
    let member = server
        .signup("Member", "invitationsmember@example.com")
        .await;
    join(&server, &leader, &org, &member).await;
    let members_org = server.create_org(&member, "Members Own Org").await;
    let sent = server.signup("Sent", "invitationssent@example.com").await;
    let kept = server.signup("Kept", "invitationskept@example.com").await;
    let elsewhere = server
        .signup("Elsewhere", "invitationselsewhere@example.com")
        .await;
    let members_invite = server.invite(&member, &org, sent.email.as_ref());
    let leaders_invite = server.invite(&leader, &org, kept.email.as_ref());
    let members_elsewhere = server.invite(&member, &members_org, elsewhere.email.as_ref());

    let (status, _json) = remove(&server, &leader, &org, &member).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_eq!(
        accept(&server, &sent, &members_invite).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        accept(&server, &kept, &leaders_invite).await,
        StatusCode::OK
    );
    assert_eq!(
        accept(&server, &elsewhere, &members_elsewhere).await,
        StatusCode::OK
    );
    let mut members = member_uuids(&server, &leader, &org).await;
    members.sort();
    let mut stayed = vec![leader.uuid, kept.uuid];
    stayed.sort();
    assert_eq!(members, stayed);
}

// POST /v0/organizations/{organization}/members - invite without name
#[tokio::test]
async fn members_invite_no_name() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "invitername@example.com").await;
    let org = server.create_org(&user, "No Name Invite Org").await;

    let body = JsonNewMember {
        name: None,
        email: "noname@example.com".parse().unwrap(),
        role: OrganizationRole::Leader,
    };

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .headers(server.fresh_session(&user))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Should still work without name
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// POST /v0/organizations/{organization}/members - invalid email
#[tokio::test]
async fn members_invite_invalid_email() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "inviterbad@example.com").await;
    let org = server.create_org(&user, "Bad Email Invite Org").await;

    let body = serde_json::json!({
        "email": "not-an-email",
        "role": "leader"
    });

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[expect(clippy::expect_used, reason = "test helper listing members")]
async fn member_uuids(server: &TestServer, user: &TestUser, org: &TestOrg) -> Vec<UserUuid> {
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "Failed to list members");
    let members: JsonMembers = resp.json().await.expect("Failed to parse response");
    members.0.into_iter().map(|member| member.uuid).collect()
}

/// How many project roles the user holds, as the database has them.
#[expect(clippy::expect_used, reason = "test helper counting project roles")]
fn project_roles(server: &TestServer, user: &TestUser) -> i64 {
    schema::project_role::table
        .inner_join(schema::user::table)
        .filter(schema::user::uuid.eq(user.uuid))
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count project roles")
}

const LAST_LEADER: &str =
    "An organization must keep at least one Leader. Invite another Leader first.";

/// Make `member` a Leader of the organization by an invitation from `leader`, accepted.
async fn join(server: &TestServer, leader: &TestUser, org: &TestOrg, member: &TestUser) {
    let invite = server.invite(leader, org, member.email.as_ref());
    assert_eq!(
        accept(server, member, &invite).await,
        StatusCode::OK,
        "Failed to join"
    );
}

/// `user` accepts the invitation `secret` names, in a browser session they signed in to now: the
/// answer's status.
#[expect(clippy::expect_used, reason = "test helper accepting an invitation")]
async fn accept(server: &TestServer, user: &TestUser, secret: &InviteSecret) -> StatusCode {
    server
        .client
        .post(server.api_url("/v0/auth/invite/accept"))
        .headers(server.fresh_session(user))
        .json(&serde_json::json!({"secret": secret.as_ref()}))
        .send()
        .await
        .expect("Request failed")
        .status()
}

/// `by` removes `member` from the organization, with `by`'s user API key: the answer's status and
/// body.
async fn remove(
    server: &TestServer,
    by: &TestUser,
    org: &TestOrg,
    member: &TestUser,
) -> (StatusCode, serde_json::Value) {
    let (status, body) = removal_with_body(server, by, org, member).await;
    (status, serde_json::from_str(&body).unwrap_or_default())
}

/// [`remove`], sent on its own, with only its status.
fn removal(
    server: &TestServer,
    by: &TestUser,
    org: &TestOrg,
    member: &TestUser,
) -> impl Future<Output = StatusCode> + Send + 'static {
    let removal = removal_with_body(server, by, org, member);
    async move { removal.await.0 }
}

/// A request from `user` in a browser session they signed in to now: its status.
#[expect(clippy::expect_used, reason = "test helper sending a request")]
async fn send(
    server: &TestServer,
    user: &TestUser,
    method: Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> StatusCode {
    let mut request = server
        .client
        .request(method, server.api_url(path))
        .headers(server.fresh_session(user));
    if let Some(body) = body {
        request = request.json(body);
    }
    request.send().await.expect("Request failed").status()
}

#[expect(clippy::expect_used, reason = "test helper removing a member")]
fn removal_with_body(
    server: &TestServer,
    by: &TestUser,
    org: &TestOrg,
    member: &TestUser,
) -> impl Future<Output = (StatusCode, String)> + Send + 'static {
    let org_slug: &str = org.slug.as_ref();
    let request = server
        .client
        .delete(server.api_url(&format!(
            "/v0/organizations/{}/members/{}",
            org_slug, member.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&by.token),
        );
    async move {
        let resp = request.send().await.expect("Request failed");
        let status = resp.status();
        (status, resp.text().await.expect("No body"))
    }
}
