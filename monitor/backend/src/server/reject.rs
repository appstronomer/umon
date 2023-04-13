use warp::{
    reject::{Reject, Rejection},
    Reply,
};
use http::StatusCode;


#[derive(Debug)]
pub enum ErrorServer {
    Unauthorized,
    BadRequest,
    InternalServerError,
    NotFound,
}
impl ErrorServer {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
impl Reject for ErrorServer {}


pub async fn handle(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    if err.is_not_found() {
        Ok(StatusCode::NOT_FOUND)
    } else if let Some(err) = err.find::<ErrorServer>() {
        Ok(err.status())
    } else {
        eprintln!("unhandled rejection: {:?}", err); // TODO: log this
        Ok(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
