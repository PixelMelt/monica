//! Paginated lists (api-design.md §5): `page` / `per_page` in, `meta` out.

use sea_orm::{ConnectionTrait, EntityTrait, FromQueryResult, PaginatorTrait, Select};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::error::ApiResult;

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PageQuery {
    /// 1-based.
    pub page: Option<u64>,
    /// Page size; the default depends on the endpoint, the maximum is 100.
    pub per_page: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PaginationMeta {
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub last_page: u64,
}

/// Runs `select` for the requested page.
pub async fn paginate<E, C>(
    db: &C,
    select: Select<E>,
    q: &PageQuery,
    default_per_page: u64,
) -> ApiResult<(Vec<E::Model>, PaginationMeta)>
where
    E: EntityTrait,
    E::Model: FromQueryResult + Sized + Send + Sync,
    C: ConnectionTrait,
{
    let per_page = q.per_page.unwrap_or(default_per_page).clamp(1, 100);
    let page = q.page.unwrap_or(1).max(1);
    let paginator = select.paginate(db, per_page);
    let totals = paginator.num_items_and_pages().await?;
    let rows = paginator.fetch_page(page - 1).await?;
    Ok((
        rows,
        PaginationMeta { page, per_page, total: totals.number_of_items, last_page: totals.number_of_pages.max(1) },
    ))
}
