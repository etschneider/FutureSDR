use async_io::block_on;
use axum::extract::Extension;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use axum_macros::debug_handler;
use futuresdr::runtime::FlowgraphHandle;
use std::sync::Arc;
use std::sync::Mutex;
use std::time;

use futuresdr::anyhow::Result;
use futuresdr::blocks::MessageSourceBuilder;
use futuresdr::runtime::Flowgraph;
use futuresdr::runtime::Pmt;
use futuresdr::runtime::Runtime;

fn main() -> Result<()> {
    let mut fg = Flowgraph::new();
    let arc_opt_fg_handle: Arc<Mutex<Option<FlowgraphHandle>>> = Arc::new(Mutex::new(None));

    fg.add_block(
        MessageSourceBuilder::new(
            Pmt::String("foo".to_string()),
            time::Duration::from_millis(100),
        )
        .build(),
    );

    let router = Router::new()
        .route("/my_route/", get(my_route))
        .layer(Extension(arc_opt_fg_handle.clone()));

    fg.set_custom_routes(router);

    println!("Visit http://127.0.0.1:1337/my_route/");
    let (task_handle, fgh) = block_on(Runtime::new().start(fg));
    {
        let mut opt_fg_handle = arc_opt_fg_handle.lock().unwrap();
        *opt_fg_handle = Some(fgh.clone());
    }
    block_on(task_handle)?;

    Ok(())
}

#[debug_handler]
async fn my_route(
    Extension(arc_opt_fgh): Extension<Arc<Mutex<Option<FlowgraphHandle>>>>,
) -> Html<String> {
    let opt_fgh = arc_opt_fgh.lock().unwrap().clone();

    let dstr = if let Some(mut fgh) = opt_fgh {
        format!("{:#?}", fgh.description().await.unwrap())
        // format!("Test")
    } else {
        "".to_owned()
    };

    Html(format!(
        r#"
    <html>
        <head>
            <meta charset='utf-8' />
            <title>FutureSDR</title>
        </head>
        <body>
            <h1>My Custom Route</h1>
            Flowgraph description: 
            <pre>{}</pre>
        </body>
    </html>
    "#,
        dstr
    ))
}
