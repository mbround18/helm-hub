use diesel::prelude::*;
use diesel::sql_query;
use diesel_async::RunQueryDsl;

use crate::{error::AppError, db::DbConn};

#[derive(Debug, QueryableByName)]
pub struct ChartSeo {
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub app_name: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub title: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub description: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub canonical_path: String,
}

pub async fn chart_seo(
    conn: &mut DbConn,
    owner: &str,
    chart: &str,
) -> Result<ChartSeo, AppError> {
    sql_query("SELECT * FROM seo.chart_seo($1, $2)")
        .bind::<diesel::sql_types::Text, _>(owner)
        .bind::<diesel::sql_types::Text, _>(chart)
        .get_result(conn)
        .await
        .map_err(Into::into)
}
