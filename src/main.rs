use poem::{
    get, handler, post,
    middleware::AddData,
    web::{Form, Html, Redirect, Data, Path},
    EndpointExt, IntoResponse, Response, Route, Server,
};
use serde::Deserialize;
use async_trait::async_trait;
use std::sync::Arc;
use log::debug;


pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug)]
pub enum StorageError {
    NotFound,
    AlreadyExists,
    InternalError(String),
}

#[async_trait]
pub trait Storage: Send + Sync {
    async fn lookup(&self, id: &str) -> Result<String>;
    async fn store(&self, id: &str, url: &str, owner: &str) -> Result<()>;
}

#[derive(Deserialize)]
struct CreateRedirectRequest {
    short_name: String,
    url: String,
    owner: String,
}

#[handler]
async fn create_redirect(
    Form(payload): Form<CreateRedirectRequest>,
    storage: Data<&Arc<InMemoryStorage>>,
) -> impl IntoResponse {
    let result = storage.store(&payload.short_name, &payload.url, &payload.owner).await;
    
    let (message, is_success) = match result {
        Ok(_) => ("Redirect created successfully".to_string(), true),
        Err(StorageError::AlreadyExists) => ("ID already exists".to_string(), false),
        Err(_) => ("An error occurred while creating the redirect".to_string(), false),
    };

    let mut html = include_str!("templates/index.html").to_string();
    
    if !message.is_empty() {
        html = html.replace(
            "<!-- MESSAGE_PLACEHOLDER -->", 
            &format!("<div class='message {}' style='display:block;'>{}</div>", 
                     if is_success { "success" } else { "error" }, 
                     message)
        );
    }

    Html(html)
}

#[handler]
async fn index() -> impl IntoResponse {
    let html = include_str!("templates/index.html")
        .replace("<!-- MESSAGE_PLACEHOLDER -->", "");
    
    Html(html)
}

#[handler]
async fn handle_redirect(
    Path(id): Path<String>,
    storage: Data<&Arc<InMemoryStorage>>,
) -> Response {
    debug!("Looked up {}", id.as_str());
    match storage.lookup(&id).await {
        Ok(url) => Redirect::permanent(url).into_response(),
        Err(StorageError::NotFound) => Redirect::temporary("/?message=Redirect not found&success=false").into_response(),
        Err(_) => Redirect::temporary("/?message=An error occurred while looking up the redirect&success=false").into_response(),
    }
}

// Example in-memory storage implementation
struct InMemoryStorage {
    data: std::sync::RwLock<std::collections::HashMap<String, (String, String)>>,
}

impl InMemoryStorage {
    fn new() -> Self {
        InMemoryStorage {
            data: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait]
impl Storage for InMemoryStorage {
    async fn lookup(&self, id: &str) -> Result<String> {
        let data = self.data.read().map_err(|e| StorageError::InternalError(e.to_string()))?;
        data.get(id)
            .map(|(url, _)| url.clone())
            .ok_or(StorageError::NotFound)
    }

    async fn store(&self, id: &str, url: &str, owner: &str) -> Result<()> {
        let mut data = self.data.write().map_err(|e| StorageError::InternalError(e.to_string()))?;
        if data.contains_key(id) {
            Err(StorageError::AlreadyExists)
        } else {
            data.insert(id.to_string(), (url.to_string(), owner.to_string()));
            Ok(())
        }
    }
}


#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let storage = Arc::new(InMemoryStorage::new());
    let app = Route::new()
        .at("/", get(index))
        .at("/create", post(create_redirect))
        .at("/go/:id", get(handle_redirect))
        .with(AddData::new(storage));

    println!("Server starting on http://localhost:3000");
    Server::new(poem::listener::TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await
}
