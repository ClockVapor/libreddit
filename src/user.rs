use askama::Template;
use hyper::{Body, Request, Response};
use time::OffsetDateTime;

use crate::client::json;
use crate::esc;
use crate::server::RequestExt;
use crate::utils::{error, filter_posts, format_url, get_filters, param, Post, Preferences, template, User};

// STRUCTS
#[derive(Template)]
#[template(path = "user.html", escape = "none")]
struct UserTemplate {
	user: User,
	posts: Vec<Post>,
	sort: (String, String),
	ends: (String, String),
	prefs: Preferences,
	url: String,
	/// Whether the user themself is filtered.
	is_filtered: bool,
	/// Whether all fetched posts are filtered (to differentiate between no posts fetched in the first place,
	/// and all fetched posts being filtered).
	all_posts_filtered: bool,
}

// FUNCTIONS
pub async fn profile(req: Request<Body>) -> Result<Response<Body>, String> {
	// Build the Reddit JSON API path
	let path = format!(
		"/user/{}.json?{}&raw_json=1",
		req.param("name").unwrap_or_else(|| "reddit".to_string()),
		req.uri().query().unwrap_or_default()
	);
	let url = String::from(req.uri().path_and_query().map_or("", |val| val.as_str()));

	// Retrieve other variables from Libreddit request
	let sort = param(&path, "sort").unwrap_or_default();
	let username = req.param("name").unwrap_or_default();
	let user = user(&username).await.unwrap_or_default();

	let filters = get_filters(&req);
	if filters.contains(&["u_", &username].concat()) {
		template(UserTemplate {
			user,
			posts: Vec::new(),
			sort: (sort, param(&path, "t").unwrap_or_default()),
			ends: (param(&path, "after").unwrap_or_default(), "".to_string()),
			prefs: Preferences::new(req),
			url,
			is_filtered: true,
			all_posts_filtered: false,
		})
	} else {
		// Request user posts/comments from Reddit
		match Post::fetch(&path, false).await {
			Ok((mut posts, after)) => {
				let all_posts_filtered = filter_posts(&mut posts, &filters);

				template(UserTemplate {
					user,
					posts,
					sort: (sort, param(&path, "t").unwrap_or_default()),
					ends: (param(&path, "after").unwrap_or_default(), after),
					prefs: Preferences::new(req),
					url,
					is_filtered: false,
					all_posts_filtered,
				})
			}
			// If there is an error show error page
			Err(msg) => error(req, msg).await,
		}
	}
}

// USER
async fn user(name: &str) -> Result<User, String> {
	// Build the Reddit JSON API path
	let path: String = format!("/user/{}/about.json?raw_json=1", name);

	// Send a request to the url
	json(path, false).await.map(|res| {
		// Grab creation date as unix timestamp
		let created: i64 = res["data"]["created"].as_f64().unwrap_or(0.0).round() as i64;

		// Closure used to parse JSON from Reddit APIs
		let about = |item| res["data"]["subreddit"][item].as_str().unwrap_or_default().to_string();

		// Parse the JSON output into a User struct
		User {
			name: res["data"]["name"].as_str().unwrap_or(name).to_owned(),
			title: esc!(about("title")),
			icon: format_url(&about("icon_img")),
			karma: res["data"]["total_karma"].as_i64().unwrap_or(0),
			created: OffsetDateTime::from_unix_timestamp(created).format("%b %d '%y"),
			banner: esc!(about("banner_img")),
			description: about("public_description"),
		}
	})
}
