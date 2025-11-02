use crate::prelude::*;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}
