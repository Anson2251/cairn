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
#[template(path = "admin/invite_list_rows.html")]
pub struct InviteListRows<'a> {
    pub invites: &'a [InviteRow],
}

#[derive(Debug, Clone)]
pub struct InviteFormData {
    pub id: uuid::Uuid,
    pub sequence: i32,
    pub code: String,
    pub cairn_name: String,
    pub used: bool,
    pub expires_at: String,
    pub created_at: String,
}

impl InviteFormData {
    pub fn from_row(invite: &InviteRow) -> Self {
        Self {
            id: invite.id,
            sequence: invite.sequence,
            code: invite.code.clone(),
            cairn_name: invite.cairn_name.clone(),
            used: invite.used,
            expires_at: invite.expires_at.clone(),
            created_at: invite.created_at.clone(),
        }
    }
}

#[derive(Template)]
#[template(path = "admin/invite_form.html")]
pub struct InviteForm<'a> {
    pub invite: &'a InviteFormData,
    pub error: &'a str,
}

#[derive(Template)]
#[template(path = "admin/invite_create_form.html")]
pub struct InviteCreateForm<'a> {
    pub error: &'a str,
}

#[derive(Template)]
#[template(path = "admin/user_list.html")]
pub struct UserList<'a> {
    pub users: &'a [UserRow],
}

#[derive(Template)]
#[template(path = "admin/user_list_rows.html")]
pub struct UserListRows<'a> {
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

#[derive(Debug, Clone)]
pub struct UserFormData {
    pub id: uuid::Uuid,
    pub email: String,
    pub username: String,
    pub role: String,
    pub email_verified: bool,
    pub created_at: String,
}

impl UserFormData {
    pub fn empty() -> Self {
        Self {
            id: uuid::Uuid::nil(),
            email: String::new(),
            username: String::new(),
            role: "user".to_string(),
            email_verified: false,
            created_at: String::new(),
        }
    }
}

#[derive(Template)]
#[template(path = "admin/user_form.html")]
pub struct UserForm<'a> {
    pub user: &'a UserFormData,
    pub is_new: bool,
    pub error: &'a str,
}
