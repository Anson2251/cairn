use askama::Template;

#[derive(Template)]
#[template(path = "auth/logged_out.html")]
pub struct LoggedOutPage;
