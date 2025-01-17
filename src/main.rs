use bytes::{Bytes, BytesMut};
use http_body_util::{BodyExt, Empty, Full};
use hyper::{body::Incoming, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use std::{error::Error, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};

async fn proxy_handler(
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Box<dyn Error + Send + Sync>> {
    let target_host = "127.0.0.1";
    let target_port = &req.uri().path()[1..].parse::<u16>()?;
    let uri = format!("http://{target_host}:{target_port}/metrics");
    //println!("Request forwarded to {uri}");

    let url = uri.parse::<hyper::Uri>().unwrap();

    let addr = format!("{target_host}:{target_port}");
    let stream = TcpStream::connect(addr).await?;
    let io = TokioIo::new(stream);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            eprintln!("Connection failed: {:?}", err);
        }
    });

    let authority = url.authority().unwrap().clone();

    let path = url.path();
    let req = Request::builder()
        .uri(path)
        .header(hyper::header::HOST, authority.as_str())
        .body(Empty::<Bytes>::new())?;

    let mut res = sender.send_request(req).await?;

    let mut buffer = BytesMut::new();
    while let Some(next) = res.frame().await {
        let frame = next?;
        if let Some(chunk) = frame.data_ref() {
            buffer.extend_from_slice(chunk);
        }
    }
    let response_bytes: Bytes = buffer.freeze();

    Ok(Response::new(Full::new(response_bytes)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Listening on {addr} ...");

    let listener = TcpListener::bind(addr).await?;

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
