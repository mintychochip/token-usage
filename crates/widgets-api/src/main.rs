use toktally_widgets_api::{default_bind, serve};

#[tokio::main]
async fn main() {
    let bind = default_bind();
    serve(&bind, None).await.unwrap();
}
