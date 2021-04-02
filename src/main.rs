use byte_unit::{Byte, ByteUnit};
use hyper::header;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicU64, Ordering};
use ta::indicators::ExponentialMovingAverage;
use ta::Next;
use tokio::{task, time};

lazy_static! {
    static ref REQUESTS: AtomicU64 = AtomicU64::new(0);
    static ref BYTES: AtomicU64 = AtomicU64::new(0);
}

async fn meter() {
    let mut interval = time::interval(time::Duration::from_secs(1));
    let mut requests_ema = ExponentialMovingAverage::new(10).unwrap();
    let mut bytes_ema = ExponentialMovingAverage::new(10).unwrap();
    loop {
        interval.tick().await;
        let requests = REQUESTS.swap(0, Ordering::Relaxed);
        let bytes = BYTES.swap(0, Ordering::Relaxed);

        let human_bytes = Byte::from_unit(bytes as f64, ByteUnit::B).unwrap();

        println!(
            "REQUESTS: {}\nMiB/s:    {}",
            requests_ema.next(requests as f64),
            bytes_ema.next(human_bytes.get_adjusted_unit(ByteUnit::MiB).get_value())
        );
    }
}

async fn srv(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        _ => {
            REQUESTS.fetch_add(1, Ordering::Relaxed);
            if let Some(content_length) = req.headers().get(header::CONTENT_LENGTH) {
                let content_length = content_length.to_str().unwrap();
                BYTES.fetch_add(content_length.parse::<u64>().unwrap(), Ordering::Relaxed);
            }
            let mut okay = Response::default();
            *okay.status_mut() = StatusCode::OK;
            okay.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/text"),
            );
            *okay.body_mut() = req.into_body();
            Ok(okay)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = ([0, 0, 0, 0], 8080).into();
    let service = make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(srv)) });
    let server = Server::bind(&addr).serve(service);

    task::spawn(meter());

    server.await?;
    Ok(())
}
