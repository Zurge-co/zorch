use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use http::HeaderName;
use uuid::Uuid;
use zorch_shared::RequestId;

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

pub async fn middleware(mut req: Request, next: Next) -> Response {
    let request_id = RequestId::from(Uuid::now_v7());
    let request_id_str = request_id.to_string();

    req.extensions_mut().insert(request_id);

    let mut response = next.run(req).await;

    if let Ok(header_value) = http::HeaderValue::from_str(&request_id_str) {
        response
            .headers_mut()
            .insert(REQUEST_ID_HEADER, header_value);
    }

    response
}
