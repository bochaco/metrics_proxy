use hyper::{body::Incoming, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use std::{error::Error, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};

const TARGET_HOST: &str = "127.0.0.1";

async fn proxy_handler(
    req: Request<Incoming>,
) -> Result<Response<Incoming>, Box<dyn Error + Send + Sync>> {
    let target_port = &req.uri().path()[1..].parse::<u16>()?;
    let uri = format!("http://{TARGET_HOST}:{target_port}/metrics");
    let url = uri.parse::<hyper::Uri>()?;
    //println!("Request forwarded to {url}");

    let addr = format!("{TARGET_HOST}:{target_port}");
    let stream = TcpStream::connect(addr).await?;
    let io = TokioIo::new(stream);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            eprintln!("Connection failed: {:?}", err);
        }
    });

    let path = url.path();
    let builder = Request::builder().uri(path);

    // Copy headers
    let headers = req.headers().clone();
    let mut req = builder.body(req.into_body())?;
    *req.headers_mut() = headers;

    let res = sender.send_request(req).await?;

    Ok(res)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await?;
    println!("Listening on {addr} ...");

    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            // Finally, we bind the incoming connection to our service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io, service_fn(proxy_handler))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
