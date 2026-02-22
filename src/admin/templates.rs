use askama::Template;

#[derive(Template)]
#[template(path = "admin/page.html")]
pub struct AdminPage<'a> {
    pub username: &'a str,
    pub email: &'a str,
    pub is_default_admin: bool,
    pub invites: &'a [InviteRow],
    pub users: &'a [UserRow],
    pub stats: &'a StatsData,
}

#[derive(Template)]
#[template(path = "admin/login.html")]
pub struct LoginPage<'a> {
    pub error: &'a str,
}

#[derive(Template)]
#[template(path = "admin/invite_list.html")]
pub struct InviteList<'a> {
    pub invites: &'a [InviteRow],
}

#[derive(Debug, Clone)]
pub struct InviteRow {
    pub id: uuid::Uuid,
    pub sequence: i32,
    pub code: String,
    pub cairn_name: String,
    pub used: bool,
    pub used_by: Option<uuid::Uuid>,
    pub used_at: String,
    pub expires_at: String,
    pub created_at: String,
}

impl InviteRow {
    pub fn has_expires_at(&self) -> bool {
        !self.expires_at.is_empty()
    }
}

#[derive(Template)]
#[template(path = "admin/invite_row.html")]
pub struct InviteRowTemplate<'a> {
    pub invite: &'a InviteRow,
}

#[derive(Template)]
#[template(path = "admin/user_list.html")]
pub struct UserList<'a> {
    pub users: &'a [UserRow],
}

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: uuid::Uuid,
    pub email: String,
    pub username: String,
    pub role: String,
    pub email_verified: bool,
    pub trailblazer_seq: String,
    pub created_at: String,
}

#[derive(Template)]
#[template(path = "admin/user_row.html")]
pub struct UserRowTemplate<'a> {
    pub user: &'a UserRow,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StatsData {
    pub total_users: i64,
    pub total_invites: i64,
    pub used_invites: i64,
    pub total_sketches: i64,
}

#[derive(Template)]
#[template(path = "admin/stats.html")]
pub struct StatsStats<'a> {
    pub stats: &'a StatsData,
}
